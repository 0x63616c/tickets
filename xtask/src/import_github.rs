//! Local-only, restartable GitHub migration. Never log credentials or source bodies.
use reqwest::{Method, Url, blocking::Client};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    error::Error,
    process::Command,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub struct Api {
    client: Client,
    base: String,
    token: String,
    github: bool,
}
impl Api {
    pub fn new(base: &str, token: String, github: bool) -> Result<Self> {
        let url = Url::parse(base)?;
        if !github
            && (url.scheme() != "http"
                || !matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")))
        {
            return Err("Tickets URL must use HTTP on loopback; production import requires separate approval".into());
        }
        if url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err("expected a base URL without path or credentials".into());
        }
        Ok(Self {
            client: Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(60))
                .user_agent("tickets-import")
                .build()?,
            base: base.trim_end_matches('/').into(),
            token,
            github,
        })
    }
    pub fn call(&self, method: Method, path: &str, body: Value) -> Result<Value> {
        for attempt in 0..4 {
            let mut request = self
                .client
                .request(method.clone(), format!("{}{path}", self.base))
                .bearer_auth(&self.token);
            if self.github {
                request = request
                    .header("Accept", "application/vnd.github+json")
                    .header("X-GitHub-Api-Version", "2022-11-28");
            }
            if !body.is_null() {
                request = request.json(&body);
            }
            let response = request
                .send()
                .map_err(|_| "HTTP transport failed; rerun to reconcile")?;
            let status = response.status();
            if self.github && (status.as_u16() == 429 || status.as_u16() == 403) && attempt < 3 {
                let headers = response.headers();
                let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
                let delay = headers
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok()?.parse::<u64>().ok())
                    .or_else(|| {
                        headers
                            .get("x-ratelimit-reset")
                            .and_then(|v| v.to_str().ok()?.parse::<u64>().ok())
                            .map(|reset| reset.saturating_sub(now) + 1)
                    });
                if let Some(delay) = delay.filter(|d| *d <= 300) {
                    eprintln!("GitHub rate limit: waiting {delay}s");
                    for _ in 0..delay {
                        thread::sleep(Duration::from_secs(1));
                    }
                    continue;
                }
            }
            if !status.is_success() {
                return Err(format!(
                    "{} request failed: HTTP {}",
                    if self.github { "GitHub" } else { "Tickets" },
                    status.as_u16()
                )
                .into());
            }
            return response.json().map_err(|_| "invalid JSON response".into());
        }
        Err("GitHub retry limit exceeded".into())
    }
    pub fn list(&self, path: &str) -> Result<Vec<Value>> {
        let mut result = Vec::new();
        let mut cursor = String::new();
        let mut page = 1;
        loop {
            let sep = if path.contains('?') { '&' } else { '?' };
            let query = if self.github {
                format!("{path}{sep}per_page=100&page={page}")
            } else if cursor.is_empty() {
                format!("{path}{sep}limit=100")
            } else {
                format!("{path}{sep}limit=100&cursor={cursor}")
            };
            let value = self.call(Method::GET, &query, Value::Null)?;
            let items = if self.github {
                value.as_array()
            } else {
                value["data"].as_array()
            }
            .ok_or("invalid list")?;
            result.extend(items.iter().cloned());
            if self.github {
                if items.is_empty() {
                    break;
                }
                page += 1;
            } else if let Some(next) = value["next_cursor"].as_str() {
                if next == cursor {
                    return Err("repeated cursor".into());
                }
                cursor = next.into();
            } else {
                break;
            }
        }
        Ok(result)
    }
}
fn id(v: &Value) -> Result<i64> {
    v["id"].as_i64().ok_or_else(|| "missing ID".into())
}
fn number(v: &Value) -> Result<u64> {
    v["number"]
        .as_u64()
        .ok_or_else(|| "missing issue number".into())
}
fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key]
        .as_str()
        .ok_or_else(|| format!("missing {key}").into())
}
fn blockers(body: &str) -> BTreeSet<u64> {
    body.split("Blocked by #")
        .skip(1)
        .filter_map(|s| {
            s.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .ok()
        })
        .collect()
}
fn marker(repo: &str, comment: &Value) -> Result<String> {
    Ok(format!(
        "<!-- tickets-import:github:{repo}:comment:{} -->",
        id(comment)?
    ))
}
fn comment_body(repo: &str, comment: &Value) -> Result<String> {
    Ok(format!(
        "{}\n\n{}",
        marker(repo, comment)?,
        comment["body"].as_str().unwrap_or("")
    ))
}

fn label_set(labels: &Value, prefix: &str) -> Result<BTreeSet<(String, String)>> {
    labels
        .as_array()
        .ok_or("missing labels")?
        .iter()
        .map(|l| {
            Ok((
                text(l, "name")?.to_owned(),
                format!("{prefix}{}", text(l, "color")?),
            ))
        })
        .collect()
}

pub fn cli(args: &[String]) -> Result<()> {
    let mut repo = None;
    let mut project = None;
    let mut url = "http://127.0.0.1:8787".to_owned();
    let mut dry = false;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--repo" => repo = Some(args.next().ok_or("missing repo")?.clone()),
            "--project" => project = Some(args.next().ok_or("missing project")?.clone()),
            "--url" => url = args.next().ok_or("missing URL")?.clone(),
            "--dry-run" => dry = true,
            _ => return Err(format!("unknown import argument: {arg}").into()),
        }
    }
    let repo = repo.ok_or("--repo required")?.to_ascii_lowercase();
    let project = project.ok_or("--project required")?;
    if repo.split('/').count() != 2
        || repo.split('/').any(|s| {
            s.is_empty()
                || s == "."
                || s == ".."
                || !s
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        })
    {
        return Err("expected owner/repo".into());
    }
    if project.is_empty() || project.len() > 10 || !project.bytes().all(|c| c.is_ascii_uppercase())
    {
        return Err("project key must be 1–10 uppercase letters".into());
    }
    let tickets = Api::new(
        &url,
        env::var("TICKETS_TOKEN").map_err(|_| "TICKETS_TOKEN required")?,
        false,
    )?;
    let token = match env::var("GITHUB_TOKEN") {
        Ok(t) => t,
        Err(_) => {
            let output = Command::new("gh").args(["auth", "token"]).output()?;
            if !output.status.success() {
                return Err("GitHub authentication unavailable".into());
            }
            String::from_utf8(output.stdout)?.trim().to_owned()
        }
    };
    let github = Api::new("https://api.github.com", token, true)?;
    import(&github, &tickets, &repo, &project, dry)
}

pub fn import(gh: &Api, api: &Api, repo: &str, key: &str, dry: bool) -> Result<()> {
    let issues: Vec<_> = gh
        .list(&format!(
            "/repos/{repo}/issues?state=all&sort=created&direction=asc"
        ))?
        .into_iter()
        .filter(|i| i.get("pull_request").is_none())
        .collect();
    let mut comments = BTreeMap::new();
    for issue in &issues {
        let n = number(issue)?;
        comments.insert(n, gh.list(&format!("/repos/{repo}/issues/{n}/comments"))?);
    }
    let open = issues.iter().filter(|i| i["state"] == "open").count();
    let closed = issues.len() - open;
    let projects = api.list("/api/v1/projects")?;
    let project = projects.iter().find(|p| p["key_prefix"] == key);
    let existing_labels = api.list("/api/v1/labels")?;
    let mut labels: BTreeMap<String, Value> = existing_labels
        .into_iter()
        .map(|l| (l["name"].as_str().unwrap_or_default().to_owned(), l))
        .collect();
    let mut wanted_labels = BTreeMap::new();
    let mut edges = BTreeSet::new();
    let numbers: BTreeSet<_> = issues.iter().map(number).collect::<Result<_>>()?;
    if numbers.len() != issues.len() {
        return Err("duplicate source issue numbers; retry a stable source snapshot".into());
    }
    for issue in &issues {
        text(issue, "title")?;
        text(issue, "html_url")?;
        if !matches!(text(issue, "state")?, "open" | "closed") {
            return Err("unknown GitHub state".into());
        }
        let comment_ids = comments[&number(issue)?]
            .iter()
            .map(id)
            .collect::<Result<BTreeSet<_>>>()?;
        if comment_ids.len() != comments[&number(issue)?].len() {
            return Err("duplicate source comment IDs".into());
        }
        for label in issue["labels"].as_array().ok_or("missing labels")? {
            wanted_labels.insert(
                text(label, "name")?.to_owned(),
                format!("#{}", text(label, "color")?),
            );
        }
        for blocker in blockers(issue["body"].as_str().unwrap_or("")) {
            if !numbers.contains(&blocker) {
                return Err(
                    format!("unresolved dependency #{blocker} -> #{}", number(issue)?).into(),
                );
            }
            edges.insert((blocker, number(issue)?));
        }
        println!(
            "{} issue #{}: {} labels, {} comments, {}",
            if dry { "PLAN" } else { "Import" },
            number(issue)?,
            issue["labels"].as_array().unwrap().len(),
            comments[&number(issue)?].len(),
            text(issue, "state")?
        );
    }
    let graph_edges = edges
        .iter()
        .map(|(a, b)| {
            Ok(tickets_core::domain::Dependency {
                blocker_id: i64::try_from(*a)?,
                blocked_id: i64::try_from(*b)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    tickets_core::dependency::DependencyGraph::new(graph_edges)
        .map_err(|e| format!("invalid source dependency graph: {e:?}"))?;
    if dry {
        println!(
            "DRY RUN: project {key} ({}), {open} open, {closed} closed, {} labels, {} dependencies; no writes",
            if project.is_some() {
                "existing"
            } else {
                "create"
            },
            wanted_labels.len(),
            edges.len()
        );
        return Ok(());
    }
    let project_id = match project {
        Some(p) => id(p)?,
        None => {
            let name = if repo == "0x63616c/voxelite" {
                "Quantum"
            } else {
                repo.split('/').next_back().unwrap()
            };
            id(&api.call(
                Method::POST,
                "/api/v1/projects",
                json!({
                    "name": name, "key_prefix": key, "color": "#789abc"
                }),
            )?)?
        }
    };
    for (name, color) in &wanted_labels {
        let label = if let Some(old) = labels.get(name) {
            if old["color"] == *color {
                old.clone()
            } else {
                api.call(
                    Method::PATCH,
                    &format!("/api/v1/labels/{}", id(old)?),
                    json!({"color":color}),
                )?
            }
        } else {
            api.call(
                Method::POST,
                "/api/v1/labels",
                json!({"name":name,"color":color}),
            )?
        };
        labels.insert(name.clone(), label);
    }
    let mut tickets = BTreeMap::new();
    for issue in &issues {
        let n = number(issue)?;
        let mut url = Url::parse("http://localhost/api/v1/tickets/by-external")?;
        url.query_pairs_mut()
            .append_pair("id", &format!("github:{repo}:{n}"));
        let path = format!("{}?{}", url.path(), url.query().unwrap());
        let ticket = api.call(
            Method::PUT,
            &path,
            json!({
                "project_id": project_id,
                "title": text(issue,"title")?,
                "body": issue["body"].as_str().unwrap_or(""),
                "status": if issue["state"] == "closed" { "done" } else { "todo" },
                "external_url": text(issue,"html_url")?
            }),
        )?;
        let tid = id(&ticket)?;
        let desired: BTreeSet<_> = issue["labels"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| id(&labels[l["name"].as_str().unwrap()]))
            .collect::<Result<_>>()?;
        let current: BTreeSet<_> = ticket["labels"]
            .as_array()
            .ok_or("missing ticket labels")?
            .iter()
            .map(id)
            .collect::<Result<_>>()?;
        for label in desired.difference(&current) {
            api.call(
                Method::POST,
                &format!("/api/v1/tickets/{tid}/labels/{label}"),
                Value::Null,
            )?;
        }
        for label in current.difference(&desired) {
            api.call(
                Method::DELETE,
                &format!("/api/v1/tickets/{tid}/labels/{label}"),
                Value::Null,
            )?;
        }
        let existing = api.list(&format!("/api/v1/tickets/{tid}/comments"))?;
        for comment in &comments[&n] {
            let marker = marker(repo, comment)?;
            let body = comment_body(repo, comment)?;
            let matches: Vec<_> = existing
                .iter()
                .filter(|c| c["body"].as_str().is_some_and(|s| s.starts_with(&marker)))
                .collect();
            if matches.len() > 1 {
                return Err("duplicate imported comment markers".into());
            }
            if let Some(old) = matches.first() {
                if old["body"] != body {
                    api.call(
                        Method::PATCH,
                        &format!("/api/v1/comments/{}", id(old)?),
                        json!({"body":body}),
                    )?;
                }
            } else {
                api.call(
                    Method::POST,
                    &format!("/api/v1/tickets/{tid}/comments"),
                    json!({"body":body}),
                )?;
            }
        }
        tickets.insert(n, ticket);
    }
    let imported_ids: BTreeSet<_> = tickets.values().map(id).collect::<Result<_>>()?;
    // Remove all obsolete edges before adding any, allowing a source edge reversal.
    let mut additions = Vec::new();
    for (n, ticket) in &tickets {
        let tid = id(ticket)?;
        let desired: BTreeSet<_> = edges
            .iter()
            .filter(|(a, _)| a == n)
            .map(|(_, b)| id(&tickets[b]))
            .collect::<Result<_>>()?;
        let current: BTreeSet<_> = ticket["blocks"]
            .as_array()
            .ok_or("missing blocks")?
            .iter()
            .filter_map(Value::as_i64)
            .collect();
        for target in current
            .difference(&desired)
            .filter(|id| imported_ids.contains(id))
        {
            api.call(
                Method::DELETE,
                &format!("/api/v1/tickets/{tid}/dependencies/{target}"),
                Value::Null,
            )?;
        }
        for target in desired.difference(&current) {
            additions.push((tid, *target));
        }
    }
    for (tid, target) in additions {
        api.call(
            Method::POST,
            &format!("/api/v1/tickets/{tid}/dependencies/{target}"),
            Value::Null,
        )?;
    }
    // Read back authoritative state; counts alone would miss corrupt fields/links.
    let actual = api.list(&format!("/api/v1/tickets?project={project_id}"))?;
    let prefix = format!("github:{repo}:");
    let imported: Vec<_> = actual
        .iter()
        .filter(|t| {
            t["external_id"]
                .as_str()
                .is_some_and(|s| s.starts_with(&prefix))
        })
        .collect();
    let mut gaps = imported.len().abs_diff(issues.len());
    let mut actual_edges = 0;
    let mut actual_comments = 0;
    let mut actual_labels = BTreeSet::new();
    for issue in &issues {
        let n = number(issue)?;
        let ticket = imported
            .iter()
            .find(|t| t["external_id"] == format!("{prefix}{n}"));
        let Some(ticket) = ticket else {
            gaps += 1;
            continue;
        };
        for field in ["title", "body"] {
            let expected = issue[field].as_str().unwrap_or("");
            if ticket[field] != expected {
                gaps += 1;
            }
        }
        if ticket["external_url"] != issue["html_url"]
            || ticket["status"]
                != if issue["state"] == "closed" {
                    "done"
                } else {
                    "todo"
                }
        {
            gaps += 1;
        }
        let expected = label_set(&issue["labels"], "#")?;
        let got = label_set(&ticket["labels"], "")?;
        for label in &got {
            if wanted_labels.contains_key(&label.0) {
                actual_labels.insert(label.clone());
            }
        }
        if expected != got {
            gaps += 1;
        }
        let expected_edges: BTreeSet<_> = edges
            .iter()
            .filter(|(a, _)| *a == n)
            .map(|(_, b)| id(&tickets[b]))
            .collect::<Result<_>>()?;
        let got_edges: BTreeSet<_> = ticket["blocks"]
            .as_array()
            .ok_or("missing blocks")?
            .iter()
            .filter_map(Value::as_i64)
            .filter(|id| imported_ids.contains(id))
            .collect();
        if expected_edges != got_edges {
            gaps += 1;
        }
        for (_, b) in edges.iter().filter(|(a, _)| *a == n) {
            let target = id(&tickets[b])?;
            if ticket["blocks"]
                .as_array()
                .unwrap()
                .contains(&json!(target))
            {
                actual_edges += 1;
            } else {
                gaps += 1;
            }
        }
        let got = api.list(&format!("/api/v1/tickets/{}/comments", id(ticket)?))?;
        for c in &comments[&n] {
            let body = comment_body(repo, c)?;
            if got.iter().filter(|c| c["body"] == body).count() == 1 {
                actual_comments += 1;
            } else {
                gaps += 1;
            }
        }
    }
    let actual_closed = imported.iter().filter(|t| t["status"] == "done").count();
    println!(
        "Reconciliation: GitHub open/closed {open}/{closed}; Tickets {}/{}; labels {}/{}; dependencies {actual_edges}/{}; comments {actual_comments}/{}; gaps {gaps}",
        imported.len() - actual_closed,
        actual_closed,
        actual_labels.len(),
        wanted_labels.len(),
        edges.len(),
        comments.values().map(Vec::len).sum::<usize>()
    );
    if gaps != 0 {
        return Err("import reconciliation has gaps".into());
    }
    println!(
        "GitHub Projects columns deferred: token project scope unavailable; blocked state is derived from dependency completion."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dependency_references_are_unique_and_directed() {
        assert_eq!(
            blockers("Blocked by #12\n- Blocked by #7\nBlocked by #12\nBlocked by #oops"),
            BTreeSet::from([7, 12])
        );
    }
}
