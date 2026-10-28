//! Authenticated one-way GitHub ingestion. No GitHub mutation requests exist here.
use crate::repository::Store;
use futures_util::StreamExt;
use js_sys::{Array, Object, Reflect, Uint8Array};
use serde::Deserialize;
use serde_json::Value;
use tickets_core::github_mirror::{self as mapping, Comment, Issue, Label, Repo};
use wasm_bindgen_futures::JsFuture;
use worker::{
    wasm_bindgen::{JsCast, JsValue},
    *,
};

const MAX_BODY: usize = 1024 * 1024;
fn error(message: &str) -> Error {
    Error::RustError(message.into())
}
fn repos(env: &Env) -> Result<Vec<Repo>> {
    mapping::config(&env.var("GITHUB_MIRROR")?.to_string())
        .map_err(|_| error("Invalid mirror configuration"))
}
async fn query(db: &D1Database, sql: &str, p: Vec<Value>) -> Result<Vec<Value>> {
    Store::query(db, sql, p)
        .await
        .map_err(|_| error("Mirror storage failed"))
}
fn statement(db: &D1Database, sql: &str, p: Vec<Value>) -> Result<D1PreparedStatement> {
    let args: Vec<JsValue> = p
        .into_iter()
        .map(|v| match v {
            Value::Null => JsValue::NULL,
            Value::String(s) => s.into(),
            Value::Number(n) => n.as_f64().unwrap().into(),
            _ => unreachable!(),
        })
        .collect();
    db.prepare(sql).bind(&args)
}
async fn batch(db: &D1Database, statements: Vec<D1PreparedStatement>) -> Result<()> {
    for result in db.batch(statements).await? {
        if !result.success() {
            return Err(error("Mirror batch failed"));
        }
    }
    Ok(())
}
async fn project(db: &D1Database, repo: &Repo) -> Result<i64> {
    let rows=query(db,"INSERT INTO projects(name,key_prefix,color) VALUES(?,?,'#789abc') ON CONFLICT(key_prefix) DO UPDATE SET key_prefix=excluded.key_prefix RETURNING id",vec![repo.name().into(),repo.key.clone().into()]).await?;
    rows[0]["id"]
        .as_i64()
        .ok_or_else(|| error("Missing project"))
}
async fn notify(env: &Env, db: &D1Database, project: i64) {
    let _ = crate::live::notify(
        env,
        db,
        crate::api::CommittedChange {
            entity: "tickets".into(),
            id: 0,
            action: "upsert".into(),
            projects: vec![project],
        },
    )
    .await;
}
// Four statements, one transactional batch, regardless of label count. JSON avoids
// exceeding D1's 100-parameter limit on issues with many labels.
async fn issue(db: &D1Database, repo: &Repo, project: i64, issue: &Issue) -> Result<()> {
    if issue.pull_request.is_some() {
        return Ok(());
    }
    let mapped = issue.upsert(project);
    let ext = issue.external_id(&repo.identity);
    let labels = serde_json::to_string(
        &issue
            .labels
            .iter()
            .map(|l| serde_json::json!({"name":l.name,"color":l.ticket_color()}))
            .collect::<Vec<_>>(),
    )?;
    let statements = vec![
        statement(
            db,
            "INSERT INTO tickets(project_id,number,title,body,status,assignee,external_id,external_url) SELECT id,next_ticket_number,?,?,?,?,?,? FROM projects WHERE id=? ON CONFLICT(external_id) DO UPDATE SET title=excluded.title,body=excluded.body,status=excluded.status,assignee=excluded.assignee,external_url=excluded.external_url,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')",
            vec![
                mapped["title"].clone(),
                mapped["body"].clone(),
                mapped["status"].clone(),
                mapped["assignee"].clone(),
                ext.clone().into(),
                mapped["external_url"].clone(),
                project.into(),
            ],
        )?,
        statement(
            db,
            "INSERT INTO labels(name,color) SELECT json_extract(value,'$.name'),json_extract(value,'$.color') FROM json_each(?) WHERE true ON CONFLICT(name) DO UPDATE SET color=excluded.color",
            vec![labels.clone().into()],
        )?,
        statement(
            db,
            "DELETE FROM ticket_labels WHERE ticket_id=(SELECT id FROM tickets WHERE external_id=?)",
            vec![ext.clone().into()],
        )?,
        statement(
            db,
            "INSERT INTO ticket_labels(ticket_id,label_id) SELECT t.id,l.id FROM tickets t JOIN labels l ON l.name IN (SELECT json_extract(value,'$.name') FROM json_each(?)) WHERE t.external_id=?",
            vec![labels.into(), ext.into()],
        )?,
    ];
    batch(db, statements).await
}
async fn comment(
    db: &D1Database,
    repo: &Repo,
    number: u64,
    comment: &Comment,
    deleted: bool,
) -> Result<bool> {
    let ext = format!("github:{}:{number}", repo.identity);
    let marker = comment.marker(&repo.identity);
    let body = comment.body(&repo.identity);
    let rows = query(
        db,
        "SELECT id FROM tickets WHERE external_id=?",
        vec![ext.into()],
    )
    .await?;
    let Some(id) = rows.first().and_then(|r| r["id"].as_i64()) else {
        return Ok(false);
    };
    let mut statements = vec![];
    if deleted {
        statements.push(statement(
            db,
            "DELETE FROM comments WHERE ticket_id=? AND substr(body,1,length(?))=?",
            vec![id.into(), marker.clone().into(), marker.into()],
        )?);
    } else {
        statements.push(statement(db,"UPDATE comments SET body=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE ticket_id=? AND substr(body,1,length(?))=?",vec![body.clone().into(),id.into(),marker.clone().into(),marker.clone().into()])?);
        statements.push(statement(db,"INSERT INTO comments(ticket_id,author,body) SELECT ?,'owner',? WHERE NOT EXISTS(SELECT 1 FROM comments WHERE ticket_id=? AND substr(body,1,length(?))=?)",vec![id.into(),body.into(),id.into(),marker.clone().into(),marker.into()])?);
    }
    batch(db, statements).await?;
    Ok(true)
}
async fn digest(secret: &str, body: &[u8]) -> Result<Vec<u8>> {
    let crypto: web_sys::Crypto =
        Reflect::get(&js_sys::global(), &"crypto".into())?.unchecked_into();
    let subtle = crypto.subtle();
    let algorithm = Object::new();
    Reflect::set(&algorithm, &"name".into(), &"HMAC".into())?;
    Reflect::set(&algorithm, &"hash".into(), &"SHA-256".into())?;
    let key = JsFuture::from(subtle.import_key_with_object(
        "raw",
        &Uint8Array::from(secret.as_bytes()),
        &algorithm,
        false,
        &Array::of1(&"sign".into()),
    )?)
    .await?;
    let result =
        JsFuture::from(subtle.sign_with_str_and_u8_array("HMAC", &key.unchecked_into(), body)?)
            .await?;
    Ok(Uint8Array::new(&result).to_vec())
}
#[derive(Deserialize)]
struct Repository {
    id: u64,
}
#[derive(Deserialize)]
struct NameChange {
    from: String,
}
#[derive(Deserialize, Default)]
struct Changes {
    name: Option<NameChange>,
}
#[derive(Deserialize)]
struct Payload {
    repository: Option<Repository>,
    #[serde(default)]
    action: String,
    issue: Option<Issue>,
    comment: Option<Comment>,
    label: Option<Label>,
    #[serde(default)]
    changes: Changes,
}
pub async fn webhook(mut req: Request, env: &Env) -> Result<Response> {
    if req.method() != Method::Post {
        return Response::error("Method not allowed", 405);
    }
    let secret = match env.secret("GITHUB_WEBHOOK_SECRET") {
        Ok(s) if !s.to_string().is_empty() => s.to_string(),
        _ => return Response::error("Mirror unavailable", 503),
    };
    if req
        .headers()
        .get("Content-Length")?
        .and_then(|s| s.parse::<usize>().ok())
        .is_some_and(|n| n > MAX_BODY)
    {
        return Response::error("Payload too large", 413);
    }
    let signature = req
        .headers()
        .get("X-Hub-Signature-256")?
        .unwrap_or_default();
    if !signature.starts_with("sha256=") || signature.len() != 71 {
        return Response::error("Invalid signature", 401);
    }
    let mut bytes = Vec::new();
    let mut stream = req.stream()?;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if chunk.len() > MAX_BODY.saturating_sub(bytes.len()) {
            return Response::error("Payload too large", 413);
        }
        bytes.extend_from_slice(&chunk);
    }
    if !mapping::signature_matches(&signature, &digest(&secret, &bytes).await?) {
        return Response::error("Invalid signature", 401);
    }
    let event = req.headers().get("X-GitHub-Event")?.unwrap_or_default();
    let payload: Payload = match serde_json::from_slice(&bytes) {
        Ok(p) => p,
        Err(_) => return Response::error("Malformed payload", 400),
    };
    let Some(repo) = repos(env)?
        .into_iter()
        .find(|r| Some(r.id) == payload.repository.as_ref().map(|r| r.id))
    else {
        return Ok(Response::empty()?.with_status(202));
    };
    if event == "ping" {
        return Response::empty();
    }
    if !["issues", "issue_comment", "label"].contains(&event.as_str()) {
        return Ok(Response::empty()?.with_status(204));
    }
    if payload
        .issue
        .as_ref()
        .is_some_and(|i| i.pull_request.is_some())
    {
        return Ok(Response::empty()?.with_status(202));
    }
    let db = env.d1("DB")?;
    let project = project(&db, &repo).await?;
    match event.as_str() {
        "issues" => {
            let Some(i) = payload.issue else {
                return Response::error("Issue required", 400);
            };
            if ["deleted", "transferred"].contains(&payload.action.as_str()) {
                query(
                    &db,
                    "DELETE FROM tickets WHERE external_id=?",
                    vec![i.external_id(&repo.identity).into()],
                )
                .await?;
            } else {
                issue(&db, &repo, project, &i).await?;
            }
        }
        "issue_comment" => {
            let (Some(i), Some(c)) = (payload.issue, payload.comment) else {
                return Response::error("Issue and comment required", 400);
            };
            issue(&db, &repo, project, &i).await?;
            comment(&db, &repo, i.number, &c, payload.action == "deleted").await?;
        }
        "label" => {
            let Some(l) = payload.label else {
                return Response::error("Label required", 400);
            };
            let old = payload
                .changes
                .name
                .map(|n| n.from)
                .unwrap_or_else(|| l.name.clone());
            if payload.action == "deleted" {
                query(&db, "DELETE FROM labels WHERE name=?", vec![l.name.into()]).await?;
            } else if payload.action == "edited" {
                // Labels are global: a rename may collide with a name already
                // used by another repository. Merge memberships transactionally.
                batch(&db,vec![
                    statement(&db,"INSERT INTO labels(name,color) VALUES(?,?) ON CONFLICT(name) DO UPDATE SET color=excluded.color",vec![l.name.clone().into(),l.ticket_color().into()])?,
                    statement(&db,"INSERT OR IGNORE INTO ticket_labels(ticket_id,label_id) SELECT ticket_id,(SELECT id FROM labels WHERE name=?) FROM ticket_labels WHERE label_id=(SELECT id FROM labels WHERE name=?)",vec![l.name.clone().into(),old.clone().into()])?,
                    statement(&db,"DELETE FROM labels WHERE name=? AND name<>?",vec![old.into(),l.name.into()])?,
                ]).await?;
            }
        }
        _ => {}
    }
    if event == "label" {
        let _ = crate::live::notify(
            env,
            &db,
            crate::api::CommittedChange {
                entity: "labels".into(),
                id: 0,
                action: "update".into(),
                projects: vec![],
            },
        )
        .await;
    } else {
        notify(env, &db, project).await;
    }
    Ok(Response::empty()?.with_status(204))
}

#[derive(Deserialize)]
struct Cursor {
    repo_id: u64,
    stream: String,
    since: String,
    high_water: String,
    page: u32,
    etag: Option<String>,
}
fn github_headers(env: &Env) -> Result<Headers> {
    let headers = Headers::new();
    headers.set("User-Agent", "tickets-mirror")?;
    headers.set("Accept", "application/vnd.github+json")?;
    headers.set("X-GitHub-Api-Version", "2022-11-28")?;
    if let Ok(token) = env.secret("GITHUB_TOKEN") {
        headers.set("Authorization", &format!("Bearer {token}"))?;
    }
    Ok(headers)
}
/// One repository per run, alternating each ten minutes. Two GitHub requests and
/// one D1 read when unchanged. At most two issues and two comments: <=23 SQL
/// statements plus project/cursor bookkeeping, even counting batch members.
pub async fn reconcile(env: &Env, scheduled_time: f64) -> Result<()> {
    let repos = repos(env)?;
    if repos.is_empty() {
        return Ok(());
    }
    let repo = &repos[(scheduled_time as u64 / 600_000) as usize % repos.len()];
    let db = env.d1("DB")?;
    let rows: Vec<Cursor> = Store::query(
        &db,
        "SELECT * FROM github_mirror_cursors WHERE repo_id=?",
        vec![repo.id.into()],
    )
    .await
    .map_err(|_| error("Cursor read failed"))?;
    let base = env
        .var("GITHUB_API_BASE_URL")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "https://api.github.com".into());
    // Test override is restricted to HTTP loopback. Never send a token to another host.
    let url = Url::parse(&base)?;
    if base != "https://api.github.com"
        && !(url.scheme() == "http"
            && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")))
    {
        return Err(error("Invalid GitHub API base"));
    }
    let mut project_id = None;
    for stream in ["issues", "comments"] {
        let cursor = rows
            .iter()
            .find(|c| c.stream == stream && c.repo_id == repo.id);
        let since = cursor
            .map(|c| c.since.as_str())
            .unwrap_or("1970-01-01T00:00:00Z");
        let page = cursor.map(|c| c.page).unwrap_or(1);
        let path = if stream == "issues" {
            "issues"
        } else {
            "issues/comments"
        };
        let mut url = Url::parse(&format!("{base}/repositories/{}/{path}", repo.id))?;
        url.query_pairs_mut()
            .append_pair("state", "all")
            .append_pair("sort", "updated")
            .append_pair("direction", "asc")
            .append_pair("since", since)
            .append_pair("per_page", "2")
            .append_pair("page", &page.to_string());
        let headers = github_headers(env)?;
        if let Some(etag) = cursor.and_then(|c| c.etag.as_ref()) {
            headers.set("If-None-Match", etag)?;
        }
        let mut init = RequestInit::new();
        init.with_headers(headers);
        let mut response = Fetch::Request(Request::new_with_init(url.as_str(), &init)?)
            .send()
            .await?;
        match response.status_code() {
            304 => continue,
            403 | 429 => return Ok(()),
            200 => {}
            _ => return Err(error("GitHub reconciliation request failed")),
        }
        let etag = response.headers().get("ETag")?;
        let items: Vec<Value> = response.json().await?;
        let project = match project_id {
            Some(id) => id,
            None => {
                let id = project(&db, repo).await?;
                project_id = Some(id);
                id
            }
        };
        for item in &items {
            if stream == "issues" {
                issue(&db, repo, project, &serde_json::from_value(item.clone())?).await?;
            } else {
                let c: Comment = serde_json::from_value(item.clone())?;
                let number = c
                    .issue_url
                    .rsplit('/')
                    .next()
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| error("Invalid comment issue URL"))?;
                if !comment(&db, repo, number, &c, false).await? {
                    // Repository comment streams also contain PR comments. Resolve
                    // the issue so a PR can never permanently block this stream.
                    let mut init = RequestInit::new();
                    init.with_headers(github_headers(env)?);
                    let mut response = Fetch::Request(Request::new_with_init(
                        &format!("{base}/repositories/{}/issues/{number}", repo.id),
                        &init,
                    )?)
                    .send()
                    .await?;
                    if matches!(response.status_code(), 403 | 429) {
                        return Ok(());
                    }
                    if response.status_code() != 200 {
                        return Ok(());
                    }
                    let missing: Issue = response.json().await?;
                    if missing.pull_request.is_none() {
                        issue(&db, repo, project, &missing).await?;
                        if !comment(&db, repo, number, &c, false).await? {
                            return Ok(());
                        }
                    }
                }
            }
        }
        let timestamps: Vec<String> = items
            .iter()
            .filter_map(|v| v["updated_at"].as_str().map(str::to_owned))
            .collect();
        let (next_since, high_water, next_page) = mapping::advance_cursor(
            since,
            cursor.map(|c| c.high_water.as_str()).unwrap_or(since),
            page,
            &timestamps,
            items.len() == 2,
        );
        let next_etag = if next_since == since && next_page == page {
            etag
        } else {
            None
        };
        query(&db,"INSERT INTO github_mirror_cursors(repo_id,stream,since,high_water,page,etag) VALUES(?,?,?,?,?,?) ON CONFLICT(repo_id,stream) DO UPDATE SET since=excluded.since,high_water=excluded.high_water,page=excluded.page,etag=excluded.etag",vec![repo.id.into(),stream.into(),next_since.into(),high_water.into(),next_page.into(),next_etag.into()]).await?;
        if !items.is_empty() {
            notify(env, &db, project).await;
        }
    }
    Ok(())
}
