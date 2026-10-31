//! Fixed API color choices read from the authoritative CSS palette.
//!
//! Rendering uses CSS variables. Persisted project/label colors must be hex,
//! independent of the user's selected UI accent; read defaults, not computed CSS.
const PALETTE: &str = include_str!("../style/tokens.css");

pub fn value(token: &str) -> &'static str {
    fn resolve(token: &str, depth: usize) -> &'static str {
        assert!(depth < 16, "cyclic palette alias: {token}");
        let root = PALETTE
            .split_once(":root {")
            .expect("palette root")
            .1
            .split_once('}')
            .expect("closed palette root")
            .0;
        let value = root
            .split(';')
            .filter_map(|declaration| {
                declaration
                    .rsplit_once("*/")
                    .map_or(declaration, |(_, rest)| rest)
                    .split_once(':')
            })
            .find_map(|(name, value)| (name.trim() == token).then_some(value.trim()))
            .unwrap_or_else(|| panic!("missing palette color: {token}"));
        if let Some(alias) = value.strip_prefix("var(").and_then(|v| v.strip_suffix(')')) {
            resolve(alias.trim(), depth + 1)
        } else {
            assert!(
                value.len() == 7
                    && value.starts_with('#')
                    && value[1..].bytes().all(|b| b.is_ascii_hexdigit()),
                "API palette color must be hex: {token}"
            );
            value
        }
    }
    resolve(token, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_choices_resolve_to_valid_api_colors() {
        for token in [
            "--project-default",
            "--label-feature",
            "--label-bug",
            "--label-infra",
            "--label-design",
            "--success",
            "--warning",
            "--text-muted",
            "--info",
        ] {
            assert_eq!(value(token).len(), 7);
            u32::from_str_radix(&value(token)[1..], 16).unwrap();
        }
        assert_eq!(value("--project-default"), value("--palette-indigo"));
        assert_eq!(value("--label-bug"), value("--danger"));
    }
}
