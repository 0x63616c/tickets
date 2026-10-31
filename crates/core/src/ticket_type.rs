//! Ticket classification and the canonical Conventional Commit PR title.
pub const TYPES: &[&str] = &[
    "feat", "fix", "chore", "refactor", "docs", "perf", "test", "ci", "build",
];

pub fn validate_type(value: &str) -> Result<(), String> {
    if TYPES.contains(&value) {
        Ok(())
    } else {
        Err(format!("type must be null or one of {}", TYPES.join(", ")))
    }
}

/// Empty scope clears it. Persist only this normalized representation.
pub fn normalize_scope(value: &str) -> Result<Option<String>, String> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() > 32
        || !value.as_bytes()[0].is_ascii_alphanumeric()
        || !value
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"._/-".contains(&c))
    {
        return Err("scope must match [a-z0-9][a-z0-9._/-]{0,31}".into());
    }
    Ok(Some(value))
}

/// Titles are plain and used verbatim. Unclassified tickets use `chore`.
pub fn pr_title(kind: Option<&str>, scope: Option<&str>, title: &str, key: &str) -> String {
    let kind = kind.unwrap_or("chore");
    match scope {
        Some(scope) => format!("{kind}({scope}): {title} [{key}]"),
        None => format!("{kind}: {title} [{key}]"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classification_and_titles() {
        for kind in TYPES {
            assert!(validate_type(kind).is_ok());
            assert_eq!(
                pr_title(Some(kind), Some("ui"), "Grid: home", "TIK-109"),
                format!("{kind}(ui): Grid: home [TIK-109]")
            );
        }
        for kind in ["", "Feat", "feature", "release"] {
            assert!(validate_type(kind).is_err());
        }
        assert_eq!(
            pr_title(None, None, "Grid home", "TIK-109"),
            "chore: Grid home [TIK-109]"
        );
        assert_eq!(
            pr_title(None, Some("ui"), " Grid home ", "TIK-109"),
            "chore(ui):  Grid home  [TIK-109]"
        );
        assert_eq!(
            pr_title(Some("fix"), None, "Grid home", "TIK-109"),
            "fix: Grid home [TIK-109]"
        );
        assert_eq!(
            normalize_scope(" UI/Api._- ").unwrap().as_deref(),
            Some("ui/api._-")
        );
        assert_eq!(normalize_scope("  ").unwrap(), None);
        assert!(normalize_scope(&"a".repeat(32)).is_ok());
        for scope in ["-ui", "a b", "ui!", "é", "a:b", &"a".repeat(33)] {
            assert!(normalize_scope(scope).is_err());
        }
    }
}
