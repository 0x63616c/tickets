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

/// Automatic notice reconciliation waits for a 5s batch window and respects a
/// 30s per-tab cooldown. Saturation handles a wall-clock adjustment backwards.
pub fn refresh_delay_ms(now: u64, last_reload: Option<u64>) -> u32 {
    last_reload.map_or(5_000, |last| {
        30_000_u64
            .saturating_sub(now.saturating_sub(last))
            .max(5_000) as u32
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notice_batches_wait_and_respect_the_tab_cooldown() {
        assert_eq!(refresh_delay_ms(0, None), 5_000);
        assert_eq!(refresh_delay_ms(6_000, Some(5_000)), 29_000);
        assert_eq!(refresh_delay_ms(30_000, Some(5_000)), 5_000);
        assert_eq!(refresh_delay_ms(35_000, Some(5_000)), 5_000);
        assert_eq!(refresh_delay_ms(100_000, Some(5_000)), 5_000);
        assert_eq!(refresh_delay_ms(4_000, Some(5_000)), 30_000);
    }

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
