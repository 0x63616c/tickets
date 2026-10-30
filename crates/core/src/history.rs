//! Ticket history policy, independent of D1 and HTTP. JSON snapshots are compared
//! inside the mutation transaction; never compute old values from an earlier read.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    User,
    ApiToken,
    Webhook,
    System,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Actor {
    pub kind: ActorKind,
    pub id: String,
    pub label: String,
}
impl Actor {
    pub fn user(id: i64) -> Self {
        Self {
            kind: ActorKind::User,
            id: id.to_string(),
            label: "Owner".into(),
        }
    }
    pub fn api_token(id: i64) -> Self {
        Self {
            kind: ActorKind::ApiToken,
            id: id.to_string(),
            label: format!("API token #{id}"),
        }
    }
    pub fn webhook() -> Self {
        Self {
            kind: ActorKind::Webhook,
            id: "github".into(),
            label: "GitHub".into(),
        }
    }
    pub fn system() -> Self {
        Self {
            kind: ActorKind::System,
            id: "repository".into(),
            label: "System".into(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id: i64,
    pub subject_id: i64,
    pub ticket_id: i64,
    pub actor_kind: ActorKind,
    pub actor_id: String,
    pub actor_label: String,
    pub field: String,
    pub old_value: Value,
    pub new_value: Value,
    pub recorded_at: String,
}
// Field name, snapshot property. Project values retain their display metadata,
// but only a changed project ID constitutes a move.
pub const SCALARS: &[(&str, &str)] = &[
    ("status", "status"),
    ("priority", "priority"),
    ("assignee", "assignee"),
    ("title", "title"),
    ("description", "body"),
    ("project", "project"),
];
pub const SETS: &[(&str, &str)] = &[
    ("label", "labels"),
    ("blocker", "blockers"),
    ("blocks", "blocks"),
];
pub const CLAIM_FIELDS: &[&str] = &["claim", "heartbeat", "expiry", "release"];

/// SQL diff over `pairs(subject_id,ticket_id,old,new)`. JSON values are encoded
/// for storage, including strings; absence is SQL NULL. Only allowlisted paths
/// are interpolated. Runtime values are always bound by the adapter.
pub fn diff_sql() -> String {
    let mut queries = vec![
        "SELECT subject_id,ticket_id,'created' field,NULL old_value,new new_value FROM pairs WHERE old IS NULL".into(),
        "SELECT subject_id,ticket_id,'deleted',old,NULL FROM pairs WHERE new IS NULL".into(),
    ];
    // D1 limits compound SELECTs to five terms. Drive the allowlisted fields
    // through JSON tables instead of adding one UNION branch for every field.
    let scalars = serde_json::to_string(
        &SCALARS
            .iter()
            .copied()
            .collect::<std::collections::BTreeMap<_, _>>(),
    )
    .expect("static scalar fields");
    queries.push(format!("SELECT subject_id,ticket_id,f.key,json_quote(json_extract(old,'$.'||f.value)),json_quote(json_extract(new,'$.'||f.value)) FROM pairs,json_each('{scalars}') f WHERE old IS NOT NULL AND new IS NOT NULL AND json_extract(old,'$.'||CASE WHEN f.key='project' THEN 'project.id' ELSE f.value END) IS NOT json_extract(new,'$.'||CASE WHEN f.key='project' THEN 'project.id' ELSE f.value END)"));
    let sets = serde_json::to_string(
        &SETS
            .iter()
            .copied()
            .collect::<std::collections::BTreeMap<_, _>>(),
    )
    .expect("static set fields");
    for (suffix, source, other, old, new) in [
        ("added", "new", "old", "NULL", "j.value"),
        ("removed", "old", "new", "j.value", "NULL"),
    ] {
        queries.push(format!("SELECT subject_id,ticket_id,f.key||'_{suffix}',{old},{new} FROM pairs,json_each('{sets}') f,json_each({source},'$.'||f.value) j WHERE pairs.old IS NOT NULL AND pairs.new IS NOT NULL AND NOT EXISTS(SELECT 1 FROM json_each({other},'$.'||f.value) k WHERE json_extract(k.value,'$.id')=json_extract(j.value,'$.id'))"));
    }
    queries.join(" UNION ALL ")
}

/// Reference implementation used to verify the transactional SQL against domain
/// semantics (including null clears and set membership independent of order).
pub fn diff(old: Option<&Value>, new: Option<&Value>) -> Vec<Value> {
    let (old, new) = match (old, new) {
        (None, Some(new)) => {
            return vec![json!({"field":"created","old_value":null,"new_value":new})];
        }
        (Some(old), None) => {
            return vec![json!({"field":"deleted","old_value":old,"new_value":null})];
        }
        (Some(old), Some(new)) => (old, new),
        (None, None) => return vec![],
    };
    let mut changes = vec![];
    for (field, property) in SCALARS {
        let changed = if *field == "project" {
            old[property]["id"] != new[property]["id"]
        } else {
            old[property] != new[property]
        };
        if changed {
            changes
                .push(json!({"field":field,"old_value":old[property],"new_value":new[property]}));
        }
    }
    for (field, property) in SETS {
        for (suffix, source, other, added) in
            [("added", new, old, true), ("removed", old, new, false)]
        {
            for value in source[property].as_array().into_iter().flatten() {
                if !other[property]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|v| v["id"] == value["id"])
                {
                    changes.push(json!({"field":format!("{field}_{suffix}"),"old_value":if added {Value::Null} else {value.clone()},"new_value":if added {value.clone()} else {Value::Null}}));
                }
            }
        }
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_scalar_and_null_clear() {
        for (field, property) in SCALARS {
            let a = if *field == "project" {
                json!({(*property):{"id":1,"name":"One"}})
            } else {
                json!({(*property):"old"})
            };
            let b = if *field == "project" {
                json!({(*property):{"id":2,"name":"Two"}})
            } else {
                json!({(*property):null})
            };
            let changes = diff(Some(&a), Some(&b));
            assert_eq!(changes.len(), 1);
            assert_eq!(changes[0]["field"], *field);
            assert!(diff(Some(&a), Some(&a)).is_empty());
        }
    }
    #[test]
    fn sets_creation_deletion_and_claim_contract() {
        for (field, property) in SETS {
            let a = json!({(*property):[{"id":1,"name":"A"}]});
            let b = json!({(*property):[{"id":2,"name":"B"}]});
            let changes = diff(Some(&a), Some(&b));
            assert_eq!(changes.len(), 2);
            assert_eq!(changes[0]["field"], format!("{field}_added"));
            assert_eq!(changes[1]["field"], format!("{field}_removed"));
            assert_eq!(diff(None, Some(&a)).len(), 1);
            assert_eq!(diff(Some(&a), None)[0]["field"], "deleted");
        }
        assert_eq!(CLAIM_FIELDS, ["claim", "heartbeat", "expiry", "release"]);
    }
}
