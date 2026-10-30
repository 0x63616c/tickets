//! D1 adapter for the core pickup policy. All claim writes share TIK-60 history.
use crate::{
    api_repository::{ApiRepository, TICKET_SELECT},
    mutation::{self, Scope},
    repository::{Error, Result, Store},
};
use serde_json::{Value, json};
use tickets_core::{history::Actor, pickup::Rules};

const NOW: &str = "coalesce((SELECT now FROM ticket_mutation_clock WHERE context_id=1),strftime('%Y-%m-%dT%H:%M:%fZ','now'))";
const EFFECTIVE: &str = "coalesce((SELECT rules FROM pickup_rules WHERE scope_id=tickets.project_id),(SELECT rules FROM pickup_rules WHERE scope_id=0))";
const UNBLOCKS: &str = "(SELECT json_group_array(key) FROM (SELECT p.key_prefix||'-'||b.number key FROM ticket_dependencies d JOIN tickets b ON b.id=d.blocked_id JOIN projects p ON p.id=b.project_id WHERE d.blocker_id=tickets.id AND b.status NOT IN ('done','canceled') ORDER BY b.id))";

/// Same selector for preview and locked claim, with one optional project parameter.
fn selector() -> String {
    let order = tickets_core::pickup::ORDER_SQL;
    format!("SELECT id FROM (SELECT tickets.*,p.archived,{EFFECTIVE} rules,
 EXISTS(SELECT 1 FROM ticket_dependencies d JOIN tickets b ON b.id=d.blocker_id WHERE d.blocked_id=tickets.id AND b.status NOT IN ('done','canceled')) blocked,
 EXISTS(SELECT 1 FROM ticket_pull_requests pr WHERE pr.ticket_id=tickets.id AND pr.state IN ('open','draft')) open_pr,
 EXISTS(SELECT 1 FROM ticket_dependencies d JOIN tickets b ON b.id=d.blocked_id WHERE d.blocker_id=tickets.id AND b.status NOT IN ('done','canceled')) unblocks,
 (SELECT count(*) FROM tickets active WHERE active.project_id=tickets.project_id AND (active.status='in_progress' OR (active.claimed_by IS NOT NULL AND active.claim_expires_at>{NOW}))) in_flight
 FROM tickets JOIN projects p ON p.id=tickets.project_id) c
 WHERE (?1 IS NULL OR project_id=?1) AND archived=0 AND status='todo' AND claimed_by IS NULL
 AND (NOT json_extract(rules,'$.skip_blocked') OR NOT blocked)
 AND (NOT json_extract(rules,'$.skip_open_pr') OR NOT open_pr)
 AND in_flight<json_extract(rules,'$.concurrency_cap')
 AND NOT EXISTS(SELECT 1 FROM ticket_labels tl JOIN labels l ON l.id=tl.label_id JOIN json_each(c.rules,'$.excluded_labels') e ON l.name=e.value COLLATE NOCASE WHERE tl.ticket_id=c.id)
 ORDER BY {order} LIMIT 1")
}
fn returning() -> String {
    format!(
        "{}, {EFFECTIVE} pickup_rules, {UNBLOCKS} pickup_unblocks",
        TICKET_SELECT.replacen("tickets.*", "*", 1)
    )
}
fn owner(actor: &Actor) -> String {
    format!(
        "{}:{}",
        serde_json::to_value(&actor.kind).unwrap().as_str().unwrap(),
        actor.id
    )
}
async fn picked<S: Store>(r: &ApiRepository<'_, S>, mut ticket: Value) -> Result<Value> {
    let rules: Rules = serde_json::from_str(ticket["pickup_rules"].as_str().unwrap())
        .map_err(|e| Error::Storage(e.to_string()))?;
    let unblocks: Vec<String> = serde_json::from_str(ticket["pickup_unblocks"].as_str().unwrap())
        .map_err(|e| Error::Storage(e.to_string()))?;
    let reason = tickets_core::pickup::reason(
        ticket["priority"].as_u64().unwrap() as u8,
        &unblocks,
        &rules,
    );
    ticket.as_object_mut().unwrap().remove("pickup_rules");
    ticket.as_object_mut().unwrap().remove("pickup_unblocks");
    Ok(json!({"ticket":r.enrich("tickets",ticket).await?,"reason":reason}))
}
pub(crate) async fn next<S: Store>(
    r: &ApiRepository<'_, S>,
    project: Option<i64>,
    claim: bool,
) -> Result<Option<Value>> {
    let select = selector();
    let rows: Vec<Value> = if claim {
        let sql=format!("UPDATE tickets SET claimed_by=?2,claim_id=lower(hex(randomblob(16))),
 claim_expires_at=strftime('%Y-%m-%dT%H:%M:%fZ',{NOW},'+'||json_extract({EFFECTIVE},'$.lease_seconds')||' seconds'),
 claim_heartbeat_at={NOW},claim_event='claim',claim_version=claim_version+1,status='in_progress',updated_at={NOW}
 WHERE id IN (SELECT ticket_id FROM ticket_mutation_before) AND claimed_by IS NULL AND id=({select}) RETURNING {}",returning());
        mutation::execute(
            r.0,
            r.1,
            Scope::selection(select, vec![json!(project)]),
            &sql,
            vec![json!(project), owner(r.1).into()],
        )
        .await?
    } else {
        r.0.query(
            &format!(
                "SELECT {} FROM tickets WHERE id=({select})",
                returning().replacen("*,", "tickets.*,", 1)
            ),
            vec![json!(project)],
        )
        .await?
    };
    match rows.into_iter().next() {
        Some(v) => Ok(Some(picked(r, v).await?)),
        None => Ok(None),
    }
}
pub(crate) async fn lease<S: Store>(
    r: &ApiRepository<'_, S>,
    id: i64,
    claim_id: &str,
    heartbeat: bool,
) -> Result<Option<Value>> {
    let set = if heartbeat {
        format!(
            "claim_expires_at=max(claim_expires_at,strftime('%Y-%m-%dT%H:%M:%fZ',{NOW},'+'||json_extract({EFFECTIVE},'$.lease_seconds')||' seconds')),claim_heartbeat_at={NOW},claim_event='heartbeat'"
        )
    } else {
        "claimed_by=NULL,claim_id=NULL,claim_expires_at=NULL,claim_heartbeat_at=NULL,claim_event='release',status=CASE WHEN status='in_progress' THEN 'todo' ELSE status END".into()
    };
    let sql = format!(
        "UPDATE tickets SET {set},claim_version=claim_version+1,updated_at={NOW} WHERE id=?1 AND claimed_by=?2 AND claim_id=?3 AND claim_expires_at>{NOW} RETURNING {}",
        TICKET_SELECT.replacen("tickets.*", "*", 1)
    );
    let rows: Vec<Value> = mutation::execute(
        r.0,
        r.1,
        Scope::ticket(id),
        &sql,
        vec![id.into(), owner(r.1).into(), claim_id.into()],
    )
    .await?;
    match rows.into_iter().next() {
        Some(v) => Ok(Some(r.enrich("tickets", v).await?)),
        None => Ok(None),
    }
}
/// Bounded cron work, using the same atomic snapshot/diff path as HTTP mutations.
pub async fn sweep<S: Store>(store: &S) -> Result<Vec<Value>> {
    let select = format!(
        "SELECT id FROM tickets WHERE claimed_by IS NOT NULL AND claim_expires_at<={NOW} ORDER BY claim_expires_at,id LIMIT 25"
    );
    let sql = format!(
        "UPDATE tickets SET claimed_by=NULL,claim_id=NULL,claim_expires_at=NULL,claim_heartbeat_at=NULL,claim_event='expiry',claim_version=claim_version+1,status=CASE WHEN status='in_progress' THEN 'todo' ELSE status END,updated_at={NOW} WHERE id IN (SELECT ticket_id FROM ticket_mutation_before) AND claimed_by IS NOT NULL AND claim_expires_at<={NOW} RETURNING id,project_id"
    );
    mutation::execute(
        store,
        &Actor::system(),
        Scope::selection(select, vec![]),
        &sql,
        vec![],
    )
    .await
}
pub(crate) async fn rules<S: Store>(
    store: &S,
    project: Option<i64>,
    method: &str,
    body: Value,
) -> Result<Value> {
    if let Some(id) = project {
        let rows: Vec<Value> = store
            .query("SELECT id FROM projects WHERE id=?", vec![id.into()])
            .await?;
        if rows.is_empty() {
            return Err(Error::NotFound);
        }
    }
    let scope = project.unwrap_or(0);
    if method == "PUT" {
        let rules = Rules::parse(body).map_err(Error::Validation)?;
        let _:Vec<Value>=store.query("INSERT INTO pickup_rules(scope_id,rules) VALUES(?,?) ON CONFLICT(scope_id) DO UPDATE SET rules=excluded.rules,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')",vec![scope.into(),serde_json::to_string(&rules).unwrap().into()]).await?;
    } else if method == "DELETE" {
        if project.is_none() {
            return Err(Error::Validation(
                "Workspace rules cannot be removed".into(),
            ));
        }
        let _: Vec<Value> = store
            .query(
                "DELETE FROM pickup_rules WHERE scope_id=?",
                vec![scope.into()],
            )
            .await?;
    }
    let rows:Vec<Value>=store.query("SELECT rules,scope_id FROM pickup_rules WHERE scope_id IN (0,?) ORDER BY scope_id DESC LIMIT 1",vec![scope.into()]).await?;
    let row = rows
        .into_iter()
        .next()
        .ok_or_else(|| Error::Storage("Missing workspace rules".into()))?;
    Ok(
        json!({"rules":serde_json::from_str::<Value>(row["rules"].as_str().unwrap()).map_err(|e|Error::Storage(e.to_string()))?,"source":if row["scope_id"]==0 {"workspace"} else {"project"},"project_id":project}),
    )
}
