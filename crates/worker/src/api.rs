//! Thin, runtime independent HTTP routing behind the Worker's authentication gate.
use crate::{
    api_repository::ApiRepository,
    repository::{Error, Store},
};
use serde_json::{Value, json};
use tickets_core::api::{list_options, positive_id};

pub struct Reply {
    pub status: u16,
    pub body: Value,
}
#[derive(Debug)]
struct Failure(u16, &'static str, String);
type Result<T> = std::result::Result<T, Failure>;
impl From<Error> for Failure {
    fn from(e: Error) -> Self {
        match e {
            Error::NotFound => Self(404, "not_found", "Resource not found".into()),
            Error::Validation(s) => Self(422, "validation", s),
            Error::Dependency(e) => {
                use tickets_core::dependency::DependencyError::*;
                Self(
                    409,
                    "dependency_conflict",
                    match e {
                        SelfDependency => "A ticket cannot block itself",
                        Duplicate => "This dependency already exists",
                        Cycle => "This dependency would create a cycle",
                    }
                    .into(),
                )
            }
            Error::Storage(s) => {
                if s.contains("dependency cycle") {
                    Self(
                        409,
                        "dependency_conflict",
                        "This dependency would create a cycle".into(),
                    )
                } else if s.contains("UNIQUE") {
                    Self(409, "conflict", "Resource already exists".into())
                } else if s.contains("FOREIGN KEY") {
                    Self(
                        422,
                        "validation",
                        "Referenced resource does not exist".into(),
                    )
                } else {
                    Self(500, "internal", "Storage operation failed".into())
                }
            }
        }
    }
}
fn bad(message: impl Into<String>) -> Failure {
    Failure(400, "bad_request", message.into())
}
fn id(s: &str) -> Result<i64> {
    positive_id(s).map_err(bad)
}
/// Extension point for #16. Called only after a successful domain mutation.
pub struct ChangeEvent<'a> {
    pub entity: &'a str,
    pub id: i64,
    pub action: &'a str,
}
pub fn change_event(_event: ChangeEvent<'_>) {}

pub async fn handle<S: Store>(
    store: &S,
    method: &str,
    path: &str,
    query: &[(String, String)],
    body: Value,
) -> Reply {
    match dispatch(&ApiRepository(store), method, path, query, body).await {
        Ok(r) => r,
        Err(Failure(status, code, message)) => Reply {
            status,
            body: json!({"error":{"code":code,"message":message}}),
        },
    }
}
async fn resource<S: Store>(r: &ApiRepository<'_, S>, entity: &str, key: &str) -> Result<Value> {
    if entity == "tickets"
        && let Some((prefix, number)) = key.rsplit_once('-')
    {
        tickets_core::domain::validate_prefix(prefix).map_err(bad)?;
        return Ok(r.ticket_key(prefix, id(number)?).await?);
    }
    Ok(r.get(entity, id(key)?).await?)
}
async fn list<S: Store>(
    r: &ApiRepository<'_, S>,
    entity: &str,
    q: &[(String, String)],
    ticket: Option<i64>,
) -> Result<Reply> {
    let o = list_options(entity, q).map_err(bad)?;
    Ok(Reply {
        status: 200,
        body: r
            .list(entity, &o.filters, ticket, o.limit, o.cursor)
            .await?,
    })
}
async fn changed<S: Store>(
    r: &ApiRepository<'_, S>,
    entity: &str,
    id: i64,
    action: &str,
    value: Value,
    status: u16,
) -> Result<Reply> {
    change_event(ChangeEvent { entity, id, action });
    Ok(Reply {
        status,
        body: r.enrich(entity, value).await?,
    })
}
async fn dispatch<S: Store>(
    r: &ApiRepository<'_, S>,
    method: &str,
    path: &str,
    q: &[(String, String)],
    mut body: Value,
) -> Result<Reply> {
    let parts: Vec<_> = path.trim_matches('/').split('/').collect();
    if parts.len() < 3 || parts[..2] != ["api", "v1"] {
        return Err(Error::NotFound.into());
    }
    let entity = parts[2];
    if !["projects", "tickets", "labels", "comments"].contains(&entity) {
        return Err(Error::NotFound.into());
    }
    match (method, parts.as_slice()) {
        ("GET", [_, _, e]) if *e != "comments" => list(r, entity, q, None).await,
        ("POST", [_, _, e]) if *e != "comments" => {
            let v = r.create(entity, &body, false).await?;
            changed(r, entity, v["id"].as_i64().unwrap(), "create", v, 201).await
        }
        ("PUT", [_, _, "tickets", "by-external"]) => {
            let ext = q
                .iter()
                .find(|(k, _)| k == "id")
                .map(|(_, v)| v)
                .ok_or_else(|| bad("id query required"))?;
            if q.len() != 1 {
                return Err(bad("Expected only one id query parameter"));
            }
            let object = body
                .as_object_mut()
                .ok_or_else(|| Error::Validation("expected a JSON object".into()))?;
            object.insert("external_id".into(), ext.clone().into());
            let v = r.create("tickets", &body, true).await?;
            changed(r, "tickets", v["id"].as_i64().unwrap(), "upsert", v, 200).await
        }
        ("GET" | "POST", [_, _, "tickets", key, "comments"]) => {
            let t = resource(r, "tickets", key).await?["id"].as_i64().unwrap();
            if method == "GET" {
                return list(r, "comments", q, Some(t)).await;
            }
            let v = r.create_comment(t, &body).await?;
            changed(r, "comments", v["id"].as_i64().unwrap(), "create", v, 201).await
        }
        ("POST" | "DELETE", [_, _, "tickets", key, kind, other])
            if ["labels", "dependencies"].contains(kind) =>
        {
            let t = resource(r, "tickets", key).await?["id"].as_i64().unwrap();
            r.link(kind, t, id(other)?, method == "POST").await?;
            changed(
                r,
                kind,
                t,
                if method == "POST" { "create" } else { "delete" },
                json!({"ok":true}),
                200,
            )
            .await
        }
        ("GET" | "PATCH" | "DELETE", [_, _, _, key]) => {
            if method == "DELETE" && entity == "projects" {
                return Err(Error::NotFound.into());
            }
            let existing = resource(r, entity, key).await?;
            let n = existing["id"].as_i64().unwrap();
            match method {
                "GET" => Ok(Reply {
                    status: 200,
                    body: r.enrich(entity, existing).await?,
                }),
                "PATCH" => {
                    tickets_core::api::validate(entity, &body).map_err(Error::Validation)?;
                    if body.as_object().unwrap().is_empty() {
                        return Err(bad("Patch must contain fields"));
                    }
                    let v = r.patch(entity, &existing, &body).await?;
                    changed(r, entity, n, "update", v, 200).await
                }
                _ => {
                    r.delete(entity, n).await?;
                    changed(r, entity, n, "delete", json!({"ok":true}), 200).await
                }
            }
        }
        _ => Err(Error::NotFound.into()),
    }
}
