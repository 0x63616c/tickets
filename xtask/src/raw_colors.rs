//! Palette-only guard across CSS, Rust literals, HTML, SVG and script sources.
use proc_macro2::{TokenStream, TokenTree};
use regex::Regex;
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path, process::Command};

const PALETTE: &str = "crates/web/style/tokens.css";
const ALLOWLIST: &str = "xtask/raw-color-allowlist.json";
const NAMES: &str = "aliceblue antiquewhite aqua aquamarine azure beige bisque black blanchedalmond blue blueviolet brown burlywood cadetblue chartreuse chocolate coral cornflowerblue cornsilk crimson cyan darkblue darkcyan darkgoldenrod darkgray darkgreen darkgrey darkkhaki darkmagenta darkolivegreen darkorange darkorchid darkred darksalmon darkseagreen darkslateblue darkslategray darkslategrey darkturquoise darkviolet deeppink deepskyblue dimgray dimgrey dodgerblue firebrick floralwhite forestgreen fuchsia gainsboro ghostwhite gold goldenrod gray green greenyellow grey honeydew hotpink indianred indigo ivory khaki lavender lavenderblush lawngreen lemonchiffon lightblue lightcoral lightcyan lightgoldenrodyellow lightgray lightgreen lightgrey lightpink lightsalmon lightseagreen lightskyblue lightslategray lightslategrey lightsteelblue lightyellow lime limegreen linen magenta maroon mediumaquamarine mediumblue mediumorchid mediumpurple mediumseagreen mediumslateblue mediumspringgreen mediumturquoise mediumvioletred midnightblue mintcream mistyrose moccasin navajowhite navy oldlace olive olivedrab orange orangered orchid palegoldenrod palegreen paleturquoise palevioletred papayawhip peachpuff peru pink plum powderblue purple rebeccapurple red rosybrown royalblue saddlebrown salmon sandybrown seagreen seashell sienna silver skyblue slateblue slategray slategrey snow springgreen steelblue tan teal thistle tomato turquoise violet wheat white whitesmoke yellow yellowgreen";
type Counts = BTreeMap<String, usize>;

struct Scanner {
    raw: Regex,
    comments: Regex,
    declarations: Regex,
    quoted: Regex,
    words: Regex,
}
impl Scanner {
    fn new() -> Self {
        Self {
            raw: Regex::new(r"(?i)(?:#|%23)(?:[0-9a-f]{8}|[0-9a-f]{6}|[0-9a-f]{4}|[0-9a-f]{3})\b|\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\([^)]*\)").unwrap(),
            // Preserve quoted strings, including URLs, while discarding comments.
            comments: Regex::new(r#"(?s)/\*.*?\*/|<!--.*?-->|//[^\n]*|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|`(?:\\.|[^`\\])*`"#).unwrap(),
            declarations: Regex::new(r#"(?:^|[;{'`"])\s*[\w-]+\s*:\s*([^;{}'`"]+)"#).unwrap(),
            quoted: Regex::new(r#"["'`]([a-zA-Z]+)["'`]|(?:fill|stroke|color)\s*=\s*([a-zA-Z]+)"#).unwrap(),
            words: Regex::new(r"[-a-zA-Z_][\w-]*").unwrap(),
        }
    }
    fn colors(&self, source: &str) -> Vec<String> {
        let source = self
            .comments
            .replace_all(source, |c: &regex::Captures<'_>| {
                let s = &c[0];
                if s.starts_with("/*") || s.starts_with("//") || s.starts_with("<!--") {
                    " ".repeat(s.len())
                } else {
                    s.to_owned()
                }
            });
        let mut found: BTreeMap<usize, String> = self
            .raw
            .find_iter(&source)
            // A short hexadecimal CSS ID selector is not a color.
            .filter(|m| !source[m.end()..].trim_start().starts_with('{'))
            .map(|m| (m.start(), m.as_str().to_owned()))
            .collect();
        let named = |word: &str| {
            NAMES
                .split_whitespace()
                .any(|n| n.eq_ignore_ascii_case(word))
        };
        if named(source.trim()) {
            found.insert(0, source.trim().to_owned());
        }
        for captures in self.quoted.captures_iter(&source) {
            let m = captures.get(1).or_else(|| captures.get(2)).unwrap();
            if named(m.as_str()) {
                found.insert(m.start(), m.as_str().to_owned());
            }
        }
        for captures in self.declarations.captures_iter(&source) {
            let value = captures.get(1).unwrap();
            // CSS selectors / prose aren't declarations. Inline declarations in
            // Rust/JS strings use the same grammar as stylesheets.
            for m in self.words.find_iter(value.as_str()) {
                if named(m.as_str()) {
                    found.insert(value.start() + m.start(), m.as_str().to_owned());
                }
            }
        }
        found.into_values().collect()
    }
    fn rust(&self, stream: TokenStream, out: &mut Vec<String>) {
        let mut attribute = false;
        for token in stream {
            match &token {
                TokenTree::Group(group)
                    if attribute
                        && group.delimiter() == proc_macro2::Delimiter::Bracket
                        && group.stream().to_string().starts_with("doc =") => {}
                TokenTree::Group(group) => self.rust(group.stream(), out),
                TokenTree::Literal(literal) => {
                    if let Ok(value) = syn::parse_str::<syn::LitStr>(&literal.to_string()) {
                        out.extend(self.colors(&value.value()));
                    }
                }
                _ => {}
            }
            attribute = matches!(token, TokenTree::Punct(p) if p.as_char() == '#');
        }
    }
    fn scan(&self, path: &str, source: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        if path.ends_with(".rs") {
            let mut out = Vec::new();
            self.rust(
                source.parse().map_err(|e| format!("{path}: {e}"))?,
                &mut out,
            );
            Ok(out)
        } else {
            Ok(self.colors(source))
        }
    }
}

fn suggestion(palette: &str, literal: &str) -> String {
    let normalized = |s: &str| match s.to_ascii_lowercase().as_str() {
        "#fff" | "white" => "#ffffff".to_owned(),
        "#000" | "black" => "#000000".to_owned(),
        _ => s.to_ascii_lowercase(),
    };
    for line in palette.lines() {
        if let Some((token, value)) = line.trim().split_once(':')
            && token.starts_with("--")
            && normalized(value.trim().trim_end_matches(';')) == normalized(literal)
        {
            return format!("use var({token}) (crate::colors::value(\"{token}\") for API data)");
        }
    }
    "use var(--text), var(--bg), var(--border), var(--accent), or a semantic --status-*/--priority-*/--success/--warning/--danger/--info token; add a deliberate palette role if none fits".into()
}

fn compare(actual: &Counts, allowlist: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    let Some(entries) = allowlist.as_object() else {
        return vec!["allowlist must be an object".into()];
    };
    for (key, entry) in entries {
        let expected = entry["count"].as_u64().unwrap_or(0) as usize;
        if expected == 0 || entry["reason"].as_str().is_none_or(|r| r.trim().is_empty()) {
            errors.push(format!(
                "{key}: exception needs a positive count and a reason"
            ));
        }
        if actual.get(key).copied().unwrap_or(0) != expected {
            errors.push(format!(
                "{key}: stale exception; update/remove it in {ALLOWLIST}"
            ));
        }
    }
    for (key, count) in actual {
        if !entries.contains_key(key) {
            errors.push(format!("{key} ({count} occurrences)"));
        }
    }
    errors
}

pub fn check(root: &Path) -> crate::Result {
    let output = Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "crates/web",
            "crates/worker",
            "crates/core/src/github_app.rs",
        ])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err("raw-colors: git ls-files failed".into());
    }
    let palette = fs::read_to_string(root.join(PALETTE))?;
    let scanner = Scanner::new();
    let mut actual = Counts::new();
    for path in std::str::from_utf8(&output.stdout)?.split_terminator('\0') {
        let extension = Path::new(path)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if path == PALETTE
            || path.starts_with("crates/worker/tests/")
            || !matches!(
                extension,
                "css"
                    | "rs"
                    | "js"
                    | "mjs"
                    | "ts"
                    | "jsx"
                    | "tsx"
                    | "html"
                    | "svg"
                    | "json"
                    | "webmanifest"
                    | "py"
            )
            || !root.join(path).exists()
        {
            continue;
        }
        for color in scanner.scan(path, &fs::read_to_string(root.join(path))?)? {
            *actual.entry(format!("{path}|{color}")).or_default() += 1;
        }
    }
    let allowlist: Value = serde_json::from_str(&fs::read_to_string(root.join(ALLOWLIST))?)?;
    let errors = compare(&actual, &allowlist)
        .into_iter()
        .map(|e| {
            let literal = e
                .split_once('|')
                .map(|(_, s)| s.split(" (").next().unwrap_or(s))
                .unwrap_or("");
            format!("{e}: {}", suggestion(&palette, literal))
        })
        .collect::<Vec<_>>();
    if !errors.is_empty() {
        return Err(format!(
            "raw-colors: colors belong in {PALETTE}\n{}",
            errors.join("\n")
        )
        .into());
    }
    println!(
        "raw-colors: palette-only sources; {} documented exception literals",
        actual.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_all_syntaxes_and_fallbacks() {
        let s = Scanner::new();
        for value in [
            "#fff",
            "#abcd",
            "#aabbcc",
            "#aabbccdd",
            "%23aabbcc",
            "rgb(1 2 3)",
            "rgba(1,2,3,.5)",
            "hsl(0 0% 0%)",
            "hsla(0,0%,0%,.5)",
            "red",
            "RebeccaPurple",
        ] {
            assert!(
                !s.colors(&format!("color:var(--oops,{value})")).is_empty(),
                "{value}"
            );
        }
        assert!(s.colors("color:var(--text);fill:currentColor;background:transparent;border:inherit;--red-token:var(--danger);color:var(--white);fill:var(--red)").is_empty());
        assert!(
            s.colors("/* red #fff */ #abc { color:var(--text) }")
                .is_empty()
        );
    }
    #[test]
    fn rust_macros_raw_strings_and_scripts_are_checked() {
        let s = Scanner::new();
        assert_eq!(
            s.scan(
                "a.rs",
                r##"// "#fff"
            /// Documentation: #fff
            view! { <svg fill="red" style=r#"color:rgb(0 0 0)"#/> }
            let js = "ctx.fillStyle = 'blue'"; let data = "#abcdef";"##
            )
            .unwrap(),
            ["red", "rgb(0 0 0)", "blue", "#abcdef"]
        );
        assert_eq!(
            s.scan(
                "a.html",
                "<!-- #fff --><svg fill='navy'/><script>x.style.color='red'</script>"
            )
            .unwrap(),
            ["navy", "red"]
        );
        assert_eq!(
            s.scan("a.html", r#"<b style="background:rebeccapurple"/>"#)
                .unwrap(),
            ["rebeccapurple"]
        );
        assert_eq!(
            s.scan(
                "a.js",
                "const c=`red`; const css=`background:blue`; const url=`https://example.test/#fff`"
            )
            .unwrap(),
            ["red", "blue", "#fff"]
        );
        assert!(
            s.scan(
                "a.js",
                "// #fff\nconst url='https://example.test'; x.style.color='red'"
            )
            .unwrap()
            .contains(&"red".to_string())
        );
    }
    #[test]
    fn exceptions_are_exact_counted_and_require_reasons() {
        let actual = BTreeMap::from([("a|#fff".into(), 1)]);
        let valid = serde_json::json!({"a|#fff":{"count":1,"reason":"standalone favicon"}});
        assert!(compare(&actual, &valid).is_empty());
        assert!(!compare(&Counts::new(), &valid).is_empty());
        assert!(!compare(&actual, &serde_json::json!({})).is_empty());
        assert!(
            !compare(
                &actual,
                &serde_json::json!({"a|#fff":{"count":2,"reason":""}})
            )
            .is_empty()
        );
    }
    #[test]
    fn errors_name_existing_tokens() {
        assert!(suggestion("--danger: #eb5757;", "#eb5757").contains("var(--danger)"));
        assert!(suggestion("--white: #ffffff;", "#fff").contains("var(--white)"));
    }
}
