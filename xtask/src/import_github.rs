//! Explicitly gated, restartable GitHub migration. Never log credentials or source bodies.
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
        Self::with_remote(base, token, github, false)
    }
    pub fn with_remote(
        base: &str,
        token: String,
        github: bool,
        allow_remote: bool,
    ) -> Result<Self> {
        let url = Url::parse(base)?;
        if !github
            && (url.scheme() != "http"
                || !matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")))
            && !(allow_remote && url.scheme() == "https" && url.host_str().is_some())
        {
            return Err(
                "Tickets URL must use HTTP on loopback, or HTTPS with --allow-remote".into(),
            );
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
                .redirect(if github {
                    reqwest::redirect::Policy::limited(5)
                } else {
                    reqwest::redirect::Policy::none()
                })
                .timeout(Duration::from_secs(60))
                .user_agent("tickets-import")
                .build()?,
            base: base.trim_end_matches('/').into(),
            token,
            github,
        })
    }
    pub fn call(&self, method: Method, path: &str, body: Value) -> Result<Value> {
        // POST creates (especially comments) and DELETE/link operations are not
        // replay-safe. A failed response may follow a committed write: rerun the
        // importer to reconcile rather than blindly repeating those operations.
        let safe = matches!(method, Method::GET | Method::PUT | Method::PATCH);
        for attempt in 0..4 {
            let mut request = self
                .client
                .request(method.clone(), format!("{}{path}", self.base))
                .bearer_auth(&self.token);
            if self.github {
                request = request
                    .header("Accept", "application/vnd.github+json")
                    .header("X-GitHub-Api-Version", "2026-03-10");
            }
            if !body.is_null() {
                request = request.json(&body);
            }
            let response = match request.send() {
                Ok(response) => response,
                Err(error)
                    if safe
                        && attempt < 3
                        && (error.is_connect() || error.is_timeout() || error.is_request()) =>
                {
                    thread::sleep(Duration::from_secs(1 << attempt));
                    continue;
                }
                Err(_) => return Err("HTTP transport failed; rerun to reconcile".into()),
            };
            let status = response.status();
            if safe
                && attempt < 3
                && (status.as_u16() == 429
                    || status.is_server_error()
                    || (self.github && status.as_u16() == 403))
            {
                let headers = response.headers();
                let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
                let delay = headers
                    .get("retry-after")
                    .and_then(|v| retry_after(v.to_str().ok()?, SystemTime::now()))
                    .or_else(|| {
                        headers
                            .get("x-ratelimit-reset")
                            .and_then(|v| v.to_str().ok()?.parse::<u64>().ok())
                            .map(|reset| reset.saturating_sub(now).saturating_add(1))
                    });
                let delay = delay.unwrap_or(1 << attempt).max(1 << attempt);
                if delay <= 300 {
                    eprintln!("API retry: waiting {delay}s");
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
        Err("API retry limit exceeded".into())
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
fn retry_after(value: &str, now: SystemTime) -> Option<u64> {
    value.parse().ok().or_else(|| {
        httpdate::parse_http_date(value).ok().map(|date| {
            date.duration_since(now)
                .unwrap_or_default()
                .as_secs()
                .saturating_add(1)
        })
    })
}
use tickets_core::github_mirror::{Comment, Issue, identity_repo};
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
    body.split(['\n', '\r'])
        .flat_map(|line| {
            line.split("Blocked by #")
                .skip(1)
                .chain(line.split("Depends on #").skip(1))
        })
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
    Ok(serde_json::from_value::<Comment>(comment.clone())?.marker(repo))
}
fn comment_body(repo: &str, comment: &Value) -> Result<String> {
    Ok(serde_json::from_value::<Comment>(comment.clone())?.body(repo))
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

#[derive(Clone, Debug)]
pub struct Options {
    pub state: String,
    pub since: Option<String>,
    pub exclude: BTreeSet<u64>,
    pub dry: bool,
    pub close_github: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            state: "open".into(),
            since: None,
            exclude: BTreeSet::new(),
            dry: false,
            close_github: false,
        }
    }
}
fn selected(issue: &Value, options: &Options) -> bool {
    issue.get("pull_request").is_none()
        && issue["number"]
            .as_u64()
            .is_some_and(|n| !options.exclude.contains(&n))
        && (options.state == "all" || issue["state"] == options.state)
        && (issue["state"] != "closed"
            || options.since.as_deref().is_none_or(|since| {
                issue["closed_at"]
                    .as_str()
                    .is_some_and(|date| date >= since)
            }))
}
/// Rewrite local shorthand only; leave URLs, Markdown destinations and code alone.
fn rewrite_refs(body: &str, repo: &str, keys: &BTreeMap<u64, String>) -> String {
    let mut result = String::new();
    let chars: Vec<char> = body.chars().collect();
    let mut i = 0;
    let mut code_ticks = 0;
    while i < chars.len() {
        if chars[i] == '`' {
            let start = i;
            while i < chars.len() && chars[i] == '`' {
                i += 1;
            }
            let count = i - start;
            if code_ticks == 0 {
                code_ticks = count;
            } else if code_ticks == count {
                code_ticks = 0;
            }
            result.extend(chars[start..i].iter());
            continue;
        }
        if code_ticks == 0
            && chars[i] == '#'
            && (i == 0 || chars[i - 1].is_whitespace() || "([,:".contains(chars[i - 1]))
            && !(i >= 2 && chars[i - 1] == '(' && chars[i - 2] == ']')
        {
            let start = i + 1;
            let mut end = start;
            while end < chars.len() && chars[end].is_ascii_digit() {
                end += 1;
            }
            if end < chars.len() && (chars[end].is_alphabetic() || chars[end] == '_') {
                result.push(chars[i]);
                i += 1;
                continue;
            }
            if let Ok(n) = chars[start..end].iter().collect::<String>().parse::<u64>() {
                result.push_str(
                    &keys
                        .get(&n)
                        .cloned()
                        .unwrap_or_else(|| format!("https://github.com/{repo}/issues/{n}")),
                );
                i = end;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    result
}
fn same_repo(issue: &Value, repo: &str) -> bool {
    issue["html_url"]
        .as_str()
        .is_some_and(|url| url.starts_with(&format!("https://github.com/{repo}/issues/")))
}
fn close_message(base: &str, ticket: &Value) -> Result<String> {
    Ok(format!(
        "Moved to Tickets: {base}/ticket?id={} ({}). Tickets is now the system of record; this issue is closed.",
        id(ticket)?,
        text(ticket, "key")?
    ))
}
fn already_moved(comments: &[Value], message: &str) -> bool {
    let Some((destination, _)) = message.split_once(" (") else {
        return false;
    };
    comments.iter().any(|comment| {
        comment["body"].as_str().is_some_and(|body| {
            body.strip_prefix(destination).is_some_and(|rest| {
                rest.starts_with(" (")
                    && rest
                        .ends_with("). Tickets is now the system of record; this issue is closed.")
            })
        })
    })
}

fn valid_date(date: &str) -> bool {
    if date.len() != 10
        || !date.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
    {
        return false;
    }
    let year: u32 = date[..4].parse().unwrap();
    let month: usize = date[5..7].parse().unwrap();
    let day: u32 = date[8..].parse().unwrap();
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    year > 0 && (1..=12).contains(&month) && day > 0 && day <= days[month - 1]
}

pub fn cli(args: &[String]) -> Result<()> {
    let mut repo = None;
    let mut project = None;
    let mut url = "http://127.0.0.1:8787".to_owned();
    let mut options = Options::default();
    let mut allow_remote = false;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--repo" => repo = Some(args.next().ok_or("missing repo")?.clone()),
            "--project" => project = Some(args.next().ok_or("missing project")?.clone()),
            "--url" => url = args.next().ok_or("missing URL")?.clone(),
            "--dry-run" => options.dry = true,
            "--close-github" => options.close_github = true,
            "--state" => options.state = args.next().ok_or("missing state")?.clone(),
            "--since" => options.since = Some(args.next().ok_or("missing date")?.clone()),
            "--exclude" => {
                options.exclude = args
                    .next()
                    .ok_or("missing exclusions")?
                    .split(',')
                    .map(str::parse)
                    .collect::<std::result::Result<_, _>>()?;
            }
            "--allow-remote" => allow_remote = true,
            _ => return Err(format!("unknown import argument: {arg}").into()),
        }
    }
    if !["all", "open", "closed"].contains(&options.state.as_str()) {
        return Err("--state must be all, open or closed".into());
    }
    if let Some(date) = &options.since {
        // Accept a calendar date; compare against GitHub's UTC closed_at timestamp.
        let valid = valid_date(date);
        if !valid {
            return Err("--since must be YYYY-MM-DD".into());
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
    let tickets = Api::with_remote(
        &url,
        env::var("TICKETS_TOKEN").map_err(|_| "TICKETS_TOKEN required")?,
        false,
        allow_remote,
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
    import(&github, &tickets, &repo, &project, &options)
}

pub fn import(gh: &Api, api: &Api, repo: &str, key: &str, options: &Options) -> Result<()> {
    let dry = options.dry;
    let metadata = gh.call(Method::GET, &format!("/repos/{repo}"), Value::Null)?;
    let canonical = text(&metadata, "full_name")?.to_ascii_lowercase();
    if identity_repo(&canonical) != identity_repo(repo) {
        return Err("repository renamed: refusing a new external-ID namespace; keep the original --repo and add an explicit identity alias before importing".into());
    }
    let source_repo = canonical.as_str();
    let repo = identity_repo(repo);
    let source = gh.list(&format!(
        "/repos/{source_repo}/issues?state={}&sort=created&direction=asc",
        options.state
    ))?;
    let issues: Vec<_> = source
        .iter()
        .filter(|i| selected(i, options))
        .cloned()
        .collect();
    let skipped = source.len() - issues.len();
    let mut comments = BTreeMap::new();
    let mut receipts = BTreeMap::new();
    for issue in &issues {
        let n = number(issue)?;
        let discussion = gh.list(&format!("/repos/{source_repo}/issues/{n}/comments"))?;
        receipts.insert(n, discussion.clone());
        comments.insert(
            n,
            discussion
                .into_iter()
                .filter(|comment| {
                    !comment["body"].as_str().is_some_and(|body| {
                        body.starts_with("Moved to Tickets: ")
                            && body.ends_with(
                                "Tickets is now the system of record; this issue is closed.",
                            )
                    })
                })
                .collect::<Vec<_>>(),
        );
    }
    let open = issues.iter().filter(|i| i["state"] == "open").count();
    let closed = issues.len() - open;
    let projects = api.list("/api/v1/projects")?;
    let project = projects
        .iter()
        .find(|p| p["key_prefix"] == key)
        .ok_or("--project must name an existing project")?;
    let project_id = id(project)?;
    let prior = api.list("/api/v1/tickets")?;
    let updated = issues
        .iter()
        .filter(|issue| {
            prior
                .iter()
                .any(|ticket| ticket["external_id"] == format!("github:{repo}:{}", issue["number"]))
        })
        .count();
    let mut statuses = BTreeMap::new();
    for issue in &issues {
        let n = number(issue)?;
        let mut status = serde_json::from_value::<Issue>(issue.clone())?
            .status()
            .to_owned();
        if issue["state"] == "closed"
            && let Some(ticket) = prior
                .iter()
                .find(|t| t["external_id"] == format!("github:{repo}:{n}"))
            && already_moved(&receipts[&n], &close_message(&api.base, ticket)?)
        {
            // Our archival close is not completion of the native task. A rerun
            // after --close-github must retain its authoritative Tickets status.
            status = text(ticket, "status")?.to_owned();
        }
        statuses.insert(n, status);
    }
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
        for label in serde_json::from_value::<Issue>(issue.clone())?.labels {
            wanted_labels.insert(label.name.clone(), label.ticket_color());
        }
        for blocker in blockers(issue["body"].as_str().unwrap_or("")) {
            if !numbers.contains(&blocker) {
                continue;
            }
            edges.insert((blocker, number(issue)?));
        }
        let n = number(issue)?;
        for blocker in gh.list(&format!(
            "/repos/{source_repo}/issues/{n}/dependencies/blocked_by"
        ))? {
            if same_repo(&blocker, source_repo)
                && let Ok(b) = number(&blocker)
                && numbers.contains(&b)
            {
                edges.insert((b, n));
            }
        }
        // A GitHub sub-issue is a blocker of its parent.
        for child in gh.list(&format!("/repos/{source_repo}/issues/{n}/sub_issues"))? {
            if same_repo(&child, source_repo)
                && let Ok(b) = number(&child)
                && numbers.contains(&b)
            {
                edges.insert((b, n));
            }
        }
        println!(
            "{} issue #{}: {} labels, {} comments, {}",
            if dry {
                if prior
                    .iter()
                    .any(|t| t["external_id"] == format!("github:{repo}:{}", issue["number"]))
                {
                    "UPDATE"
                } else {
                    "CREATE"
                }
            } else {
                "Import"
            },
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
            "DRY RUN: project {key} (existing), {open} open, {closed} closed, {} labels, {} dependencies; no writes",
            wanted_labels.len(),
            edges.len()
        );
        println!(
            "Summary {source_repo}: imported={} updated={updated} skipped={skipped} closed=0 (planned)",
            issues.len() - updated
        );
        println!("issue\tkey");
        for issue in &issues {
            let n = number(issue)?;
            let key = prior
                .iter()
                .find(|t| t["external_id"] == format!("github:{repo}:{n}"))
                .and_then(|t| t["key"].as_str())
                .unwrap_or("(allocated on import)");
            println!("{source_repo}#{n}\t{key}");
        }
        return Ok(());
    }
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
        let external_id = format!("github:{repo}:{n}");
        let mut url = Url::parse("http://localhost/api/v1/tickets/by-external")?;
        url.query_pairs_mut().append_pair("id", &external_id);
        let path = format!("{}?{}", url.path(), url.query().unwrap());
        let mut payload = serde_json::from_value::<Issue>(issue.clone())?.upsert(project_id);
        payload["status"] = json!(statuses[&n]);
        if prior.iter().any(|t| t["external_id"] == external_id) {
            // Keep the saved canonical body until all native keys are known.
            // Restoring raw GitHub shorthand here and rewriting it below would
            // manufacture two description edits on an otherwise unchanged replay.
            payload.as_object_mut().unwrap().remove("body");
        }
        let ticket = api.call(Method::PUT, &path, payload)?;
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
    let keys = tickets
        .iter()
        .map(|(n, t)| Ok((*n, text(t, "key")?.to_owned())))
        .collect::<Result<BTreeMap<_, _>>>()?;
    for issue in &issues {
        let n = number(issue)?;
        let body = rewrite_refs(issue["body"].as_str().unwrap_or(""), source_repo, &keys);
        if tickets[&n]["body"] != body {
            api.call(
                Method::PATCH,
                &format!("/api/v1/tickets/{}", id(&tickets[&n])?),
                json!({"body":body}),
            )?;
        }
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
    let mut gaps = 0;
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
            let expected = if field == "body" {
                rewrite_refs(issue[field].as_str().unwrap_or(""), source_repo, &keys)
            } else {
                issue[field].as_str().unwrap_or("").to_owned()
            };
            if ticket[field] != expected {
                gaps += 1;
            }
        }
        if ticket["assignee"]
            != serde_json::from_value::<Issue>(issue.clone())?.upsert(project_id)["assignee"]
            || ticket["external_url"] != issue["html_url"]
            || ticket["status"] != statuses[&n]
            || ticket["priority"] != serde_json::from_value::<Issue>(issue.clone())?.priority()
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
    let actual_closed = tickets
        .values()
        .filter(|t| t["status"] == "done" || t["status"] == "canceled")
        .count();
    println!(
        "Reconciliation: GitHub open/closed {open}/{closed}; Tickets {}/{}; labels {}/{}; dependencies {actual_edges}/{}; comments {actual_comments}/{}; gaps {gaps}",
        issues.len() - actual_closed,
        actual_closed,
        actual_labels.len(),
        wanted_labels.len(),
        edges.len(),
        comments.values().map(Vec::len).sum::<usize>()
    );
    if gaps != 0 {
        return Err("import reconciliation has gaps".into());
    }
    let mut closed_count = 0;
    if options.close_github {
        // No GitHub writes until every field, comment and edge has passed read-back verification.
        for issue in &issues {
            let n = number(issue)?;
            let ticket = &tickets[&n];
            let message = close_message(&api.base, ticket)?;
            let current = gh.list(&format!("/repos/{source_repo}/issues/{n}/comments"))?;
            if !already_moved(&current, &message) {
                gh.call(
                    Method::POST,
                    &format!("/repos/{source_repo}/issues/{n}/comments"),
                    json!({"body":message}),
                )?;
            }
            gh.call(Method::PATCH, &format!("/repos/{source_repo}/issues/{n}"), json!({"state":"closed","state_reason":if ticket["status"]=="canceled" {"not_planned"} else {"completed"}}))?;
            closed_count += usize::from(issue["state"] != "closed");
        }
    }
    println!(
        "Summary {source_repo}: imported={} updated={updated} skipped={} closed={closed_count}",
        issues.len() - updated,
        skipped
    );
    println!("issue\tkey");
    for (n, ticket) in &tickets {
        println!("{source_repo}#{n}\t{}", text(ticket, "key")?);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn references_selection_and_close_idempotency() {
        assert!(valid_date("2024-02-29"));
        assert!(!valid_date("2026-02-29"));
        assert!(!valid_date("2026-99-99"));
        let keys = BTreeMap::from([(1, "WWW-7".into()), (2, "WWW-8".into())]);
        assert_eq!(
            rewrite_refs(
                "Blocked by #1\nDepends on #2; #99. `#1` https://github.com/a/b/issues/1#2",
                "a/b",
                &keys
            ),
            "Blocked by WWW-7\nDepends on WWW-8; https://github.com/a/b/issues/99. `#1` https://github.com/a/b/issues/1#2"
        );
        let mut issue = json!({"number":1,"state":"open"});
        assert!(selected(&issue, &Options::default()));
        issue["state"] = json!("closed");
        issue["closed_at"] = json!("2026-01-02T00:00:00Z");
        assert!(!selected(&issue, &Options::default()));
        let mut options = Options {
            state: "all".into(),
            since: Some("2026-01-01".into()),
            ..Options::default()
        };
        assert!(selected(&issue, &options));
        options.since = Some("2026-01-03".into());
        assert!(!selected(&issue, &options));
        issue["state"] = json!("open");
        assert!(selected(&issue, &options));
        options.exclude.insert(1);
        assert!(!selected(&issue, &options));
        let message =
            close_message("https://tickets.example", &json!({"id":42,"key":"WWW-7"})).unwrap();
        assert_eq!(
            message,
            "Moved to Tickets: https://tickets.example/ticket?id=42 (WWW-7). Tickets is now the system of record; this issue is closed."
        );
        assert!(!already_moved(&[], &message));
        assert!(already_moved(&[json!({"body":message})], &message));
        assert!(already_moved(
            &[json!({"body":message.replace("WWW-7", "OLD-3")})],
            &message
        ));
    }
    #[test]
    fn remote_urls_and_identity() {
        for url in ["http://localhost", "http://127.0.0.1:8790", "http://[::1]"] {
            assert!(Api::new(url, "test".into(), false).is_ok());
        }
        for url in [
            "https://tickets.worldwidewebb.co",
            "https://example.com:443/",
        ] {
            assert!(Api::new(url, "test".into(), false).is_err());
            assert!(Api::with_remote(url, "test".into(), false, true).is_ok());
        }
        for url in [
            "http://example.com",
            "https://user@example.com",
            "https://example.com/path",
            "https://example.com?x",
            "https://example.com#x",
            "https://user:pass@example.com",
        ] {
            assert!(Api::with_remote(url, "test".into(), false, true).is_err());
        }
        assert_eq!(
            identity_repo("0x63616c/quantum"),
            identity_repo("0x63616c/voxelite")
        );
        assert_eq!(retry_after("12", UNIX_EPOCH), Some(12));
        assert_eq!(
            retry_after("Thu, 01 Jan 1970 00:00:10 GMT", UNIX_EPOCH),
            Some(11)
        );
        assert_eq!(retry_after("invalid", UNIX_EPOCH), None);
    }
    #[test]
    fn dependency_references_are_unique_and_directed() {
        assert_eq!(
            blockers("Blocked by #12\n- Blocked by #7\nBlocked by #12\nBlocked by #oops"),
            BTreeSet::from([7, 12])
        );
    }
}
