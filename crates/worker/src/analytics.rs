//! A single aggregate statement supplies a coherent snapshot (one D1 read + auth).
use crate::repository::{Error, Result, Store};
use serde_json::Value;
use tickets_core::api::positive_id;

pub async fn home<S: Store>(store: &S, query: &[(String, String)]) -> Result<Value> {
    let mut project = 0_i64;
    let mut metric = String::new();
    let mut week = 7_i64;
    let mut status = String::new();
    let mut cursor = 0_i64;
    let mut seen = std::collections::BTreeSet::new();
    for (key, value) in query {
        if !seen.insert(key) {
            return Err(Error::Validation("Duplicate analytics parameter".into()));
        }
        match key.as_str() {
            "project" => project = positive_id(value).map_err(Error::Validation)?,
            "cursor" => cursor = positive_id(value).map_err(Error::Validation)?,
            "metric"
                if [
                    "in_flight",
                    "review",
                    "blocked",
                    "created",
                    "completed",
                    "inventory",
                ]
                .contains(&value.as_str()) =>
            {
                metric = value.clone()
            }
            "week" if value.len() == 1 && matches!(value.as_bytes()[0], b'0'..=b'7') => {
                week = value.parse().unwrap()
            }
            "status"
                if ["backlog", "todo", "in_progress", "in_review"].contains(&value.as_str()) =>
            {
                status = value.clone()
            }
            _ => return Err(Error::Validation("Invalid analytics parameter".into())),
        }
    }
    if (seen.contains(&"week".to_string()) && !["created", "completed"].contains(&metric.as_str()))
        || (!status.is_empty() && metric != "inventory")
        || (cursor > 0 && metric.is_empty())
    {
        return Err(Error::Validation(
            "Parameter does not apply to this metric".into(),
        ));
    }
    let rows: Vec<Value> = store
        .query(
            include_str!("analytics.sql"),
            vec![
                project.into(),
                metric.into(),
                week.into(),
                status.into(),
                cursor.into(),
            ],
        )
        .await?;
    let row = rows
        .first()
        .ok_or_else(|| Error::Storage("Missing analytics response".into()))?;
    if row["scope_valid"] != 1 {
        return Err(Error::Validation("Choose an active project".into()));
    }
    serde_json::from_str(row["home"].as_str().unwrap_or(""))
        .map_err(|e| Error::Storage(e.to_string()))
}
