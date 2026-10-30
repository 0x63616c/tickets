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
    pub change: Option<CommittedChange>,
}
/// Internal notification metadata; never included in REST responses.
pub struct CommittedChange {
    pub entity: String,
    pub id: i64,
    pub action: String,
    /// Empty for global label invalidations.
    pub projects: Vec<i64>,
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
fn row_id(value: &Value) -> Result<i64> {
    value["id"]
        .as_i64()
        .ok_or_else(|| Error::Storage("Invalid resource ID".into()).into())
}
fn bad(message: impl Into<String>) -> Failure {
    Failure(400, "bad_request", message.into())
}
fn id(s: &str) -> Result<i64> {
    positive_id(s).map_err(bad)
}
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
            change: None,
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
        change: None,
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
    mut projects: Vec<i64>,
) -> Result<Reply> {
    if let Some(project) = value["project_id"].as_i64() {
        projects.push(project);
    }
    if entity == "projects" {
        projects.push(id);
    }
    projects.sort_unstable();
    projects.dedup();
    Ok(Reply {
        status,
        change: Some(CommittedChange {
            entity: entity.into(),
            id,
            action: action.into(),
            projects,
        }),
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
    if method == "GET" && parts == ["api", "v1", "analytics", "home"] {
        return Ok(Reply {
            status: 200,
            change: None,
            body: crate::analytics::home(r.0, q).await?,
        });
    }
    let entity = parts[2];
    if !["projects", "tickets", "labels", "comments"].contains(&entity) {
        return Err(Error::NotFound.into());
    }
    match (method, parts.as_slice()) {
        ("GET", [_, _, e]) if *e != "comments" => list(r, entity, q, None).await,
        ("POST", [_, _, e]) if *e != "comments" => {
            let v = r.create(entity, &body, false).await?;
            changed(r, entity, row_id(&v)?, "create", v, 201, vec![]).await
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
            let projects = r.external_projects(ext).await?;
            let v = r.create("tickets", &body, true).await?;
            changed(r, "tickets", row_id(&v)?, "upsert", v, 200, projects).await
        }
        ("GET" | "POST", [_, _, "tickets", key, "comments"]) => {
            let ticket = resource(r, "tickets", key).await?;
            let t = row_id(&ticket)?;
            if method == "GET" {
                return list(r, "comments", q, Some(t)).await;
            }
            let v = r.create_comment(t, &body).await?;
            changed(
                r,
                "comments",
                row_id(&v)?,
                "create",
                v,
                201,
                vec![ticket["project_id"].as_i64().unwrap()],
            )
            .await
        }
        ("POST" | "DELETE", [_, _, "tickets", key, kind, other])
            if ["labels", "dependencies"].contains(kind) =>
        {
            let ticket = resource(r, "tickets", key).await?;
            let t = row_id(&ticket)?;
            let other_project = r.link(kind, t, id(other)?, method == "POST").await?;
            let mut projects = vec![ticket["project_id"].as_i64().unwrap()];
            projects.extend(other_project);
            changed(
                r,
                kind,
                t,
                if method == "POST" { "create" } else { "delete" },
                json!({"ok":true}),
                200,
                projects,
            )
            .await
        }
        ("GET" | "PATCH" | "DELETE", [_, _, _, key]) => {
            if method == "DELETE" && entity == "projects" {
                return Err(Error::NotFound.into());
            }
            let existing = resource(r, entity, key).await?;
            let n = row_id(&existing)?;
            let mut projects = existing["project_id"]
                .as_i64()
                .into_iter()
                .collect::<Vec<_>>();
            if entity == "comments" && method != "GET" {
                let ticket = r
                    .get("tickets", existing["ticket_id"].as_i64().unwrap())
                    .await?;
                projects.extend(ticket["project_id"].as_i64());
            }
            if entity == "tickets" && method != "GET" {
                projects.extend(r.ticket_projects(n).await?);
            }
            match method {
                "GET" => Ok(Reply {
                    status: 200,
                    change: None,
                    body: r.enrich(entity, existing).await?,
                }),
                "PATCH" => {
                    tickets_core::api::validate(entity, &body).map_err(Error::Validation)?;
                    if body.as_object().unwrap().is_empty() {
                        return Err(bad("Patch must contain fields"));
                    }
                    let v = r.patch(entity, &existing, &body).await?;
                    changed(r, entity, n, "update", v, 200, projects).await
                }
                _ => {
                    r.delete(entity, n).await?;
                    changed(r, entity, n, "delete", json!({"ok":true}), 200, projects).await
                }
            }
        }
        _ => Err(Error::NotFound.into()),
    }
}
