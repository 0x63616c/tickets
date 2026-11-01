//! Durable run persistence. Each mutation and its event commit in one D1 batch.
use crate::{
    api::{Failure, Reply, Result, bad, resource},
    api_repository::ApiRepository,
    repository::{Error, Statement, Store},
};
use serde_json::{Value, json};
use tickets_core::{
    api::positive_id,
    pipeline::{State, validate_document},
};

const ACTIVE: &str = "state NOT IN ('awaiting_review','done','canceled')";
const NOW: &str = "strftime('%Y-%m-%dT%H:%M:%fZ','now')";
fn reply(status: u16, body: Value) -> Reply {
    Reply {
        status,
        body,
        change: None,
    }
}
fn actor<S: Store>(r: &ApiRepository<'_, S>) -> String {
    format!(
        "{}:{}",
        serde_json::to_value(&r.1.kind).unwrap().as_str().unwrap(),
        r.1.id
    )
}
async fn get<S: Store>(store: &S, id: i64) -> Result<Value> {
    let rows: Vec<Value> = store
        .query("SELECT * FROM pipeline_runs WHERE id=?", vec![id.into()])
        .await?;
    rows.into_iter()
        .next()
        .ok_or_else(|| Error::NotFound.into())
}
pub(crate) async fn dispatch<S: Store>(
    r: &ApiRepository<'_, S>,
    method: &str,
    parts: &[&str],
    q: &[(String, String)],
    body: Value,
) -> Result<Reply> {
    if method == "GET" && parts.is_empty() {
        return list(r.0, q).await;
    }
    if !q.is_empty() {
        return Err(bad("Unexpected query parameters"));
    }
    if let ("PUT", [key]) = (method, parts) {
        validate_document(&body, &["intent", "repo", "owner"]).map_err(bad)?;
        let ticket = resource(r, "tickets", key).await?;
        let ticket_id = ticket["id"].clone();
        let intent = body.get("intent").cloned().unwrap_or(json!("merge"));
        let mut results=r.0.batch(vec![
            Statement::new("INSERT INTO pipeline_runs(ticket_id,key,repo,intent,owner) VALUES(?,?,?,?,?) ON CONFLICT DO NOTHING RETURNING *",vec![ticket_id.clone(),ticket["key"].clone(),body["repo"].clone(),intent,body["owner"].clone()]),
            Statement::new(format!("INSERT INTO pipeline_run_events(run_id,actor,from_state,to_state,detail) SELECT id,?,NULL,'queued','{{\"action\":\"created\"}}' FROM pipeline_runs WHERE ticket_id=? AND {ACTIVE} AND NOT EXISTS(SELECT 1 FROM pipeline_run_events e WHERE e.run_id=pipeline_runs.id)"),vec![actor(r).into(),ticket_id.clone()]),
            Statement::new(format!("SELECT * FROM pipeline_runs WHERE ticket_id=? AND {ACTIVE}"),vec![ticket_id]),
        ]).await?;
        let status = if results[0].is_empty() { 200 } else { 201 };
        return Ok(reply(
            status,
            results.pop().unwrap().pop().ok_or(Error::NotFound)?,
        ));
    }
    let Some(raw_id) = parts.first() else {
        return Err(Error::NotFound.into());
    };
    let id = positive_id(raw_id).map_err(bad)?;
    match (method, parts) {
        ("GET", [_]) => Ok(reply(200, get(r.0, id).await?)),
        ("GET", [_, "events"]) => {
            get(r.0, id).await?;
            let mut rows: Vec<Value> =
                r.0.query(
                    "SELECT * FROM pipeline_run_events WHERE run_id=? ORDER BY id",
                    vec![id.into()],
                )
                .await?;
            for row in &mut rows {
                row["detail"] = serde_json::from_str(row["detail"].as_str().unwrap())
                    .map_err(|e| Error::Storage(e.to_string()))?;
            }
            Ok(reply(200, json!({"data":rows})))
        }
        ("PATCH", [_]) => patch(r, id, body).await,
        _ => Err(Error::NotFound.into()),
    }
}
async fn patch<S: Store>(r: &ApiRepository<'_, S>, id: i64, mut body: Value) -> Result<Reply> {
    let allowed = [
        "expected_version",
        "state",
        "owner",
        "claim_id",
        "pr",
        "head_sha",
        "attempts",
        "last_error",
        "next_check_at",
        "escalated_at",
        "intent",
    ];
    validate_document(&body, &allowed).map_err(bad)?;
    let version = body
        .get("expected_version")
        .and_then(Value::as_i64)
        .ok_or_else(|| bad("expected_version is required"))?;
    body.as_object_mut().unwrap().remove("expected_version");
    // The event is inserted from the old row before UPDATE; both use the same
    // version/terminal guard under the batch's write lock. No pre-read race.
    let guard = format!(
        "id=?1 AND version=?2 AND ({ACTIVE} OR NOT EXISTS(SELECT 1 FROM json_each(?3) WHERE key='state' AND value IS NOT pipeline_runs.state))"
    );
    let params = vec![id.into(), version.into(), body.to_string().into()];
    // Avoid a compound SELECT (D1 limits apply); json_each carries all supplied fields.
    let different = body
        .as_object()
        .unwrap()
        .keys()
        .map(|k| format!("WHEN '{k}' THEN {k} IS NOT j.value"))
        .collect::<Vec<_>>()
        .join(" ");
    let detail = if body.as_object().unwrap().is_empty() {
        "'{}'".into()
    } else {
        format!(
            "(SELECT json_group_object(j.key,json_object('old',CASE j.key {} END,'new',j.value)) FROM json_each(?3) j WHERE CASE j.key {different} ELSE 0 END)",
            body.as_object()
                .unwrap()
                .keys()
                .map(|k| format!("WHEN '{k}' THEN {k}"))
                .collect::<Vec<_>>()
                .join(" ")
        )
    };
    let mut event_params = params.clone();
    event_params.push(actor(r).into());
    let mut sets = body
        .as_object()
        .unwrap()
        .keys()
        .map(|k| format!("{k}=json_extract(?3,'$.{k}')"))
        .collect::<Vec<_>>();
    sets.push(format!("entered_at=CASE WHEN json_type(?3,'$.state') IS NOT NULL AND state IS NOT json_extract(?3,'$.state') THEN {NOW} ELSE entered_at END"));
    sets.push(format!("updated_at={NOW}"));
    sets.push("version=version+1".into());
    let mut result=r.0.batch(vec![
        Statement::new(format!("INSERT INTO pipeline_run_events(run_id,actor,from_state,to_state,detail) SELECT id,?4,state,coalesce(json_extract(?3,'$.state'),state),{detail} FROM pipeline_runs WHERE {guard}"),event_params),
        Statement::new(format!("UPDATE pipeline_runs SET {} WHERE {guard} RETURNING *",sets.join(",")),params),
        Statement::new("SELECT * FROM pipeline_runs WHERE id=?",vec![id.into()]),
    ]).await?;
    if let Some(run) = result[1].pop() {
        return Ok(reply(200, run));
    }
    let current = result[2].pop().ok_or(Error::NotFound)?;
    if current["version"] != version {
        return Ok(reply(
            409,
            json!({"error":{"code":"version_conflict","message":"Run version changed"},"current":current}),
        ));
    }
    Err(Failure(
        409,
        "terminal_run",
        "Terminal runs cannot change state; enqueue a new run with PUT".into(),
    ))
}
async fn list<S: Store>(store: &S, q: &[(String, String)]) -> Result<Reply> {
    let mut seen = std::collections::HashSet::new();
    let mut states = vec![];
    let mut clauses = vec![];
    let mut params = vec![];
    let mut limit = 50;
    for (k, v) in q {
        if k != "state" && !seen.insert(k) {
            return Err(bad("Duplicate run filter"));
        }
        match k.as_str() {
            "state" => {
                for s in v.split(',') {
                    states.push(State::parse(s).map_err(bad)?.as_str());
                }
            }
            "repo" | "key" => {
                clauses.push(format!("{k}=?"));
                params.push(v.clone().into());
            }
            "active" => clauses.push(match v.as_str() {
                "true" => ACTIVE.into(),
                "false" => format!("NOT ({ACTIVE})"),
                _ => return Err(bad("active must be true or false")),
            }),
            "due" if v == "true" => {
                clauses.push(ACTIVE.into());
                clauses.push(format!("(next_check_at IS NULL OR next_check_at<={NOW})"));
            }
            "limit" => {
                limit = positive_id(v).map_err(bad)?;
                if limit > 100 {
                    return Err(bad("limit must be 1 through 100"));
                }
            }
            _ => return Err(bad("Unknown or invalid run filter")),
        }
    }
    states.sort_unstable();
    states.dedup();
    if !states.is_empty() {
        clauses.push(format!("state IN ({})", vec!["?"; states.len()].join(",")));
        params.extend(states.into_iter().map(Value::from));
    }
    let predicate = if clauses.is_empty() {
        "1".into()
    } else {
        clauses.join(" AND ")
    };
    params.push(limit.into());
    let rows: Vec<Value> = store
        .query(
            &format!("SELECT * FROM pipeline_runs WHERE {predicate} ORDER BY id LIMIT ?"),
            params,
        )
        .await?;
    Ok(reply(200, json!({"data":rows})))
}
