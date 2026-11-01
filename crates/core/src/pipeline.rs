//! Durable coding pipeline vocabulary, independent of Worker transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Queued,
    Claimed,
    Implementing,
    PrOpen,
    Checks,
    Fixing,
    Syncing,
    Merging,
    AwaitingReview,
    Stuck,
    WaitingOnCalum,
    Done,
    Canceled,
}
impl State {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "queued" => Ok(Self::Queued),
            "claimed" => Ok(Self::Claimed),
            "implementing" => Ok(Self::Implementing),
            "pr_open" => Ok(Self::PrOpen),
            "checks" => Ok(Self::Checks),
            "fixing" => Ok(Self::Fixing),
            "syncing" => Ok(Self::Syncing),
            "merging" => Ok(Self::Merging),
            "awaiting_review" => Ok(Self::AwaitingReview),
            "stuck" => Ok(Self::Stuck),
            "waiting_on_calum" => Ok(Self::WaitingOnCalum),
            "done" => Ok(Self::Done),
            "canceled" => Ok(Self::Canceled),
            _ => Err(format!("Unknown state: {value}")),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Claimed => "claimed",
            Self::Implementing => "implementing",
            Self::PrOpen => "pr_open",
            Self::Checks => "checks",
            Self::Fixing => "fixing",
            Self::Syncing => "syncing",
            Self::Merging => "merging",
            Self::AwaitingReview => "awaiting_review",
            Self::Stuck => "stuck",
            Self::WaitingOnCalum => "waiting_on_calum",
            Self::Done => "done",
            Self::Canceled => "canceled",
        }
    }
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::AwaitingReview | Self::Done | Self::Canceled)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    Merge,
    Draft,
    NoMerge,
}
impl Intent {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "merge" => Ok(Self::Merge),
            "draft" => Ok(Self::Draft),
            "no_merge" => Ok(Self::NoMerge),
            _ => Err(format!("Unknown intent: {value}")),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::Draft => "draft",
            Self::NoMerge => "no_merge",
        }
    }
}
/// Reject unknown mutation fields and validate run values before SQL binding.
pub fn validate_document(body: &serde_json::Value, allowed: &[&str]) -> Result<(), String> {
    let fields = body
        .as_object()
        .ok_or_else(|| "Expected an object".to_string())?;
    if fields.keys().any(|k| !allowed.contains(&k.as_str())) {
        return Err("Unknown run field".into());
    }
    for (key, value) in fields {
        match key.as_str() {
            "state" => {
                State::parse(
                    value
                        .as_str()
                        .ok_or_else(|| "state must be a string".to_string())?,
                )?;
            }
            "intent" => {
                Intent::parse(
                    value
                        .as_str()
                        .ok_or_else(|| "intent must be a string".to_string())?,
                )?;
            }
            "expected_version" | "attempts" | "pr" => {
                if key == "pr" && value.is_null() {
                    continue;
                }
                let n = value
                    .as_i64()
                    .ok_or_else(|| format!("{key} must be an integer"))?;
                if n < if key == "attempts" { 0 } else { 1 } {
                    return Err(format!("Invalid {key}"));
                }
            }
            _ => {
                if !value.is_null() && !value.is_string() {
                    return Err(format!("{key} must be a string or null"));
                }
                if matches!(key.as_str(), "next_check_at" | "escalated_at") && !value.is_null() {
                    validate_timestamp(value.as_str().unwrap())?;
                }
            }
        }
    }
    Ok(())
}
// UTC millisecond timestamps are lexically ordered by the due selector.
fn validate_timestamp(s: &str) -> std::result::Result<(), String> {
    let b = s.as_bytes();
    if b.len() != 24
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'.'
        || b[23] != b'Z'
        || b.iter()
            .enumerate()
            .any(|(i, c)| ![4, 7, 10, 13, 16, 19, 23].contains(&i) && !c.is_ascii_digit())
    {
        return Err("Expected UTC timestamp YYYY-MM-DDTHH:MM:SS.sssZ".into());
    }
    let year: u32 = s[..4].parse().unwrap();
    let month: usize = s[5..7].parse().unwrap();
    let day: u32 = s[8..10].parse().unwrap();
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if !(1..=12).contains(&month)
        || day == 0
        || day > days[month - 1]
        || s[11..13].parse::<u32>().unwrap() > 23
        || s[14..16].parse::<u32>().unwrap() > 59
        || s[17..19].parse::<u32>().unwrap() > 59
    {
        return Err("Invalid UTC timestamp".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vocabulary() {
        for (name, terminal) in [
            ("queued", false),
            ("claimed", false),
            ("implementing", false),
            ("pr_open", false),
            ("checks", false),
            ("fixing", false),
            ("syncing", false),
            ("merging", false),
            ("awaiting_review", true),
            ("stuck", false),
            ("waiting_on_calum", false),
            ("done", true),
            ("canceled", true),
        ] {
            let state = State::parse(name).unwrap();
            assert_eq!(state.as_str(), name);
            assert_eq!(state.is_terminal(), terminal);
        }
        for name in ["merge", "draft", "no_merge"] {
            assert_eq!(Intent::parse(name).unwrap().as_str(), name);
        }
        for unknown in ["", "unknown", "Queued", "MERGE"] {
            assert!(State::parse(unknown).is_err());
            assert!(Intent::parse(unknown).is_err());
        }
    }
}
