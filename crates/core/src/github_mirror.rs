//! One-shot GitHub migration mapping and shared webhook signature validation.
use serde::{Deserialize, Serialize};
use serde_json::json;

pub fn identity_repo(repo: &str) -> &str {
    match repo {
        "0x63616c/quantum" => "0x63616c/voxelite",
        _ => repo,
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct User {
    pub login: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Label {
    pub name: String,
    pub color: String,
}
impl Label {
    pub fn ticket_color(&self) -> String {
        format!("#{}", self.color)
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    #[serde(default)]
    pub state_reason: Option<String>,
    pub html_url: String,
    #[serde(default)]
    pub labels: Vec<Label>,
    #[serde(default)]
    pub assignees: Vec<User>,
    #[serde(default)]
    pub pull_request: Option<serde_json::Value>,
    #[serde(default)]
    pub updated_at: String,
}
impl Issue {
    pub fn external_id(&self, repo: &str) -> String {
        format!("github:{}:{}", identity_repo(repo), self.number)
    }
    pub fn status(&self) -> &'static str {
        match (self.state.as_str(), self.state_reason.as_deref()) {
            ("closed", Some("not_planned")) => "canceled",
            ("closed", _) => "done",
            _ => "todo",
        }
    }
    pub fn priority(&self) -> u8 {
        self.labels
            .iter()
            .filter_map(|label| match label.name.as_str() {
                "priority:p0" => Some(1),
                "priority:p1" => Some(2),
                "priority:p2" => Some(3),
                "priority:p3" => Some(4),
                _ => None,
            })
            .min()
            .unwrap_or(0)
    }
    pub fn upsert(&self, project: i64) -> serde_json::Value {
        json!({"project_id":project,"title":self.title,"body":self.body.as_deref().unwrap_or(""),"status":self.status(),"priority":self.priority(),"assignee":self.assignees.first().map(|u|&u.login),"external_url":self.html_url})
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Comment {
    pub id: u64,
    pub body: Option<String>,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub issue_url: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}
impl Comment {
    pub fn marker(&self, repo: &str) -> String {
        format!(
            "<!-- tickets-import:github:{}:comment:{} -->",
            identity_repo(repo),
            self.id
        )
    }
    pub fn body(&self, repo: &str) -> String {
        let author = self
            .user
            .as_ref()
            .map(|u| format!("**@{}** on GitHub\n\n", u.login))
            .unwrap_or_default();
        format!(
            "{}\n\n{author}{}{}",
            self.marker(repo),
            if self.created_at.is_empty() {
                String::new()
            } else {
                format!("_Originally posted {}_\n\n", self.created_at)
            },
            self.body.as_deref().unwrap_or("")
        )
    }
}
/// Decode exactly GitHub's SHA-256 signature; compare digest bytes in constant time.
pub fn signature_matches(header: &str, digest: &[u8]) -> bool {
    let Some(hex) = header.strip_prefix("sha256=") else {
        return false;
    };
    if hex.len() != 64 || digest.len() != 32 {
        return false;
    }
    let mut bytes = [0u8; 32];
    for (i, chunk) in hex.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let Ok(s) = std::str::from_utf8(chunk) else {
            return false;
        };
        let Ok(byte) = u8::from_str_radix(s, 16) else {
            return false;
        };
        bytes[i] = byte;
    }
    crate::auth::constant_eq(&bytes, digest)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migration_state_priority_and_attribution() {
        let mut value = json!({"number":3,"title":"Title","body":null,"state":"open","html_url":"https://github.com/a/b/issues/3","labels":[{"name":"priority:p2","color":"abcdef"}]});
        for (state, reason, expected) in [
            ("open", None, "todo"),
            ("closed", Some("completed"), "done"),
            ("closed", Some("not_planned"), "canceled"),
            ("closed", None, "done"),
        ] {
            value["state"] = json!(state);
            value["state_reason"] = json!(reason);
            let issue: Issue = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(issue.upsert(1)["status"], expected);
            assert_eq!(issue.upsert(1)["priority"], 3);
        }
        let c:Comment=serde_json::from_value(json!({"id":1,"body":"Hello","user":{"login":"octocat"},"created_at":"2026-10-01T00:00:00Z"})).unwrap();
        assert!(c.body("a/b").contains("**@octocat** on GitHub"));
        assert!(c.body("a/b").contains("2026-10-01T00:00:00Z"));
        assert_eq!(c.marker("0x63616c/quantum"), c.marker("0x63616c/voxelite"));
    }
    #[test]
    fn mapping() {
        let issue:Issue=serde_json::from_value(json!({"number":3,"title":"Title","body":null,"state":"closed","html_url":"https://github.com/a/b/issues/3","assignees":[{"login":"agent"}]})).unwrap();
        assert_eq!(
            issue.external_id("0x63616c/quantum"),
            "github:0x63616c/voxelite:3"
        );
        assert_eq!(issue.upsert(1)["status"], "done");
        assert_eq!(issue.upsert(1)["assignee"], "agent");
        let c: Comment = serde_json::from_value(
            json!({"id":7,"body":"<script>x</script>","user":{"login":"agent"}}),
        )
        .unwrap();
        assert!(
            c.body("a/b")
                .starts_with("<!-- tickets-import:github:a/b:comment:7 -->")
        );
        let html = crate::markdown::render(&c.body("a/b"));
        assert!(!html.contains("tickets-import"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("@agent"));
    }
    #[test]
    fn signatures() {
        let header = format!("sha256={}", "ab".repeat(32));
        assert!(signature_matches(&header, &[0xab; 32]));
        assert!(!signature_matches(&header, &[0xac; 32]));
        assert!(!signature_matches("", &[0; 32]));
    }
}
