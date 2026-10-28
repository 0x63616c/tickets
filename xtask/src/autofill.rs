//! Guards against password managers (1Password, LastPass) treating app fields as identity fields.
use std::{fs, path::Path};

const REQUIRED: [&str; 4] = [
    r#"autocomplete="off""#,
    r#"data-1p-ignore="true""#,
    r#"data-lpignore="true""#,
    r#"data-form-type="other""#,
];
const LOGIN: [&str; 2] = [
    r#"autocomplete="username""#,
    r#"autocomplete="current-password""#,
];

/// Returns one message per form control missing the no-autofill attributes.
pub fn violations(file: &str, src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for tag in ["<input", "<textarea", "<select"] {
        for (pos, _) in src.match_indices(tag) {
            let rest = &src[pos..];
            if !rest[tag.len()..].starts_with(|c: char| c.is_whitespace() || c == '/' || c == '>') {
                continue;
            }
            let end = rest.find('>').map_or(rest.len(), |i| i + 1);
            let el = &rest[..end];
            if LOGIN.iter().any(|a| el.contains(a)) {
                continue;
            }
            let line = src[..pos].lines().count().max(1);
            for attr in REQUIRED.iter().filter(|a| !el.contains(*a)) {
                out.push(format!("{file}:{line}: {tag} is missing {attr}"));
            }
            for bad in [r#"id="name""#, r#"name="name""#, r#"id="project-name""#] {
                if el.contains(bad) {
                    out.push(format!("{file}:{line}: {tag} uses identity-like {bad}"));
                }
            }
        }
    }
    out
}

pub fn check(root: &Path) -> crate::Result {
    let mut all = Vec::new();
    let mut stack = vec![root.join("crates/web/src")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let src = fs::read_to_string(&path)?;
                all.extend(violations(&path.display().to_string(), &src));
            }
        }
    }
    if all.is_empty() {
        println!("autofill-check: all non-login form controls opt out of autofill");
        Ok(())
    } else {
        Err(all.join("\n").into())
    }
}

#[cfg(test)]
mod tests {
    use super::violations;

    #[test]
    fn flags_bare_input_and_allows_login() {
        assert_eq!(violations("f", r#"<input id="project-title"/>"#).len(), 4);
        assert!(violations("f", r#"<input autocomplete="username"/>"#).is_empty());
        let ok = r#"<textarea autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other" rows="2"/>"#;
        assert!(violations("f", ok).is_empty());
    }

    #[test]
    fn web_crate_passes() {
        super::check(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..")).unwrap();
    }
}
