//! Execute documented curl argv directly, without a shell or ambient credentials.
use serde_json::Value;
use std::{error::Error, fs, path::Path, process::Command};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

struct Example {
    line: usize,
    args: Vec<String>,
    expected: Value,
}

fn examples(source: &str) -> Result<Vec<Example>> {
    let mut result = Vec::new();
    let mut fence = false;
    let mut expected = None;
    let mut command = String::new();
    let mut start = 0;
    for (index, line) in source.lines().enumerate() {
        if line == "```sh" {
            fence = true;
            continue;
        }
        if line == "```" {
            if !command.is_empty() || expected.is_some() {
                return Err("unfinished curl example".into());
            }
            fence = false;
            continue;
        }
        if !fence || line.trim().is_empty() {
            continue;
        }
        if let Some(json) = line.strip_prefix("# expect ") {
            if expected.is_some() || !command.is_empty() {
                return Err("expectation without a curl command".into());
            }
            expected = Some(serde_json::from_str(json)?);
            start = index + 1;
            continue;
        }
        let continued = line.ends_with('\\');
        command.push_str(line.strip_suffix('\\').unwrap_or(line));
        command.push(' ');
        if continued {
            continue;
        }
        let mut args = shlex::split(&command).ok_or("invalid curl quoting")?;
        if args.first().map(String::as_str) != Some("curl") {
            return Err(format!("line {}: expected curl", index + 1).into());
        }
        args.remove(0);
        result.push(Example {
            line: start,
            args,
            expected: expected.take().ok_or("curl has no expectation")?,
        });
        command.clear();
    }
    if fence || result.is_empty() {
        return Err("missing or unclosed curl examples".into());
    }
    Ok(result)
}

fn subset(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Object(a), Value::Object(e)) => e
            .iter()
            .all(|(key, value)| a.get(key).is_some_and(|a| subset(a, value))),
        (Value::Array(a), Value::Array(e)) => {
            a.len() == e.len() && a.iter().zip(e).all(|(a, e)| subset(a, e))
        }
        _ => actual == expected,
    }
}

pub fn run(root: &Path, token: &str) -> Result<()> {
    let source = fs::read_to_string(root.join(".agents/skills/tickets/SKILL.md"))?;
    let examples = examples(&source)?;
    for example in &examples {
        // Only these two environment substitutions are supported by the walkthrough.
        // Expand after tokenization so credentials cannot become command syntax.
        let args: Vec<_> = example
            .args
            .iter()
            .map(|arg| {
                arg.replace("$TICKETS_URL", "http://127.0.0.1:8795")
                    .replace("$TICKETS_TOKEN", token)
            })
            .collect();
        let output = Command::new("curl")
            .args(&args)
            .args(["--max-time", "10", "--write-out", "\n%{http_code}"])
            .output()?;
        let fail = || format!("skill example at line {} failed", example.line);
        if !output.status.success() {
            return Err(fail().into());
        }
        let stdout = String::from_utf8(output.stdout)?;
        let (body, status) = stdout.rsplit_once('\n').ok_or_else(fail)?;
        let status: u16 = status.parse()?;
        // -i is useful for the documented error example. Strip only HTTP headers.
        let body = if args.iter().any(|arg| arg == "-i") {
            body.split_once("\r\n\r\n").ok_or_else(fail)?.1
        } else {
            body
        };
        let value: Value = serde_json::from_str(body).map_err(|_| fail())?;
        if example.expected["status"].as_u64() != Some(u64::from(status))
            || example
                .expected
                .get("body")
                .is_some_and(|e| !subset(&value, e))
            || (status >= 400
                && (!value["error"]["code"].is_string() || !value["error"]["message"].is_string()))
        {
            // Do not print responses or argv: authentication responses may contain secrets.
            return Err(format!("{} (HTTP {status}, status/body mismatch)", fail()).into());
        }
        println!("PASS: skill line {} (HTTP {status})", example.line);
    }
    println!("PASS: all {} documented curl examples", examples.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extraction_preserves_json_and_continuations() {
        let e = examples("```sh\n# expect {\"status\":201}\ncurl -sS \\\n  -d '{\"title\":\"two words\"}' \"$TICKETS_URL/api/v1/tickets\"\n```\n").unwrap();
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].args[2], "{\"title\":\"two words\"}");
        assert_eq!(e[0].args[3], "$TICKETS_URL/api/v1/tickets");
        assert!(examples("```sh\ncurl example\n```\n").is_err());
        assert!(examples("```sh\n# expect {\"status\":200}\n```\n").is_err());
    }

    #[test]
    fn assertions_detect_missing_fields_and_extra_rows() {
        assert!(subset(
            &serde_json::json!({"data":[{"id":1,"title":"x"}]}),
            &serde_json::json!({"data":[{"id":1}]})
        ));
        assert!(!subset(
            &serde_json::json!({"data":[{"id":1},{"id":2}]}),
            &serde_json::json!({"data":[{"id":1}]})
        ));
        assert!(!subset(
            &serde_json::json!({}),
            &serde_json::json!({"id":1})
        ));
    }
}
