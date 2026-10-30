//! Command-palette query language and compact search documents.
use crate::domain::{Priority, Status, validate_prefix};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SearchTicket {
    pub id: i64,
    pub project_id: i64,
    pub key: String,
    pub title: String,
    pub status: Status,
    pub priority: Priority,
    pub assignee: Option<String>,
    pub updated_at: String,
    pub external_url: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct SearchQuery {
    pub text: String,
    pub terms: Vec<String>,
    pub status: Option<Status>,
    pub project: Option<String>,
}
impl SearchQuery {
    pub fn parse(input: &str) -> Result<Self, String> {
        if input.chars().count() > 200 {
            return Err("Use at most 200 characters.".into());
        }
        let mut query = Self::default();
        for word in input.split_whitespace() {
            let word = word.to_lowercase();
            if let Some(value) = word.strip_prefix("status:") {
                if query.status.is_some() {
                    return Err("Use one status: filter.".into());
                }
                query.status = Some(serde_json::from_value(serde_json::json!(value)).map_err(
                    |_| "Use status:backlog, todo, in_progress, in_review, done or canceled.",
                )?);
            } else if let Some(value) = word.strip_prefix("p:") {
                let prefix = value.to_uppercase();
                validate_prefix(&prefix).map_err(|_| "Use a project key, like p:QUA.")?;
                if query.project.replace(prefix).is_some() {
                    return Err("Use one p: project filter.".into());
                }
            } else {
                query.terms.push(word);
            }
        }
        if query.terms.len() > 8 {
            return Err("Search with up to 8 words.".into());
        }
        query.text = query.terms.join(" ");
        Ok(query)
    }
    pub fn filtered(&self) -> bool {
        self.status.is_some() || self.project.is_some()
    }
}

/// Literal SQL LIKE pattern: user wildcards never become query syntax.
pub fn pattern(text: &str, fuzzy: bool) -> String {
    let mut out = String::from("%");
    for c in text.chars() {
        if matches!(c, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(c);
        if fuzzy {
            out.push('%');
        }
    }
    if !fuzzy {
        out.push('%');
    }
    out
}

/// Ordered-character matching, used for local commands and fixture previews.
pub fn fuzzy_match(text: &str, query: &str) -> bool {
    let text = text.to_lowercase();
    query.to_lowercase().split_whitespace().all(|word| {
        let mut chars = text.chars();
        word.chars()
            .all(|c| chars.by_ref().any(|candidate| c == candidate))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filters_are_case_insensitive_and_not_search_terms() {
        let q = SearchQuery::parse("p:qua render mesh status:in_progress").unwrap();
        assert_eq!(q.project.as_deref(), Some("QUA"));
        assert_eq!(q.status, Some(Status::InProgress));
        assert_eq!(q.text, "render mesh");
        for invalid in ["status:wat", "p:", "p:QUA p:TIK", "status:todo status:done"] {
            assert!(SearchQuery::parse(invalid).is_err());
        }
    }
    #[test]
    fn literal_patterns_and_unicode_subsequences() {
        assert_eq!(pattern("50%_\\", false), "%50\\%\\_\\\\%");
        assert!(fuzzy_match("Command palette", "cmd palete"));
        assert!(fuzzy_match("Réviser le rendu", "rév rnd"));
        assert!(!fuzzy_match("palette", "ticket"));
    }
}
