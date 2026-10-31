//! Single entry point for every ticket-affecting write, including trigger/cascade
//! effects. Scratch data and append-only history commit with the domain change.
use crate::repository::{Result, Statement, Store};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tickets_core::history::Actor;

pub struct Scope {
    sql: String,
    params: Vec<Value>,
}
impl Scope {
    /// Selector is internal SQL only; evaluated after the batch obtains its write lock.
    pub(crate) fn selection(sql: String, params: Vec<Value>) -> Self {
        Self { sql, params }
    }

    /// Include neighboring tickets so deletion cascades are visible on survivors.
    pub fn ticket(id: i64) -> Self {
        Self { sql: "SELECT id FROM tickets WHERE id=? OR id IN (SELECT blocked_id FROM ticket_dependencies WHERE blocker_id=? UNION SELECT blocker_id FROM ticket_dependencies WHERE blocked_id=?)".into(), params:vec![id.into(),id.into(),id.into()] }
    }
    pub fn external(id: &str) -> Self {
        Self {
            sql: "SELECT id FROM tickets WHERE external_id=?".into(),
            params: vec![id.into()],
        }
    }
    pub fn create() -> Self {
        Self {
            sql: "SELECT id FROM tickets WHERE 0".into(),
            params: vec![],
        }
    }
    pub fn label(id: i64) -> Self {
        Self {
            sql: "SELECT ticket_id FROM ticket_labels WHERE label_id=?".into(),
            params: vec![id.into()],
        }
    }
    pub fn relation(ticket: i64, other: i64) -> Self {
        Self {
            sql: "SELECT id FROM tickets WHERE id IN (?,?)".into(),
            params: vec![ticket.into(), other.into()],
        }
    }
    pub fn pull_request(repo: &str, number: i64, links: &str) -> Self {
        Self { sql:"SELECT ticket_id FROM ticket_pull_requests WHERE repo=? AND number=? UNION SELECT t.id FROM json_each(?) j JOIN projects p ON p.key_prefix=json_extract(j.value,'$.prefix') JOIN tickets t ON t.project_id=p.id AND t.number=json_extract(j.value,'$.number')".into(), params:vec![repo.into(),number.into(),links.into()] }
    }
}
const SNAPSHOT: &str = "json_object('status',t.status,'priority',t.priority,'assignee',t.assignee,'title',t.title,'body',t.body,'type',t.type,'scope',t.scope,
 'claim',json_object('owner',t.claimed_by,'id',t.claim_id,'expires_at',t.claim_expires_at,'heartbeat_at',t.claim_heartbeat_at,'action',t.claim_event,'version',t.claim_version),
 'project',json_object('id',p.id,'name',p.name,'key',p.key_prefix),
 'labels',json((SELECT json_group_array(json(v)) FROM (SELECT json_object('id',l.id,'name',l.name,'color',l.color) v FROM labels l JOIN ticket_labels tl ON tl.label_id=l.id WHERE tl.ticket_id=t.id ORDER BY l.id))),
 'blockers',json((SELECT json_group_array(json(v)) FROM (SELECT json_object('id',b.id,'key',bp.key_prefix||'-'||b.number,'title',b.title) v FROM ticket_dependencies d JOIN tickets b ON b.id=d.blocker_id JOIN projects bp ON bp.id=b.project_id WHERE d.blocked_id=t.id ORDER BY b.id))),
 'blocks',json((SELECT json_group_array(json(v)) FROM (SELECT json_object('id',b.id,'key',bp.key_prefix||'-'||b.number,'title',b.title) v FROM ticket_dependencies d JOIN tickets b ON b.id=d.blocked_id JOIN projects bp ON bp.id=b.project_id WHERE d.blocker_id=t.id ORDER BY b.id))))";
const FROM: &str = "FROM tickets t JOIN ticket_subjects s ON s.ticket_id=t.id JOIN projects p ON p.id=t.project_id";

pub async fn execute<S: Store, T: DeserializeOwned>(
    store: &S,
    actor: &Actor,
    scope: Scope,
    sql: &str,
    params: Vec<Value>,
) -> Result<Vec<T>> {
    let before = format!(
        "INSERT INTO ticket_mutation_before SELECT s.id,t.id,{SNAPSHOT} {FROM} WHERE t.id IN ({})",
        scope.sql
    );
    let append = format!("WITH after AS (SELECT s.id subject_id,t.id ticket_id,{SNAPSHOT} snapshot {FROM} WHERE s.id IN (SELECT subject_id FROM ticket_mutation_before) OR s.id>(SELECT last_subject FROM ticket_mutation_context)),
 pairs AS (SELECT b.subject_id,b.ticket_id,b.snapshot old,a.snapshot new FROM ticket_mutation_before b LEFT JOIN after a ON a.subject_id=b.subject_id UNION ALL SELECT a.subject_id,a.ticket_id,NULL,a.snapshot FROM after a WHERE a.subject_id NOT IN (SELECT subject_id FROM ticket_mutation_before)),
 changes AS ({}) INSERT INTO ticket_history(subject_id,ticket_id,actor_kind,actor_id,actor_label,field,old_value,new_value) SELECT c.subject_id,c.ticket_id,x.actor_kind,x.actor_id,x.actor_label,c.field,c.old_value,c.new_value FROM changes c CROSS JOIN ticket_mutation_context x", tickets_core::history::diff_sql());
    let statements = vec![
        Statement::new(
            "INSERT INTO ticket_mutation_context SELECT 1,?,?,?,coalesce(max(id),0) FROM ticket_subjects",
            vec![json!(actor.kind), json!(actor.id), json!(actor.label)],
        ),
        Statement::new(before, scope.params),
        Statement::new(sql, params),
        Statement::new(append, vec![]),
        Statement::new("DELETE FROM ticket_mutation_before", vec![]),
        Statement::new("DELETE FROM ticket_mutation_context", vec![]),
    ];
    let mut results = store.batch(statements).await?;
    results
        .swap_remove(2)
        .into_iter()
        .map(|v| {
            serde_json::from_value(v).map_err(|e| crate::repository::Error::Storage(e.to_string()))
        })
        .collect()
}

/// Shared by signed webhook transport and host tests.
pub async fn pull_request<S: Store>(store: &S, values: Vec<Value>) -> Result<Vec<Value>> {
    let scope = Scope::pull_request(
        values[0].as_str().unwrap(),
        values[1].as_i64().unwrap(),
        values[7].as_str().unwrap(),
    );
    execute(
        store,
        &Actor::webhook(),
        scope,
        // The before snapshot includes both removed links and new matches under
        // the same write lock. RETURNING captures scope before scratch cleanup.
        &format!("{},(SELECT json_group_array(project_id) FROM (SELECT DISTINCT project_id FROM tickets WHERE id IN (SELECT ticket_id FROM ticket_mutation_before UNION SELECT blocked_id FROM ticket_dependencies WHERE blocker_id IN (SELECT ticket_id FROM ticket_mutation_before) UNION SELECT blocker_id FROM ticket_dependencies WHERE blocked_id IN (SELECT ticket_id FROM ticket_mutation_before)))) AS projects", tickets_core::pull_request::UPSERT),
        values,
    )
    .await
}
