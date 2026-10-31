//! Runtime-independent installation/check storage; never stores credentials.
use crate::repository::{Error, Result, Statement, Store};
use serde_json::{Value, json};

pub async fn installation<S: Store>(db: &S, event: &str, payload: &Value) -> Result<()> {
    let install = &payload["installation"];
    let id = install["id"]
        .as_i64()
        .filter(|n| *n > 0)
        .ok_or_else(|| Error::Validation("Invalid installation".into()))?;
    let action = payload["action"].as_str().unwrap_or_default();
    if event == "installation" && action == "deleted" {
        let _: Vec<Value> = db
            .query(
                "DELETE FROM github_app_installations WHERE installation_id=?",
                vec![json!(id)],
            )
            .await?;
        return Ok(());
    }
    if !(event == "installation_repositories" && ["added", "removed"].contains(&action)
        || event == "installation"
            && [
                "created",
                "suspend",
                "unsuspend",
                "new_permissions_accepted",
            ]
            .contains(&action))
    {
        return Ok(());
    }
    let login = install["account"]["login"]
        .as_str()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| Error::Validation("Invalid installation account".into()))?;
    let selection = payload["repository_selection"]
        .as_str()
        .or_else(|| install["repository_selection"].as_str());
    let suspended = if action == "unsuspend" {
        Value::Null
    } else if action == "suspend" {
        install["suspended_at"]
            .as_str()
            .map(|v| json!(v))
            .unwrap_or(Value::Null)
    } else {
        install["suspended_at"].clone()
    };
    let mut statements = vec![Statement::new(
        "INSERT INTO github_app_installations(installation_id,account_login,repository_selection,suspended_at) VALUES(?,?,?,CASE WHEN ?='suspend' THEN coalesce(?,strftime('%Y-%m-%dT%H:%M:%fZ','now')) ELSE ? END) ON CONFLICT(installation_id) DO UPDATE SET account_login=excluded.account_login,repository_selection=excluded.repository_selection,suspended_at=excluded.suspended_at,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')",
        vec![
            json!(id),
            json!(login),
            json!(selection),
            json!(action),
            suspended.clone(),
            suspended,
        ],
    )];
    let insert = "INSERT OR IGNORE INTO github_app_installation_repos(installation_id,repo_full_name) SELECT ?,lower(json_extract(value,'$.full_name')) FROM json_each(?) WHERE json_type(value,'$.full_name')='text'";
    if event == "installation" {
        // Some permission/suspension payloads omit repositories; absence is not empty.
        if payload["repositories"].is_array() {
            statements.push(Statement::new(
                "DELETE FROM github_app_installation_repos WHERE installation_id=?",
                vec![json!(id)],
            ));
            statements.push(Statement::new(
                insert,
                vec![json!(id), json!(payload["repositories"].to_string())],
            ));
        }
    } else {
        statements.push(Statement::new("DELETE FROM github_app_installation_repos WHERE installation_id=? AND repo_full_name IN (SELECT lower(json_extract(value,'$.full_name')) FROM json_each(?))",vec![json!(id),json!(payload["repositories_removed"].to_string())]));
        statements.push(Statement::new(
            insert,
            vec![json!(id), json!(payload["repositories_added"].to_string())],
        ));
    }
    db.batch(statements).await?;
    Ok(())
}

pub async fn overview<S: Store>(db: &S) -> Result<Value> {
    let rows: Vec<Value> = db
        .query(
            "SELECT slug,name,html_url FROM github_app WHERE id=1",
            vec![],
        )
        .await?;
    let mut installations:Vec<Value>=db.query("SELECT account_login,repository_selection,(SELECT json_group_array(repo_full_name) FROM (SELECT repo_full_name FROM github_app_installation_repos r WHERE r.installation_id=i.installation_id ORDER BY repo_full_name)) AS repos FROM github_app_installations i ORDER BY account_login,installation_id",vec![]).await?;
    for installation in &mut installations {
        installation["repos"] =
            serde_json::from_str(installation["repos"].as_str().unwrap_or("[]"))
                .map_err(|_| Error::Storage("Invalid repositories".into()))?;
    }
    Ok(
        json!({"registered":!rows.is_empty(),"app_slug":rows.first().map(|v|&v["slug"]),"app_name":rows.first().map(|v|&v["name"]),"html_url":rows.first().map(|v|&v["html_url"]),"installations":installations}),
    )
}

pub async fn tickets<S: Store>(
    db: &S,
    links: &[tickets_core::pull_request::Link],
) -> Result<Vec<Value>> {
    db.query("SELECT p.key_prefix||'-'||t.number AS key,t.title,t.status FROM json_each(?) j JOIN projects p ON p.key_prefix=json_extract(j.value,'$.prefix') JOIN tickets t ON t.project_id=p.id AND t.number=json_extract(j.value,'$.number') ORDER BY p.key_prefix,t.number",vec![json!(serde_json::to_string(links).map_err(|_|Error::Validation("Invalid links".into()))?)]).await
}

pub async fn save_check<S: Store>(
    db: &S,
    repo: &str,
    number: u64,
    sha: &str,
    conclusion: &str,
    run_id: i64,
) -> Result<()> {
    let _:Vec<Value>=db.query("INSERT INTO github_pull_request_checks(repo,number,head_sha,tickets_check,check_run_id) VALUES(?,?,?,?,?) ON CONFLICT(repo,number,head_sha) DO UPDATE SET tickets_check=excluded.tickets_check,check_run_id=excluded.check_run_id,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE excluded.check_run_id>coalesce(github_pull_request_checks.check_run_id,0)",vec![json!(repo),json!(number),json!(sha),json!(conclusion),json!(run_id)]).await?;
    Ok(())
}

/// One snapshot per PR/head; old-head completions never appear on a new head.
pub async fn save_ci<S: Store>(db: &S, repo: &str, suite: &Value) -> Result<()> {
    let sha = suite["head_sha"]
        .as_str()
        .ok_or_else(|| Error::Validation("Missing head SHA".into()))?;
    let updated = suite["updated_at"]
        .as_str()
        .ok_or_else(|| Error::Validation("Missing CI timestamp".into()))?;
    let _:Vec<Value>=db.query("INSERT INTO github_pull_request_checks(repo,number,head_sha,ci_status,ci_updated_at) SELECT ?,number,?,?,? FROM (SELECT json_extract(value,'$.number') AS number FROM json_each(?) WHERE json_extract(value,'$.number')>0 UNION SELECT number FROM github_pull_request_events WHERE repo=? AND head_sha=?) WHERE true ON CONFLICT(repo,number,head_sha) DO UPDATE SET ci_status=excluded.ci_status,ci_updated_at=excluded.ci_updated_at,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE excluded.ci_updated_at>coalesce(github_pull_request_checks.ci_updated_at,'')",vec![json!(repo),json!(sha),suite["conclusion"].clone(),json!(updated),json!(suite["pull_requests"].to_string()),json!(repo),json!(sha)]).await?;
    Ok(())
}
