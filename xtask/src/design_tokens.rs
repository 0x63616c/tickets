//! Design-token lint and Quantum-style exact-match, shrink-only debt ratchet.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use proc_macro2::{TokenStream, TokenTree};
use serde_json::{Value, json};

const ALLOWLIST: &str = "xtask/lint-allowlist.json";
type Counts = BTreeMap<String, usize>;
type Fallible<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn git(root: &Path, args: &[&str]) -> Fallible<String> {
    let output = Command::new("git").args(args).current_dir(root).output()?;
    if !output.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn read(root: &Path, path: &str, staged: bool) -> Fallible<String> {
    if staged {
        git(root, &["show", &format!(":{path}")])
    } else {
        Ok(fs::read_to_string(root.join(path))?)
    }
}

// Validate the ref first: a broken/shallow ref must never disable the ratchet.
fn previous_baseline(root: &Path, reference: &str) -> Fallible<Option<Counts>> {
    let commit = git(
        root,
        &["rev-parse", "--verify", &format!("{reference}^{{commit}}")],
    )?;
    let commit = commit.trim();
    if git(root, &["ls-tree", "--name-only", commit, "--", ALLOWLIST])?
        .trim()
        .is_empty()
    {
        return Ok(None); // First rollout only.
    }
    Ok(Some(serde_json::from_str(&git(
        root,
        &["show", &format!("{commit}:{ALLOWLIST}")],
    )?)?))
}

fn ratchet(actual: &Counts, baseline: &Counts, previous: Option<&Counts>) -> Vec<String> {
    let mut errors = Vec::new();
    for key in actual
        .keys()
        .chain(baseline.keys())
        .collect::<BTreeSet<_>>()
    {
        let observed = actual.get(key).copied().unwrap_or(0);
        let allowed = baseline.get(key).copied().unwrap_or(0);
        if observed != allowed || allowed == 0 {
            errors.push(format!("{key}: found {observed}, allowlisted {allowed}; use tokens or remove stale debt from {ALLOWLIST}"));
        }
    }
    if let Some(previous) = previous {
        for (key, count) in baseline {
            let old = previous.get(key).copied().unwrap_or(0);
            if *count > old {
                errors.push(format!(
                    "{key}: allowlist grew from {old} to {count}; {ALLOWLIST} may only shrink"
                ));
            }
        }
    }
    errors
}

fn raw_value(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if ["rgb(", "rgba(", "hsl(", "hsla(", "oklch(", "oklab("]
        .iter()
        .any(|f| lower.contains(f))
    {
        return true;
    }
    for (i, c) in lower.char_indices() {
        let tail = &lower[i..];
        if c == '#' {
            let count = tail[1..].bytes().take_while(u8::is_ascii_hexdigit).count();
            if [3, 4, 6, 8].contains(&count)
                && !tail[1 + count..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
            {
                return true;
            }
        }
        if c.is_ascii_digit() || c == '.' || c == '}' {
            let n = tail
                .bytes()
                .take_while(|c| c.is_ascii_digit() || *c == b'.' || *c == b'}')
                .count();
            for unit in ["px", "rem", "em"] {
                if tail[n..].starts_with(unit)
                    && !tail[n + unit.len()..].starts_with(|c: char| c.is_ascii_alphanumeric())
                {
                    return true;
                }
            }
        }
    }
    false
}

#[derive(Default)]
struct Scan {
    counts: Counts,
    inline: Vec<Value>,
}

impl Scan {
    fn count(&mut self, path: &str, rule: &str, value: &str) {
        // JSON encoding keeps multiline literals on one line and avoids key ambiguity.
        let literal = serde_json::to_string(value).expect("string is JSON serializable");
        *self
            .counts
            .entry(format!("{path}:{rule}:{literal}"))
            .or_default() += 1;
    }

    fn inspect(&mut self, path: &str, source: &str) -> Fallible<()> {
        let tokens: TokenStream = source.parse().map_err(|e| format!("{path}: {e}"))?;
        self.tokens(path, tokens);
        Ok(())
    }

    fn tokens(&mut self, path: &str, stream: TokenStream) {
        let tokens: Vec<_> = stream.into_iter().collect();
        let doc_attributes: BTreeSet<_> = tokens
            .iter()
            .enumerate()
            .filter_map(|(i, token)| {
                let TokenTree::Group(group) = token else {
                    return None;
                };
                (i > 0
                    && tokens[i - 1].to_string() == "#"
                    && group.delimiter() == proc_macro2::Delimiter::Bracket
                    && group.stream().to_string().starts_with("doc ="))
                .then_some(i)
            })
            .collect();
        let mut inline_literals = BTreeSet::new();
        for (i, token) in tokens.iter().enumerate() {
            if !matches!(token, TokenTree::Ident(name) if name == "style") {
                continue;
            }
            let mut next = i + 1;
            let mut property = String::new();
            if tokens.get(next).is_some_and(|t| t.to_string() == ":") {
                next += 1;
                while let Some(t) = tokens.get(next) {
                    if t.to_string() == "=" {
                        break;
                    }
                    if !matches!(t, TokenTree::Ident(_) | TokenTree::Punct(_)) {
                        break;
                    }
                    property.push_str(&t.to_string());
                    next += 1;
                }
            }
            if !tokens.get(next).is_some_and(|t| t.to_string() == "=") {
                continue;
            }
            next += 1;
            if let Some(TokenTree::Literal(lit)) = tokens.get(next)
                && let Ok(lit) = syn::parse_str::<syn::LitStr>(&lit.to_string())
            {
                inline_literals.insert(next);
                let value = lit.value();
                if property.is_empty() {
                    // Literal declaration lists have no Rust interpolation. Stylelint
                    // handles stylesheet syntax; inline values share its value policy.
                    for declaration in value.split(';').filter(|d| !d.trim().is_empty()) {
                        if let Some((property, value)) = declaration.split_once(':') {
                            self.inline.push(json!({"path":path,"property":property.trim(),"value":value.trim()}));
                        } else {
                            self.count(path, "rust-inline-invalid", &value);
                        }
                    }
                } else {
                    self.inline
                        .push(json!({"path":path,"property":property,"value":value}));
                }
            } else {
                // Variables, closures and format! are runtime CSS injection sites.
                // Keep each existing site auditable; new styling belongs in CSS.
                let expression = tokens
                    .get(next)
                    .map(ToString::to_string)
                    .unwrap_or_default();
                self.count(
                    path,
                    "rust-dynamic-style",
                    &format!("{property}={expression}"),
                );
            }
        }
        for (i, token) in tokens.into_iter().enumerate() {
            match token {
                TokenTree::Group(group) => {
                    if !doc_attributes.contains(&i) {
                        self.tokens(path, group.stream());
                    }
                }
                TokenTree::Literal(lit) if !inline_literals.contains(&i) => {
                    if let Ok(lit) = syn::parse_str::<syn::LitStr>(&lit.to_string())
                        && raw_value(&lit.value())
                    {
                        self.count(path, "rust-raw", &lit.value());
                    }
                }
                _ => {}
            }
        }
    }
}

fn css_findings(
    root: &Path,
    files: &BTreeMap<String, String>,
    inline: &[Value],
) -> Fallible<Counts> {
    let mut child = Command::new("node")
        .arg("crates/web/lint/collect.mjs")
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("design-tokens needs Node and npm ci --prefix crates/web: {e}"))?;
    let input = serde_json::to_vec(&json!({"files":files,"inline":inline}))?;
    let write = child
        .stdin
        .take()
        .ok_or("missing lint stdin")?
        .write_all(&input);
    let output = child.wait_with_output()?;
    write?;
    if !output.status.success() {
        return Err("Stylelint failed; run npm ci --prefix crates/web and retry".into());
    }
    let findings: Vec<Value> = serde_json::from_slice(&output.stdout)?;
    let mut counts = Counts::new();
    for finding in findings {
        let key = finding["key"]
            .as_str()
            .ok_or("missing Stylelint finding key")?;
        *counts.entry(key.to_owned()).or_default() += 1;
    }
    Ok(counts)
}

pub fn check(root: &Path, args: &[String]) -> crate::Result {
    if args
        .iter()
        .any(|a| !matches!(a.as_str(), "--staged" | "--css" | "--report"))
    {
        return Err("usage: cargo xtask check design-tokens [--staged] [--css] [--report]".into());
    }
    let staged = args.iter().any(|a| a == "--staged");
    let css_only = args.iter().any(|a| a == "--css");
    let report = args.iter().any(|a| a == "--report");
    let tracked = git(root, &["ls-files", "-z", "--cached", "--", "crates/web"])?;
    let untracked = if staged {
        String::new()
    } else {
        git(
            root,
            &[
                "ls-files",
                "-z",
                "--others",
                "--exclude-standard",
                "--",
                "crates/web",
            ],
        )?
    };
    let mut scan = Scan::default();
    let mut css = BTreeMap::new();
    for path in tracked
        .split_terminator('\0')
        .chain(untracked.split_terminator('\0'))
        .collect::<BTreeSet<_>>()
    {
        if !staged && !root.join(path).exists() {
            continue;
        }
        if path.ends_with(".css") {
            css.insert(path.to_owned(), read(root, path, staged)?);
        } else if !css_only && path.ends_with(".rs") {
            scan.inspect(path, &read(root, path, staged)?)?;
        }
    }
    scan.counts.extend(css_findings(root, &css, &scan.inline)?);
    if report {
        println!("{}", serde_json::to_string_pretty(&scan.counts)?);
        return Ok(());
    }
    let baseline: Counts = serde_json::from_str(&read(root, ALLOWLIST, staged)?)?;
    let reference = std::env::var("LINT_BASE_REF").unwrap_or_else(|_| "HEAD".into());
    let previous = previous_baseline(root, &reference)?;
    let filter = |counts: Counts| -> Counts {
        counts
            .into_iter()
            .filter(|(key, _)| {
                !css_only
                    || key
                        .split_once(':')
                        .is_some_and(|(_, rule)| rule.starts_with("css:"))
            })
            .collect()
    };
    // Always check growth of the whole file, including in CSS-only mode.
    let mut errors = ratchet(&baseline, &baseline, previous.as_ref());
    errors.extend(ratchet(&scan.counts, &filter(baseline), None));
    if !errors.is_empty() {
        return Err(errors.join("\n").into());
    }
    println!(
        "design-tokens: {} existing findings exactly match {ALLOWLIST}; no growth",
        scan.counts.values().sum::<usize>()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(source: &str) -> Scan {
        let mut scan = Scan::default();
        scan.inspect("crates/web/src/example.rs", source).unwrap();
        scan
    }

    struct GitFixture(std::path::PathBuf);

    impl GitFixture {
        fn new() -> Self {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "tickets-token-lint-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir(&root).unwrap();
            git(&root, &["init", "--quiet"]).unwrap();
            let fixture = Self(root);
            fixture.commit();
            fixture
        }

        fn commit(&self) {
            git(
                &self.0,
                &[
                    "-c",
                    "user.name=Test",
                    "-c",
                    "user.email=test@example.invalid",
                    "-c",
                    "core.hooksPath=/dev/null",
                    "commit",
                    "--quiet",
                    "--allow-empty",
                    "-m",
                    "fixture",
                ],
            )
            .unwrap();
        }
    }

    impl Drop for GitFixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn staged_sources_and_baseline_ignore_unstaged_fixes() {
        let fixture = GitFixture::new();
        let root = &fixture.0;
        fs::create_dir_all(root.join("crates/web/src")).unwrap();
        fs::create_dir_all(root.join("xtask")).unwrap();
        let path = "crates/web/src/example.rs";
        fs::write(root.join(path), r#"fn example() { let _ = "13px"; }"#).unwrap();
        fs::write(root.join(ALLOWLIST), "{}").unwrap();
        git(root, &["add", "."]).unwrap();
        fs::write(
            root.join(path),
            r#"fn example() { let _ = "var(--space-3)"; }"#,
        )
        .unwrap();
        fs::write(root.join(ALLOWLIST), "not the staged baseline").unwrap();
        assert_eq!(
            scan(&read(root, path, true).unwrap())
                .counts
                .values()
                .sum::<usize>(),
            1
        );
        assert!(scan(&read(root, path, false).unwrap()).counts.is_empty());
        assert_eq!(read(root, ALLOWLIST, true).unwrap(), "{}");
        git(root, &["add", path]).unwrap();
        assert!(scan(&read(root, path, true).unwrap()).counts.is_empty());
    }

    #[test]
    fn ratchet_bootstraps_only_when_file_is_absent_at_a_valid_commit() {
        let fixture = GitFixture::new();
        let root = &fixture.0;
        assert!(previous_baseline(root, "HEAD").unwrap().is_none());
        assert!(previous_baseline(root, "refs/heads/does-not-exist").is_err());
        fs::create_dir(root.join("xtask")).unwrap();
        fs::write(root.join(ALLOWLIST), r#"{"file:rule:value":1}"#).unwrap();
        git(root, &["add", ALLOWLIST]).unwrap();
        fixture.commit();
        assert_eq!(
            previous_baseline(root, "HEAD").unwrap().unwrap()["file:rule:value"],
            1
        );
        assert!(previous_baseline(root, "HEAD~1").unwrap().is_none());
        fs::write(root.join(ALLOWLIST), "broken JSON").unwrap();
        git(root, &["add", ALLOWLIST]).unwrap();
        fixture.commit();
        assert!(previous_baseline(root, "HEAD").is_err());
    }

    #[test]
    fn raw_values_in_macros_formats_classes_and_raw_strings() {
        let scan = scan(
            r##"fn view() { view! { <div class="p-[13px] text-[#abcdef]"/> } let x = format!("left:{}px;color:rgb(1,2,3)", 4); let y = r#"padding:2rem"#; }"##,
        );
        assert_eq!(scan.counts.values().sum::<usize>(), 3);
    }

    #[test]
    fn comments_identifiers_and_token_literals_are_not_raw_values() {
        let scan = scan(
            r##"// style="padding:3px" #abcdef
            /* nested /* 4px */ #123 */
            /// 16px is an example in documentation.
            fn rgb() { let x = "var(--space-4)"; let c = "card card-active"; let id = "#abcdefghi"; }
        "##,
        );
        assert!(scan.counts.is_empty());
        assert!(scan.inline.is_empty());
    }

    #[test]
    fn ordinary_blocks_starting_with_doc_are_still_scanned() {
        assert_eq!(
            scan(r#"fn example() { doc = "copy"; let _ = "13px"; }"#)
                .counts
                .values()
                .sum::<usize>(),
            1
        );
        assert_eq!(
            scan(r#"fn example() { let _ = "\u{23}abcdef"; }"#)
                .counts
                .values()
                .sum::<usize>(),
            1
        );
    }

    #[test]
    fn captures_static_style_and_property_bindings_for_shared_css_policy() {
        let scan = scan(
            r##"view! { <div style="padding:var(--space-2);color:red" style:margin-top="3px"/> }"##,
        );
        assert!(scan.counts.is_empty());
        assert_eq!(scan.inline.len(), 3);
        assert_eq!(scan.inline[2]["property"], "margin-top");
        assert_eq!(scan.inline[2]["value"], "3px");
    }

    #[test]
    fn dynamic_css_and_format_injection_cannot_silently_bypass_the_lint() {
        let scan =
            scan(r#"view! { <div style=move || format!("padding:{}px", n) style:color=color/> }"#);
        assert_eq!(scan.counts.values().sum::<usize>(), 3);
        assert_eq!(
            scan.counts
                .keys()
                .filter(|k| k.contains("rust-dynamic-style"))
                .count(),
            2
        );
    }

    #[test]
    fn detects_hex_functions_and_interpolated_units() {
        for value in [
            "#fff",
            "#abcd",
            "#aabbcc",
            "#aabbccdd",
            "hsl(1 2% 3%)",
            "RGB(1,2,3)",
            "{}px",
            "{x:.1}px",
            "-.5rem",
        ] {
            assert!(raw_value(value), "{value}");
        }
        for value in [
            "var(--text)",
            "example",
            "#abcde",
            "#abcdefg",
            "100%",
            "none",
        ] {
            assert!(!raw_value(value), "{value}");
        }
    }

    #[test]
    fn exact_match_stale_zero_entries_and_no_growth() {
        let counts =
            |entries: &[(&str, usize)]| entries.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        let one = counts(&[("file:rule:value", 1)]);
        let two = counts(&[("file:rule:value", 2)]);
        assert!(ratchet(&one, &one, Some(&two)).is_empty());
        assert!(!ratchet(&Counts::new(), &one, None).is_empty());
        assert!(!ratchet(&one, &Counts::new(), None).is_empty());
        assert!(!ratchet(&two, &two, Some(&one)).is_empty());
        assert!(!ratchet(&Counts::new(), &counts(&[("x", 0)]), None).is_empty());
        let swapped = counts(&[("file:rule:another-value", 1)]);
        assert!(!ratchet(&swapped, &swapped, Some(&one)).is_empty());
    }
}
