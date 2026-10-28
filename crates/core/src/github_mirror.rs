//! GitHub is authoritative. Keep importer and runtime identity/content identical.
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
    pub fn upsert(&self, project: i64) -> serde_json::Value {
        json!({"project_id":project,"title":self.title,"body":self.body.as_deref().unwrap_or(""),"status":if self.state=="closed" {"done"} else {"todo"},"assignee":self.assignees.first().map(|u|&u.login),"external_url":self.html_url})
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
            "{}\n\n{author}{}",
            self.marker(repo),
            self.body.as_deref().unwrap_or("")
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repo {
    pub id: u64,
    pub identity: String,
    pub key: String,
}
impl Repo {
    pub fn name(&self) -> &str {
        if self.key == "QUA" {
            "Quantum"
        } else {
            self.identity.rsplit('/').next().unwrap_or(&self.identity)
        }
    }
}
pub fn config(input: &str) -> Result<Vec<Repo>, String> {
    let mut repos = Vec::<Repo>::new();
    if input.trim().is_empty() {
        return Ok(repos);
    }
    for entry in input.split(',') {
        let (id, rest) = entry
            .split_once('=')
            .ok_or("expected repository ID=identity:KEY")?;
        let id = id.parse::<u64>().map_err(|_| "invalid repository ID")?;
        let (identity, key) = rest.split_once(':').ok_or("expected identity:KEY")?;
        let parts: Vec<_> = identity.split('/').collect();
        if id == 0
            || parts.len() != 2
            || parts.iter().any(|p| {
                p.is_empty()
                    || *p == "."
                    || *p == ".."
                    || !p
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            })
        {
            return Err("invalid repository identity".into());
        }
        crate::domain::validate_prefix(key)?;
        let identity = identity_repo(&identity.to_ascii_lowercase()).to_owned();
        if repos
            .iter()
            .any(|r| r.id == id || r.identity == identity || r.key == key)
        {
            return Err("duplicate mirror repository or project".into());
        }
        repos.push(Repo {
            id,
            identity,
            key: key.to_owned(),
        });
    }
    Ok(repos)
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
    fn configuration() {
        assert_eq!(
            config("1=0x63616c/quantum:QUA").unwrap()[0].identity,
            "0x63616c/voxelite"
        );
        for bad in [
            "0=a/b:A",
            "a=a/b:A",
            "1=a:A",
            "1=a/b:lower",
            "1=a/b:A,1=a/c:B",
            "1=a/b:A,2=a/c:A",
        ] {
            assert!(config(bad).is_err(), "{bad}")
        }
    }
    #[test]
    fn signatures() {
        let header = format!("sha256={}", "ab".repeat(32));
        assert!(signature_matches(&header, &[0xab; 32]));
        assert!(!signature_matches(&header, &[0xac; 32]));
        assert!(!signature_matches("", &[0; 32]));
    }
}

/// A sweep keeps `since` fixed across pages, retaining the high-water timestamp
/// even when GitHub ends pagination with an empty page. Call only after commit.
pub fn advance_cursor(
    since: &str,
    high_water: &str,
    page: u32,
    timestamps: &[String],
    full: bool,
) -> (String, String, u32) {
    let high = timestamps
        .iter()
        .map(String::as_str)
        .chain([high_water, since])
        .max()
        .unwrap_or(since)
        .to_owned();
    if full {
        (since.to_owned(), high, page + 1)
    } else {
        (high.clone(), high, 1)
    }
}
#[cfg(test)]
mod cursor_tests {
    use super::*;
    #[test]
    fn only_committed_pages_advance() {
        let (since, high, page) = advance_cursor("a", "a", 1, &["b".into(), "b".into()], true);
        assert_eq!((&*since, &*high, page), ("a", "b", 2));
        assert_eq!(
            advance_cursor(&since, &high, page, &[], false),
            ("b".into(), "b".into(), 1)
        );
        assert_eq!(
            advance_cursor("b", "b", 1, &[], false),
            ("b".into(), "b".into(), 1)
        );
    }
}
