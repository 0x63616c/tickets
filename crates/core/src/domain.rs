use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Backlog,
    Todo,
    InProgress,
    InReview,
    Done,
    Canceled,
}

/// 0 = no priority, 1 = urgent, 2 = high, 3 = medium, 4 = low.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum Priority {
    None,
    Urgent,
    High,
    Medium,
    Low,
}
impl TryFrom<u8> for Priority {
    type Error = String;
    fn try_from(value: u8) -> Result<Self, String> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Urgent),
            2 => Ok(Self::High),
            3 => Ok(Self::Medium),
            4 => Ok(Self::Low),
            _ => Err("priority must be 0 through 4".into()),
        }
    }
}
impl From<Priority> for u8 {
    fn from(value: Priority) -> Self {
        value as u8
    }
}

pub fn validate_name(value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err("name must not be empty".into())
    } else {
        Ok(())
    }
}
pub fn validate_prefix(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 10 || !value.bytes().all(|c| c.is_ascii_uppercase()) {
        Err("prefix must contain 1–10 uppercase ASCII letters".into())
    } else {
        Ok(())
    }
}
pub fn validate_color(value: &str) -> Result<(), String> {
    if value.len() != 7
        || !value.starts_with('#')
        || !value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
    {
        Err("color must be #RRGGBB".into())
    } else {
        Ok(())
    }
}
pub fn ticket_key(prefix: &str, number: i64) -> Result<String, String> {
    validate_prefix(prefix)?;
    if number <= 0 {
        return Err("ticket number must be positive".into());
    }
    Ok(format!("{prefix}-{number}"))
}
/// Named project glyphs share the web SVG family; other values are short emoji sequences.
pub const PROJECT_GLYPHS: &[&str] = &[
    "project", "ticket", "board", "grid", "code", "globe", "box", "bolt", "star", "layers",
];
pub fn validate_project_icon(icon: &str) -> Result<(), String> {
    if PROJECT_GLYPHS.contains(&icon) {
        return Ok(());
    }
    // Bounded Unicode sequences allow joined emoji, variation selectors and skin tones.
    // Reject text, whitespace, controls and markup; render the value as escaped text.
    if icon.is_empty() || icon.chars().count() > 16 || !icon.chars().any(|c| matches!(c as u32, 0x1f000..=0x1faff | 0x2600..=0x27bf))
        || icon.chars().any(|c| !matches!(c as u32, 0x1f000..=0x1faff | 0x2600..=0x27bf | 0xfe0f | 0x200d | 0xe0020..=0xe007f)) {
        return Err("icon must be a supported glyph or a short emoji sequence".into());
    }
    Ok(())
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    #[serde(default)]
    pub open_ticket_count: u64,
    #[serde(default)]
    pub done_ticket_count: u64,
    #[serde(default)]
    pub last_activity_at: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    pub id: i64,
    pub name: String,
    pub key_prefix: String,
    pub color: String,
    #[serde(deserialize_with = "deserialize_bool")]
    pub archived: bool,
    pub github_owner: Option<String>,
    pub github_project_kind: String,
    pub github_project_number: Option<i64>,
    pub github_repo: Option<String>,
    pub next_ticket_number: i64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ticket {
    pub id: i64,
    pub project_id: i64,
    pub number: i64,
    pub title: String,
    pub r#type: Option<String>,
    pub scope: Option<String>,
    pub body: String,
    pub status: Status,
    pub priority: Priority,
    pub assignee: Option<String>,
    pub sort_order: f64,
    pub created_at: String,
    pub updated_at: String,
    pub external_url: Option<String>,
    pub external_id: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Label {
    pub id: i64,
    pub name: String,
    pub color: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    pub blocker_id: i64,
    pub blocked_id: i64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comment {
    pub id: i64,
    pub ticket_id: i64,
    pub author: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

fn deserialize_bool<'de, D: serde::Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Bool {
        Boolean(bool),
        Integer(u8),
    }
    match Bool::deserialize(d)? {
        Bool::Boolean(b) => Ok(b),
        Bool::Integer(0) => Ok(false),
        Bool::Integer(1) => Ok(true),
        _ => Err(serde::de::Error::custom("expected boolean or 0/1")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validation() {
        assert_eq!(ticket_key("QUA", 12).unwrap(), "QUA-12");
        for prefix in ["", "qua", "A-", "ABCDEFGHIJK", "É"] {
            assert!(ticket_key(prefix, 1).is_err());
        }
        assert!(ticket_key("Q", 0).is_err());
        assert!(ticket_key("Q", -1).is_err());
        assert!(validate_name(" \n").is_err());
        assert!(validate_name("Ticket").is_ok());
        for color in ["", "#fff", "#zzzzzz", "ffffff", "éfffff"] {
            assert!(validate_color(color).is_err());
        }
        assert!(validate_color("#aB0123").is_ok());
        for n in 0..=4 {
            assert_eq!(u8::from(Priority::try_from(n).unwrap()), n);
        }
        assert!(Priority::try_from(5).is_err());
    }
}
