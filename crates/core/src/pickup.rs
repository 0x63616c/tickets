//! Pure pickup policy. SQL adapters must match this eligibility and total order.
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub skip_blocked: bool,
    pub skip_open_pr: bool,
    pub blockers_first: bool,
    pub priority_first: bool,
    pub excluded_labels: Vec<String>,
    pub concurrency_cap: u32,
    pub lease_seconds: u32,
}
impl Default for Rules {
    fn default() -> Self {
        Self {
            skip_blocked: true,
            skip_open_pr: true,
            blockers_first: true,
            priority_first: true,
            excluded_labels: vec!["waiting-on-calum".into()],
            concurrency_cap: 3,
            lease_seconds: 1800,
        }
    }
}
impl Rules {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=100).contains(&self.concurrency_cap) {
            return Err("Concurrency cap must be 1 through 100".into());
        }
        if !(60..=86400).contains(&self.lease_seconds) {
            return Err("Lease must be 60 through 86400 seconds".into());
        }
        self.validate_labels()
    }
    pub fn validate_labels(&self) -> Result<(), String> {
        if self.excluded_labels.len() > 20 {
            return Err("Use at most 20 excluded labels".into());
        }
        let mut seen = std::collections::HashSet::new();
        for label in &self.excluded_labels {
            if label.trim() != label || label.is_empty() || label.chars().any(char::is_control) {
                return Err(
                    "Use label names without surrounding spaces or control characters".into(),
                );
            }
            if label.chars().count() > 100 {
                return Err("Each label name can have at most 100 characters".into());
            }
            if !seen.insert(label.to_lowercase()) {
                return Err(format!("Remove the duplicate label ‘{label}’"));
            }
        }
        Ok(())
    }
    pub fn parse(value: serde_json::Value) -> Result<Self, String> {
        let rules: Self = serde_json::from_value(value).map_err(|e| e.to_string())?;
        rules.validate()?;
        Ok(rules)
    }
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub id: i64,
    pub status: String,
    pub archived: bool,
    pub has_claim: bool,
    pub blocked: bool,
    pub open_pr: bool,
    pub unblocks: Vec<String>,
    pub priority: u8,
    pub created_at: String,
    pub labels: Vec<String>,
    pub project_in_flight: u32,
}
pub fn eligible(c: &Candidate, r: &Rules) -> bool {
    c.status == "todo"
        && !c.archived
        && !c.has_claim
        && (!r.skip_blocked || !c.blocked)
        && (!r.skip_open_pr || !c.open_pr)
        && c.project_in_flight < r.concurrency_cap
        && !c.labels.iter().any(|label| {
            r.excluded_labels
                .iter()
                .any(|excluded| label.eq_ignore_ascii_case(excluded))
        })
}
pub fn priority_rank(priority: u8) -> u8 {
    if priority == 0 { 5 } else { priority }
}
/// SQL projection of the same pure rank tuple. The adapter supplies candidate
/// columns `rules`, `unblocks`, `priority`, `created_at`, and `id`.
pub const ORDER_SQL: &str = "(json_extract(rules,'$.blockers_first') AND unblocks) DESC,
 CASE WHEN json_extract(rules,'$.priority_first') THEN CASE priority WHEN 0 THEN 5 ELSE priority END ELSE 0 END,
 created_at,id";

pub fn compare(a: &Candidate, ar: &Rules, b: &Candidate, br: &Rules) -> Ordering {
    (
        !(ar.blockers_first && !a.unblocks.is_empty()),
        if ar.priority_first {
            priority_rank(a.priority)
        } else {
            0
        },
        &a.created_at,
        a.id,
    )
        .cmp(&(
            !(br.blockers_first && !b.unblocks.is_empty()),
            if br.priority_first {
                priority_rank(b.priority)
            } else {
                0
            },
            &b.created_at,
            b.id,
        ))
}
pub fn reason(priority: u8, unblocks: &[String], r: &Rules) -> String {
    let mut reasons = vec![];
    if r.priority_first {
        reasons.push(
            match priority {
                1 => "Urgent priority",
                2 => "High priority",
                3 => "Medium priority",
                4 => "Low priority",
                _ => "No priority",
            }
            .to_string(),
        );
    }
    reasons.push("oldest eligible Todo in its ranking group".into());
    if !unblocks.is_empty() {
        reasons.push(format!("unblocks {}", unblocks.join(", ")));
    }
    reasons.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn candidate() -> Candidate {
        Candidate {
            id: 1,
            status: "todo".into(),
            archived: false,
            has_claim: false,
            blocked: false,
            open_pr: false,
            unblocks: vec![],
            priority: 0,
            created_at: "2026-01-01".into(),
            labels: vec![],
            project_in_flight: 0,
        }
    }
    #[test]
    fn every_exclusion_and_opt_out() {
        let r = Rules::default();
        assert!(eligible(&candidate(), &r));
        for status in ["backlog", "in_progress", "in_review", "done", "canceled"] {
            let mut c = candidate();
            c.status = status.into();
            assert!(!eligible(&c, &r));
        }
        for change in [
            |c: &mut Candidate| c.archived = true,
            |c: &mut Candidate| c.has_claim = true,
            |c: &mut Candidate| c.blocked = true,
            |c: &mut Candidate| c.open_pr = true,
            |c: &mut Candidate| c.project_in_flight = 3,
            |c: &mut Candidate| c.labels.push("Waiting-On-Calum".into()),
        ] {
            let mut c = candidate();
            change(&mut c);
            assert!(!eligible(&c, &r));
        }
        let mut c = candidate();
        c.blocked = true;
        c.open_pr = true;
        c.labels.push("waiting-on-calum".into());
        assert!(eligible(
            &c,
            &Rules {
                skip_blocked: false,
                skip_open_pr: false,
                excluded_labels: vec![],
                ..r
            }
        ));
    }
    #[test]
    fn total_order_and_preferences() {
        let r = Rules::default();
        let mut a = candidate();
        let mut b = candidate();
        b.id = 2;
        assert_eq!(compare(&a, &r, &b, &r), Ordering::Less);
        b.created_at = "2025-01-01".into();
        assert_eq!(compare(&a, &r, &b, &r), Ordering::Greater);
        for (higher, lower) in [(1, 2), (2, 3), (3, 4), (4, 0)] {
            a.priority = higher;
            b.priority = lower;
            assert_eq!(compare(&a, &r, &b, &r), Ordering::Less);
        }
        a.priority = 0;
        b.priority = 1;
        a.unblocks.push("TIK-50".into());
        assert_eq!(compare(&a, &r, &b, &r), Ordering::Less);
        let off = Rules {
            blockers_first: false,
            priority_first: false,
            ..r
        };
        assert_eq!(compare(&a, &off, &b, &off), Ordering::Greater);
    }
    #[test]
    fn validate_rules() {
        let r = Rules::default();
        assert!(r.validate().is_ok());
        for cap in [0, 101] {
            assert!(
                Rules {
                    concurrency_cap: cap,
                    ..r.clone()
                }
                .validate()
                .is_err()
            );
        }
        for seconds in [0, 59, 86401] {
            assert!(
                Rules {
                    lease_seconds: seconds,
                    ..r.clone()
                }
                .validate()
                .is_err()
            );
        }
        for labels in [
            vec![" x".into()],
            vec!["x".into(), "X".into()],
            vec!["".into()],
        ] {
            assert!(
                Rules {
                    excluded_labels: labels,
                    ..r.clone()
                }
                .validate()
                .is_err()
            );
        }
        let mut value = serde_json::to_value(r).unwrap();
        value["status"] = serde_json::json!("backlog");
        assert!(Rules::parse(value).is_err());
    }
}
