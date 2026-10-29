//! GitHub PR key parsing and replay-safe persistence, independent of the Worker.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub prefix: String,
    pub number: i64,
    pub closes: bool,
}

fn key(text: &str) -> Option<(String, i64)> {
    let (prefix, number) = text.split_once('-')?;
    crate::domain::validate_prefix(prefix).ok()?;
    if number.is_empty() || !number.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let number = number.parse().ok()?;
    (number > 0).then(|| (prefix.to_owned(), number))
}

/// Unknown projects are filtered by the database join, never created by ingestion.
pub fn links(title: &str, body: &str) -> Vec<Link> {
    let mut found = BTreeMap::new();
    for part in title.split('[').skip(1) {
        if let Some((text, _)) = part.split_once(']')
            && let Some(k) = key(text)
        {
            found.insert(k, true);
        }
    }
    let mut intent = None;
    for word in body.split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_')) {
        if word.is_empty() {
            continue;
        }
        match word.to_ascii_lowercase().as_str() {
            "closes" | "fixes" | "resolves" => intent = Some(true),
            "refs" => intent = Some(false),
            "and" if intent.is_some() => {}
            _ => {
                if let (Some(closes), Some(k)) = (intent, key(word)) {
                    found.insert(k, closes);
                } else {
                    intent = None;
                }
            }
        }
    }
    found
        .into_iter()
        .map(|((prefix, number), closes)| Link {
            prefix,
            number,
            closes,
        })
        .collect()
}

// Timestamp ties prefer terminal states, then review-ready over draft. Identical
// snapshots do not write; conflicting same-second edits keep the first delivery.
pub const UPSERT: &str = "INSERT INTO github_pull_request_events(repo,number,url,title,state,head_sha,updated_at,links) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(repo,number) DO UPDATE SET url=excluded.url,title=excluded.title,state=excluded.state,head_sha=excluded.head_sha,updated_at=excluded.updated_at,links=excluded.links WHERE excluded.updated_at>github_pull_request_events.updated_at OR (excluded.updated_at=github_pull_request_events.updated_at AND CASE excluded.state WHEN 'merged' THEN 3 WHEN 'closed' THEN 2 WHEN 'open' THEN 1 ELSE 0 END > CASE github_pull_request_events.state WHEN 'merged' THEN 3 WHEN 'closed' THEN 2 WHEN 'open' THEN 1 ELSE 0 END) RETURNING repo,(SELECT count(*) FROM json_each(links) j JOIN projects p ON p.key_prefix=json_extract(j.value,'$.prefix') JOIN tickets t ON t.project_id=p.id AND t.number=json_extract(j.value,'$.number')) AS matches";

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keywords_colons_and_invalid_keys() {
        for word in ["Closes", "fixes", "RESOLVES"] {
            for separator in [" ", ": ", ":"] {
                assert!(links("", &format!("{word}{separator}TIK-32"))[0].closes);
            }
        }
        assert!(!links("[TIK-32]", "Refs: TIK-32")[0].closes);
        assert_eq!(links("", "**Closes:** TIK-1, APP-2 and BOX-3").len(), 3);
        assert!(links("", "Closes NOT_TIK-32").is_empty());
        assert!(links("", "Closes TIK-999999999999999999999999").is_empty());
        assert!(links("", "Encloses TIK-32; Fixes tik-32").is_empty());
    }
    #[test]
    fn parses_multiple_keys_and_reference_override() {
        let parsed = links(
            "feat [WWW-12] [TIK-32] [bad-2]",
            "Refs WWW-12\nFiXeS: APP-3; resolves BOX-9",
        );
        assert_eq!(
            parsed,
            vec![
                Link {
                    prefix: "APP".into(),
                    number: 3,
                    closes: true
                },
                Link {
                    prefix: "BOX".into(),
                    number: 9,
                    closes: true
                },
                Link {
                    prefix: "TIK".into(),
                    number: 32,
                    closes: true
                },
                Link {
                    prefix: "WWW".into(),
                    number: 12,
                    closes: false
                }
            ]
        );
        assert!(links("[A-0] [A--1] [ABCDEFGHIJK-1]", "closes A-2oops").is_empty());
    }
}
