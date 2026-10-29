//! PR webhook acceptance using the signed local mirror-check lifecycle.
use super::{design::Browser, import_github::Api, mirror_check::send};
use reqwest::Method;
use serde_json::{Value, json};
use std::path::Path;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn payload(key: &str) -> Value {
    json!({"action":"opened","repository":{"id":999,"full_name":"0x63616c/Any-Repository","owner":{"login":"0X63616C"}},"pull_request":{"number":32,"html_url":"https://github.com/0x63616c/Any-Repository/pull/32","title":format!("Link pull requests [{key}]"),"body":format!("Closes [{key}](https://tickets.worldwidewebb.co/t/{key})"),"state":"open","draft":false,"merged":false,"head":{"sha":"abc123"},"updated_at":"2026-10-29T10:00:00Z"}})
}
pub fn check(root: &Path, password: &str, api: &Api) -> Result<()> {
    send("ping", json!({}), true, 200)?;
    send("pull_request", json!({}), true, 400)?;
    let ticket=api.call(Method::POST,"/api/v1/tickets",json!({"project_id":1,"title":"Link pull requests to tickets","body":"Pull requests appear here with their current state.","status":"todo"}))?;
    let key = ticket["key"].as_str().unwrap();
    let path = format!("/api/v1/tickets/{key}");
    let mut p = payload(key);
    send("pull_request", p.clone(), false, 401)?;
    let mut unknown = p.clone();
    unknown["repository"]["owner"]["login"] = json!("someone-else");
    send("pull_request", unknown, true, 202)?;
    send("pull_request", payload("UNKNOWN-999"), true, 202)?;
    p["pull_request"]["updated_at"] = json!("2026-10-29T11:00:00Z");
    send("pull_request", p.clone(), true, 204)?;
    send("pull_request", p.clone(), true, 202)?;
    let get = || api.call(Method::GET, &path, Value::Null);
    let linked = get()?;
    assert_eq!(linked["status"], "in_review");
    assert_eq!(linked["pull_requests"].as_array().unwrap().len(), 1);
    assert_eq!(linked["pull_requests"][0]["head_sha"], "abc123");
    // Repositories are accepted by their configured owner alone.
    let output = root.join("docs/screenshots/32");
    std::fs::create_dir_all(&output)?;
    let before = std::env::var_os("TICKETS_BEFORE_ASSETS").is_some();
    let mut b = Browser::start_named(root, "pr-browser")?;
    b.go(&format!(
        "http://127.0.0.1:8795/login?next=/ticket%3Fid%3D{}",
        ticket["id"]
    ))?;
    b.wait("!!document.querySelector('#username')")?;
    b.text("#username", "fixture")?;
    b.text("#password", password)?;
    b.click("button[type=submit]")?;
    b.wait(
        "!!document.querySelector('.ticket-detail h1') && !!document.querySelector('.comment')",
    )?;
    if !before {
        b.wait("!!document.querySelector('[aria-label=\"Pull requests\"] a') && document.querySelector('[aria-label=\"Pull requests\"]').innerText.includes('Open · Closes ticket')")?;
        b.eval("document.querySelector('[aria-label=\"Pull requests\"] a').focus();true")?;
    }
    let prefix = if before { "before" } else { "after" };
    b.screenshot(&output.join(format!("{prefix}-desktop.png")))?;
    b.viewport(390, 900)?;
    b.eval("document.querySelector('.dependencies').scrollIntoView();true")?;
    b.screenshot(&output.join(format!("{prefix}-mobile.png")))?;
    b.eval("if(document.documentElement.scrollWidth>innerWidth)throw Error('Horizontal overflow');true")?;
    p["action"] = json!("closed");
    p["pull_request"]["state"] = json!("closed");
    p["pull_request"]["merged"] = json!(true);
    p["pull_request"]["updated_at"] = json!("2026-10-29T12:00:00Z");
    send("pull_request", p.clone(), true, 204)?;
    assert_eq!(get()?["status"], "done");
    send("pull_request", payload(key), true, 202)?;
    assert_eq!(get()?["status"], "done");
    p["action"] = json!("edited");
    p["pull_request"]["title"] = json!("No ticket");
    p["pull_request"]["body"] = Value::Null;
    p["pull_request"]["updated_at"] = json!("2026-10-29T13:00:00Z");
    send("pull_request", p, true, 202)?;
    assert!(get()?["pull_requests"].as_array().unwrap().is_empty());
    send("pull_request", payload(key), true, 202)?;
    assert!(get()?["pull_requests"].as_array().unwrap().is_empty());
    println!(
        "PR check passed: HMAC, owner allowlist, unknown keys, linking, API key/head SHA, merge, stale replay, unlink and dark desktop/mobile UI"
    );
    Ok(())
}
