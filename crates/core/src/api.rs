//! Runtime independent validation for API mutation documents.
use crate::domain::*;
use serde_json::Value;
pub fn validate(entity: &str, input: &Value) -> Result<(), String> {
    let object = input.as_object().ok_or("expected a JSON object")?;
    let allowed: &[&str] = match entity {
        "projects" => &[
            "name",
            "key_prefix",
            "icon",
            "color",
            "archived",
            "github_owner",
            "github_project_kind",
            "github_project_number",
            "github_repo",
        ],
        "tickets" => &[
            "title",
            "type",
            "scope",
            "body",
            "status",
            "priority",
            "project_id",
            "assignee",
            "sort_order",
            "external_url",
            "external_id",
        ],
        "labels" => &["name", "color"],
        "comments" => &["body"],
        _ => return Err("unknown entity".into()),
    };
    for (key, v) in object {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("unknown field: {key}"));
        }
        match key.as_str() {
            "name" | "title" => validate_name(v.as_str().ok_or("expected string")?)?,
            "type" => {
                if !v.is_null() {
                    crate::ticket_type::validate_type(
                        v.as_str().ok_or("type must be a string or null")?,
                    )?;
                }
            }
            "scope" => {
                if !v.is_null() {
                    crate::ticket_type::normalize_scope(
                        v.as_str().ok_or("scope must be a string or null")?,
                    )?;
                }
            }
            "body" => {
                let s = v.as_str().ok_or("body must be a string")?;
                if entity == "comments" {
                    validate_name(s)?;
                }
            }
            "key_prefix" => validate_prefix(v.as_str().ok_or("expected prefix string")?)?,
            "color" => validate_color(v.as_str().ok_or("expected color string")?)?,
            "icon" => {
                if !v.is_null() {
                    validate_project_icon(v.as_str().ok_or("icon must be a string or null")?)?;
                }
            }
            "status" => {
                serde_json::from_value::<Status>(v.clone()).map_err(|_| "invalid status")?;
            }
            "priority" => {
                serde_json::from_value::<Priority>(v.clone())
                    .map_err(|_| "priority must be 0 through 4")?;
            }
            "project_id" | "github_project_number" => {
                if !(key == "github_project_number" && v.is_null())
                    && !v.as_i64().is_some_and(|n| n > 0)
                {
                    return Err(format!("{key} must be positive"));
                }
            }
            "sort_order" => {
                if !v.as_f64().is_some_and(f64::is_finite) {
                    return Err("sort_order must be finite".into());
                }
            }
            "github_project_kind" => {
                if !matches!(v.as_str(), Some("users" | "orgs")) {
                    return Err("github_project_kind must be users or orgs".into());
                }
            }
            "archived" => {
                if !v.is_boolean() {
                    return Err("archived must be boolean".into());
                }
            }
            _ => {
                if !v.is_null() && !v.is_string() {
                    return Err(format!("{key} must be string or null"));
                }
                if key == "external_id"
                    && v.as_str().is_some_and(|s| {
                        !s.split_once(':').is_some_and(|(source, id)| {
                            !source.trim().is_empty() && !id.trim().is_empty()
                        })
                    })
                {
                    return Err("external_id must be namespaced (source:id)".into());
                }
                if key == "external_url"
                    && v.as_str()
                        .is_some_and(|s| !(s.starts_with("https://") || s.starts_with("http://")))
                {
                    return Err("external_url must be an http(s) URL".into());
                }
            }
        }
    }
    Ok(())
}

pub fn validate_create(entity: &str, input: &Value) -> Result<(), String> {
    validate(entity, input)?;
    let required: &[&str] = match entity {
        "projects" => &["name", "key_prefix", "color"],
        "tickets" => &["title", "project_id"],
        "labels" => &["name", "color"],
        "comments" => &["body"],
        _ => return Err("unknown entity".into()),
    };
    for field in required {
        if input.get(field).is_none() {
            return Err(format!("{field} required"));
        }
    }
    Ok(())
}

pub struct ListOptions {
    pub limit: i64,
    pub cursor: i64,
    pub filters: Vec<(String, String)>,
}
/// Query validation is runtime independent, just like mutation validation.
pub fn list_options(entity: &str, query: &[(String, String)]) -> Result<ListOptions, String> {
    let mut options = ListOptions {
        limit: 50,
        cursor: 0,
        filters: vec![],
    };
    let mut seen = std::collections::HashSet::new();
    for (key, value) in query {
        if !seen.insert(key) {
            return Err(format!("duplicate query parameter: {key}"));
        }
        match key.as_str() {
            "limit" => {
                options.limit = positive_id(value)?;
                if options.limit > 100 {
                    return Err("limit must be 1 through 100".into());
                }
            }
            "cursor" => options.cursor = positive_id(value)?,
            _ if entity != "tickets" => return Err("Unknown list filter".into()),
            "project" | "label" => {
                positive_id(value)?;
                options.filters.push((key.clone(), value.clone()));
            }
            "status" => {
                serde_json::from_value::<Status>(Value::String(value.clone()))
                    .map_err(|_| "Invalid status")?;
                options.filters.push((key.clone(), value.clone()));
            }
            "blocked" if value != "true" && value != "false" => {
                return Err("blocked must be true or false".into());
            }
            "q" if value.trim().is_empty() => return Err("q must not be empty".into()),
            "blocked" | "q" | "assignee" => options.filters.push((key.clone(), value.clone())),
            _ => return Err("Unknown list filter".into()),
        }
    }
    Ok(options)
}
pub fn positive_id(s: &str) -> Result<i64, String> {
    s.parse()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| "Expected positive integer id".into())
}

#[cfg(test)]
mod icon_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn project_icons_are_nullable_bounded_and_never_markup() {
        for icon in [json!(null), json!("box"), json!("🚀"), json!("👩🏽‍💻")] {
            assert!(validate("projects", &json!({"icon":icon})).is_ok());
        }
        for icon in [
            json!(42),
            json!(""),
            json!("unknown"),
            json!("<svg>"),
            json!("🚀\n"),
            json!("🚀".repeat(17)),
        ] {
            assert!(validate("projects", &json!({"icon":icon})).is_err());
        }
    }
}
