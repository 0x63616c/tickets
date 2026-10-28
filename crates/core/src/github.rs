//! GitHub linking is local parsing only; it never contacts GitHub.
#[derive(Debug, PartialEq, Eq)]
pub struct ProjectLink {
    pub owner: String,
    pub number: i64,
    pub kind: String,
}

pub fn valid_owner(owner: &str) -> bool {
    !owner.is_empty()
        && owner.len() <= 39
        && !owner.starts_with('-')
        && !owner.ends_with('-')
        && owner
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-')
}

impl ProjectLink {
    pub fn parse(owner_or_url: &str, number: &str, kind: &str) -> Result<Self, String> {
        let input = owner_or_url.trim();
        let (owner, number, kind) = if input.contains('/') {
            let path = input
                .strip_prefix("https://github.com/")
                .or_else(|| input.strip_prefix("github.com/"))
                .ok_or("Use a github.com/users|orgs/owner/projects/number URL")?;
            let parts: Vec<_> = path.trim_end_matches('/').split('/').collect();
            if parts.len() != 4 || parts[2] != "projects" {
                return Err("Use a github.com/users|orgs/owner/projects/number URL".into());
            }
            (parts[1], parts[3], parts[0])
        } else {
            (input, number.trim(), kind)
        };
        if !valid_owner(owner) || !["users", "orgs"].contains(&kind) {
            return Err("Enter a valid GitHub owner and owner type".into());
        }
        let number = number
            .parse::<i64>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or("GitHub project number must be positive")?;
        Ok(Self {
            owner: owner.into(),
            number,
            kind: kind.into(),
        })
    }
    pub fn url(&self) -> String {
        format!(
            "https://github.com/{}/{}/projects/{}",
            self.kind, self.owner, self.number
        )
    }
}

pub fn valid_repo(repo: &str) -> bool {
    !repo.is_empty()
        && repo.len() <= 100
        && ![".", ".."].contains(&repo)
        && repo
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_links() {
        for kind in ["users", "orgs"] {
            let url = format!("https://github.com/{kind}/example/projects/7");
            let parsed = ProjectLink::parse(&url, "", "users").unwrap();
            assert_eq!(parsed.url(), url);
            assert_eq!(parsed, ProjectLink::parse("example", "7", kind).unwrap());
        }
        for bad in [
            "https://evil.test/users/example/projects/7",
            "https://github.com.evil/users/example/projects/7",
            "https://github.com/users/example/projects/0",
            "https://github.com/orgs/a/projects/1/extra",
            "a/b",
            "a?x",
            "",
        ] {
            assert!(ProjectLink::parse(bad, "7", "users").is_err(), "{bad}");
        }
        assert!(!valid_repo("repo/path"));
        assert!(valid_repo("example.repo"));
    }
}
