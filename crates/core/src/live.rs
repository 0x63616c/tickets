//! Small invalidations; D1 remains authoritative after missed or duplicate events.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeEvent {
    pub project_id: i64,
    pub entity: String,
    pub id: i64,
    pub action: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn event_wire_format() {
        let event = ChangeEvent {
            project_id: 2,
            entity: "tickets".into(),
            id: 7,
            action: "update".into(),
        };
        let json = r#"{"project_id":2,"entity":"tickets","id":7,"action":"update"}"#;
        assert_eq!(serde_json::to_string(&event).unwrap(), json);
        assert_eq!(serde_json::from_str::<ChangeEvent>(json).unwrap(), event);
        assert!(serde_json::from_str::<ChangeEvent>(r#"{"project_id":2}"#).is_err());
    }
}
