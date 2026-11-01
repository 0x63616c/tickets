//! Mission Control v1: forward-compatible source snapshots and compact ticket queues.
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_REPORT_BYTES: usize = 512 * 1024;
pub const LIST_FIELDS: &[&str] = &[
    "jobs",
    "pull_requests",
    "merges",
    "builds",
    "blocked",
    "needs_calum",
];

pub fn validate(source: &str, raw: &str) -> Result<Value, String> {
    if source.is_empty()
        || source.len() > 40
        || !source
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("source must be 1–40 lowercase letters, digits or hyphens".into());
    }
    if raw.len() > MAX_REPORT_BYTES {
        return Err("report exceeds 512 KB".into());
    }
    let value: Value = serde_json::from_str(raw).map_err(|_| "report must be valid JSON")?;
    let object = value.as_object().ok_or("report must be a JSON object")?;
    for field in LIST_FIELDS {
        if let Some(list) = object.get(*field)
            && !list
                .as_array()
                .is_some_and(|a| a.iter().all(Value::is_object))
        {
            return Err(format!("{field} must be an array of objects"));
        }
    }
    Ok(value)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mission {
    pub generated_at: String,
    pub reports: Vec<Report>,
    pub waiting_on_calum: Vec<Ticket>,
    pub in_review: Vec<Ticket>,
    pub recently_done: Vec<Ticket>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub source: String,
    pub received_at: String,
    pub stale: bool,
    pub report: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Label {
    pub id: i64,
    pub name: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ticket {
    pub id: i64,
    pub key: String,
    pub title: String,
    pub r#type: Option<String>,
    pub scope: Option<String>,
    pub project: String,
    pub status: String,
    pub updated_at: String,
    pub labels: Vec<Label>,
    #[serde(default)]
    pub pull_requests: Vec<Value>,
}
