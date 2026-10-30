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
fn project_query(q: &[(String, String)]) -> Result<Option<i64>> {
    match q {
        [] => Ok(None),
        [(key, value)] if key == "project" => Ok(Some(id(value)?)),
        _ => Err(bad("Expected only one optional project parameter")),
    }
}
pub async fn handle<S: Store>(
    store: &S,
    method: &str,
    path: &str,
    query: &[(String, String)],
    body: Value,
) -> Reply {
    handle_as(
        store,
        &tickets_core::history::Actor::system(),
        method,
        path,
        query,
        body,
    )
    .await
}
pub async fn handle_as<S: Store>(
    store: &S,
    actor: &tickets_core::history::Actor,
    method: &str,
    path: &str,
    query: &[(String, String)],
    body: Value,
) -> Reply {
    match dispatch(&ApiRepository(store, actor), method, path, query, body).await {
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
    if method == "GET" && parts == ["api", "v1", "github", "app", "credentials"] {
        // The auth gate derives this actor from a validated bearer token or session.
        if r.1.kind != tickets_core::history::ActorKind::ApiToken {
            return Err(Failure(403, "forbidden", "API token required".into()));
        }
        let stored = crate::github_app_registration::load(r.0)
            .await
            .map_err(|_| Error::Storage("GitHub App load failed".into()))?
            .ok_or(Error::NotFound)?;
        return Ok(Reply {
            status: 200,
            change: None,
            body: json!({
                "app_id": stored.app.app_id,
                "slug": stored.app.slug,
                "name": stored.app.name,
                "owner_login": stored.app.owner_login,
                "client_id": stored.app.client_id,
                "html_url": stored.app.html_url,
                "secrets_ciphertext": stored.secrets_ciphertext,
            }),
        });
    }
    if method == "GET" && parts == ["api", "v1", "github", "app"] {
        return Ok(Reply {
            status: 200,
            change: None,
            body: crate::github_app_events::overview(r.0).await?,
        });
    }
    if method == "GET" && parts == ["api", "v1", "analytics", "home"] {
        return Ok(Reply {
            status: 200,
            change: None,
            body: crate::analytics::home(r.0, q).await?,
        });
    }
    if method == "GET" && parts == ["api", "v1", "mission"] {
        return Ok(Reply {
            status: 200,
            change: None,
            body: crate::mission::get(r.0).await?,
        });
    }
    if let ("PUT", ["api", "v1", "mission", "reports", source]) = (method, parts.as_slice()) {
        return Ok(crate::mission::put(r.0, source, &body.to_string()).await);
    }
    if parts.as_slice() == ["api", "v1", "pickup-rules"]
        && ["GET", "PUT", "DELETE"].contains(&method)
    {
        let project = project_query(q)?;
        return Ok(Reply {
            status: 200,
            change: None,
            body: crate::pickup::rules(r.0, project, method, body).await?,
        });
    }
    if (method == "GET" && parts.as_slice() == ["api", "v1", "next"])
        || (method == "POST" && parts.as_slice() == ["api", "v1", "next", "claim"])
    {
        let claim = method == "POST";
        let project = if claim {
            if !q.is_empty() {
                return Err(bad("Use project in the claim body"));
            }
            #[derive(serde::Deserialize, Default)]
            #[serde(deny_unknown_fields)]
            struct Claim {
                project: Option<i64>,
            }
            let request: Claim = if body.is_null() {
                Claim::default()
            } else {
                serde_json::from_value(body).map_err(|e| bad(e.to_string()))?
            };
            if request.project.is_some_and(|id| id <= 0) {
                return Err(bad("project must be a positive ID"));
            }
            request.project
        } else {
            project_query(q)?
        };
        return match crate::pickup::next(r, project, claim).await? {
            Some(body) => Ok(Reply {
                status: 200,
                change: if claim {
                    Some(CommittedChange {
                        entity: "tickets".into(),
                        id: row_id(&body["ticket"])?,
                        action: "update".into(),
                        projects: vec![body["ticket"]["project_id"].as_i64().unwrap()],
                    })
                } else {
                    None
                },
                body,
            }),
            None => Ok(Reply {
                status: 204,
                change: None,
                body: Value::Null,
            }),
        };
    }
    if let ("POST", [_, _, "tickets", key, "claim", action]) = (method, parts.as_slice()) {
        if !["heartbeat", "release"].contains(action) {
            return Err(Error::NotFound.into());
        }
        if !q.is_empty() {
            return Err(bad("Claim operations do not accept query parameters"));
        }
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Lease {
            claim_id: String,
        }
        let request: Lease = serde_json::from_value(body).map_err(|e| bad(e.to_string()))?;
        if request.claim_id.len() != 32 || !request.claim_id.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(bad("claim_id must be the returned 32-character claim ID"));
        }
        let ticket = resource(r, "tickets", key).await?;
        let id = row_id(&ticket)?;
        let body = crate::pickup::lease(r, id, &request.claim_id, *action == "heartbeat")
            .await?
            .ok_or_else(|| {
                Failure(
                    409,
                    "claim_conflict",
                    "Claim expired, was released, or belongs to another owner".into(),
                )
            })?;
        return Ok(Reply {
            status: 200,
            change: Some(CommittedChange {
                entity: "tickets".into(),
                id,
                action: "update".into(),
                projects: vec![body["project_id"].as_i64().unwrap()],
            }),
            body,
        });
    }
    let entity = parts[2];
    if method == "GET" && parts.as_slice() == ["api", "v1", "search"] {
        let mut seen = std::collections::HashSet::new();
        let mut query = tickets_core::search::SearchQuery::default();
        let mut recent = vec![];
        let mut limit = 30;
        for (key, value) in q {
            if !seen.insert(key) {
                return Err(bad("Duplicate search parameter"));
            }
            match key.as_str() {
                "q" => query = tickets_core::search::SearchQuery::parse(value).map_err(bad)?,
                "limit" => {
                    limit = id(value)?;
                    if limit > 50 {
                        return Err(bad("limit must be 1 through 50"));
                    }
                }
                "recent" if value.is_empty() => {}
                "recent" => {
                    if value.len() > 420 {
                        return Err(bad("Use at most 20 recent ticket IDs"));
                    }
                    for value in value.split(',') {
                        let value = id(value)?;
                        if !recent.contains(&value) {
                            recent.push(value);
                        }
                        if recent.len() > 20 {
                            return Err(bad("Use at most 20 recent ticket IDs"));
                        }
                    }
                }
                _ => return Err(bad("Unknown search parameter")),
            }
        }
        return Ok(Reply {
            status: 200,
            change: None,
            body: crate::search::search(r.0, &query, &recent, limit).await?,
        });
    }
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
        ("GET", [_, _, "tickets", key, "history"]) => {
            let options = list_options("history", q).map_err(bad)?;
            let ticket = resource(r, "tickets", key).await?;
            Ok(Reply {
                status: 200,
                change: None,
                body: r
                    .history(row_id(&ticket)?, options.limit, options.cursor)
                    .await?,
            })
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
                if !r.owns_comment(&existing) {
                    return Err(Failure(
                        403,
                        "forbidden",
                        "Only the comment author can edit or delete it".into(),
                    ));
                }
                if !existing["deleted_at"].is_null() {
                    return Err(Error::NotFound.into());
                }
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
