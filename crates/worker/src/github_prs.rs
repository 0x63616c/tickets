//! Owner-allowlisted PR ingestion, independent of issue mirroring.
use serde::Deserialize;
use serde_json::{Value, json};
use worker::*;

#[derive(Deserialize)]
struct Payload {
    action: String,
    repository: Repository,
    pull_request: PullRequest,
}
#[derive(Deserialize)]
struct Repository {
    full_name: String,
    owner: Owner,
}
#[derive(Deserialize)]
struct Owner {
    login: String,
}
#[derive(Deserialize)]
struct Head {
    sha: String,
}
#[derive(Deserialize)]
struct PullRequest {
    number: u64,
    html_url: String,
    title: String,
    body: Option<String>,
    state: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    merged: bool,
    head: Head,
    updated_at: String,
}

pub async fn handle(env: &Env, event: &str, bytes: &[u8]) -> Result<Response> {
    if event == "ping" {
        return match serde_json::from_slice::<Value>(bytes) {
            Ok(_) => Response::empty(),
            Err(_) => Response::error("Malformed payload", 400),
        };
    }
    let payload: Payload = match serde_json::from_slice(bytes) {
        Ok(p) => p,
        Err(_) => return Response::error("Malformed PR payload", 400),
    };
    let owner = &payload.repository.owner.login;
    let allowed = env
        .var("GITHUB_PR_OWNERS")
        .map(|v| v.to_string())
        .unwrap_or_default();
    if !allowed
        .split(',')
        .any(|v| !v.trim().is_empty() && v.trim().eq_ignore_ascii_case(owner))
    {
        return Ok(Response::empty()?.with_status(202));
    }
    if ![
        "opened",
        "reopened",
        "ready_for_review",
        "converted_to_draft",
        "edited",
        "synchronize",
        "closed",
    ]
    .contains(&payload.action.as_str())
    {
        return Ok(Response::empty()?.with_status(202));
    }
    let pr = payload.pull_request;
    let repo = payload.repository.full_name.to_ascii_lowercase();
    if !repo.starts_with(&format!("{}/", owner.to_ascii_lowercase()))
        || repo.split('/').count() != 2
        || pr.number == 0
        || !pr
            .html_url
            .eq_ignore_ascii_case(&format!("https://github.com/{repo}/pull/{}", pr.number))
        || pr.updated_at.len() != 20
        || !pr.updated_at.ends_with('Z')
        || !["open", "closed"].contains(&pr.state.as_str())
    {
        return Response::error("Invalid PR metadata", 400);
    }
    let state = if pr.merged {
        "merged"
    } else if pr.state == "closed" {
        "closed"
    } else if pr.draft {
        "draft"
    } else {
        "open"
    };
    let links =
        tickets_core::pull_request::links(&pr.title, pr.body.as_deref().unwrap_or_default());
    let db = env.d1("DB")?;
    // Common atomic mutation batch includes trigger-driven status transitions.
    let rows: Vec<Value> = crate::mutation::pull_request(
        &db,
        vec![
            json!(repo),
            json!(pr.number),
            json!(pr.html_url),
            json!(pr.title),
            json!(state),
            json!(pr.head.sha),
            json!(format!("{}.000Z", &pr.updated_at[..19])),
            json!(serde_json::to_string(&links)?),
        ],
    )
    .await
    .map_err(|_| Error::RustError("PR storage failed".into()))?;
    if !rows.is_empty() {
        // Global invalidation also reaches tickets whose link was removed.
        let _ = crate::live::notify(
            env,
            &db,
            crate::api::CommittedChange {
                entity: "tickets".into(),
                id: 0,
                action: "update".into(),
                projects: vec![],
            },
        )
        .await;
    }
    Ok(Response::empty()?.with_status(
        if rows.is_empty() || rows.first().is_some_and(|r| r["matches"] == 0) || links.is_empty() {
            202
        } else {
            204
        },
    ))
}
