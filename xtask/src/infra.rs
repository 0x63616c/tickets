//! Read-only production drift detection. Never log API bodies or binding payloads.
use reqwest::{StatusCode, blocking::Client};
use serde_json::{Value, json};
use std::{env, error::Error, fs, fs::OpenOptions, io::Write, path::Path, time::Duration};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
// Wrangler 4 removed usage_model; all deployments use the standard model.
const USAGE_MODEL: &str = "standard";
const ZONE: &str = "worldwidewebb.co";
// Names only. The test below checks the deploy step's jq object and environment.
const SECRETS: &[&str] = &[
    "BOOTSTRAP_USERNAME",
    "BOOTSTRAP_PASSWORD_HASH",
    "GITHUB_WEBHOOK_SECRET",
    "GITHUB_APP_STORE_KEY",
    "GITHUB_APP_SETUP_STATE",
];

struct Expected {
    script: String,
    settings: Value,
    bindings: Vec<Value>,
    crons: Value,
    subdomain: Value,
    domains: Vec<Value>,
    sqlite_classes: Vec<String>,
}

fn array(value: &Value) -> Result<&Vec<Value>> {
    value
        .as_array()
        .ok_or_else(|| "Missing/invalid API or config array".into())
}
fn string(value: &Value) -> Result<&str> {
    value
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Missing/invalid API or config string".into())
}
fn sorted(mut values: Vec<Value>) -> Value {
    values.sort_by_key(Value::to_string);
    json!(values)
}

impl Expected {
    fn parse(source: &str) -> Result<Self> {
        let config: toml::Value = toml::from_str(source)?;
        let config = serde_json::to_value(config)?;
        let prod = &config["env"]["production"];
        let inherited = |key: &str| prod.get(key).unwrap_or(&config[key]).clone();
        let mut settings = json!({});
        for key in [
            "compatibility_date",
            "compatibility_flags",
            "logpush",
            "tail_consumers",
            "observability",
            "placement",
        ] {
            settings[key] = inherited(key);
            if settings[key].is_null() {
                return Err(format!("Declare {key} explicitly in wrangler.toml").into());
            }
        }
        settings["usage_model"] = json!(USAGE_MODEL);
        let mut bindings = Vec::new();
        let assets = inherited("assets");
        bindings.push(json!({"name":string(&assets["binding"])?,"type":"assets"}));
        for db in array(&prod["d1_databases"])? {
            bindings.push(json!({"name":string(&db["binding"])?,"type":"d1","id":string(&db["database_id"])?}));
        }
        for binding in array(&prod["durable_objects"]["bindings"])? {
            if binding.get("script_name").is_some() {
                return Err("External DO bindings are not supported by infra-check".into());
            }
            bindings.push(json!({"name":string(&binding["name"])?,"type":"durable_object_namespace","class_name":string(&binding["class_name"])?}));
        }
        for (name, value) in prod["vars"].as_object().ok_or("Missing production vars")? {
            bindings.push(json!({"name":name,"type":"plain_text","text":string(value)?}));
        }
        for name in SECRETS {
            bindings.push(json!({"name":name,"type":"secret_text"}));
        }
        let mut domains = Vec::new();
        for route in array(&prod["routes"])? {
            if route["custom_domain"] != true {
                return Err("infra-check currently expects custom-domain routes only".into());
            }
            domains.push(json!({"hostname":string(&route["pattern"])?,"enabled":true}));
        }
        let migrations = inherited("migrations");
        let mut sqlite_classes = Vec::new();
        for migration in array(&migrations)? {
            if let Some(classes) = migration.get("new_sqlite_classes") {
                for class in array(classes)? {
                    sqlite_classes.push(string(class)?.to_owned());
                }
            }
        }
        Ok(Self {
            script: string(&prod["name"])?.to_owned(),
            settings,
            bindings,
            crons: sorted(array(&prod["triggers"]["crons"])?.clone()),
            subdomain: json!({"enabled":inherited("workers_dev"),"previews_enabled":inherited("preview_urls")}),
            domains,
            sqlite_classes,
        })
    }
}

#[derive(Default)]
struct Live {
    settings: Value,
    schedules: Value,
    subdomain: Value,
    domains: Vec<Value>,
    dns: Vec<Value>,
    namespaces: Vec<Value>,
    routes: Vec<Value>,
    rulesets: Option<Vec<Value>>,
    warnings: Vec<String>,
}

struct Row {
    field: String,
    expected: Value,
    live: Value,
}
impl Row {
    fn new(field: impl Into<String>, expected: Value, live: Value) -> Self {
        Self {
            field: field.into(),
            expected,
            live,
        }
    }
    fn drift(&self) -> bool {
        self.expected != self.live
    }
}

fn compare(expected: &Expected, live: &Live) -> Vec<Row> {
    let mut rows = vec![Row::new(
        "script exists",
        json!(true),
        json!(!live.settings.is_null()),
    )];
    for key in [
        "compatibility_date",
        "compatibility_flags",
        "usage_model",
        "logpush",
        "tail_consumers",
    ] {
        let actual = live
            .settings
            .get(key)
            .cloned()
            .unwrap_or_else(|| match key {
                "compatibility_flags" | "tail_consumers" => json!([]),
                _ => Value::Null,
            });
        let want = expected.settings[key].clone();
        let (want, actual) = if key == "compatibility_flags" || key == "tail_consumers" {
            (
                sorted(want.as_array().cloned().unwrap_or_default()),
                actual
                    .as_array()
                    .map(|a| sorted(a.clone()))
                    .unwrap_or(actual),
            )
        } else {
            (want, actual)
        };
        rows.push(Row::new(key, want, actual));
    }
    for pointer in [
        "/enabled",
        "/head_sampling_rate",
        "/logs/enabled",
        "/logs/invocation_logs",
        "/logs/head_sampling_rate",
    ] {
        let want = expected.settings["observability"]
            .pointer(pointer)
            .cloned()
            .unwrap_or_default();
        let actual = live.settings["observability"]
            .pointer(pointer)
            .cloned()
            .unwrap_or_default();
        // TOML's 1 and the API's 1.0 represent the same sampling rate.
        let actual = if want.is_number() && actual.as_f64() == want.as_f64() {
            want.clone()
        } else {
            actual
        };
        rows.push(Row::new(format!("observability{pointer}"), want, actual));
    }
    let placement = &live.settings["placement"]["mode"];
    let placement = if placement.is_null() || placement == "default" {
        json!("off")
    } else {
        placement.clone()
    };
    rows.push(Row::new(
        "placement.mode",
        expected.settings["placement"]["mode"].clone(),
        placement,
    ));
    let actual_bindings = live.settings["bindings"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    // Only project safe metadata; never serialize arbitrary API binding objects.
    let names = |bindings: &[Value]| {
        sorted(
            bindings
                .iter()
                .map(|b| json!({"name":b["name"],"type":b["type"]}))
                .collect(),
        )
    };
    rows.push(Row::new(
        "bindings (names/types)",
        names(&expected.bindings),
        names(&actual_bindings),
    ));
    for binding in &expected.bindings {
        let actual = actual_bindings
            .iter()
            .find(|b| b["name"] == binding["name"] && b["type"] == binding["type"])
            .unwrap_or(&Value::Null);
        let name = binding["name"].as_str().unwrap_or("?");
        for key in ["id", "class_name", "text"] {
            if let Some(want) = binding.get(key) {
                if key == "text" {
                    rows.push(Row::new(
                        format!("binding {name} value"),
                        json!("matches configured var"),
                        json!(if actual[key] == *want {
                            "matches configured var"
                        } else {
                            "missing/different value"
                        }),
                    ));
                } else {
                    rows.push(Row::new(
                        format!("binding {name}.{key}"),
                        want.clone(),
                        actual[key].clone(),
                    ));
                }
            }
        }
        if binding["type"] == "durable_object_namespace" {
            let namespace = live.namespaces.iter().find(|n| {
                n["id"] == actual["namespace_id"]
                    && n["script"] == expected.script
                    && n["class"] == binding["class_name"]
            });
            let sqlite = expected
                .sqlite_classes
                .iter()
                .any(|c| binding["class_name"] == *c);
            rows.push(Row::new(
                format!("DO {name} exists with SQLite migration"),
                json!(true),
                json!(sqlite && namespace.is_some_and(|n| n["use_sqlite"] == true)),
            ));
        }
    }
    let crons = live.schedules["schedules"]
        .as_array()
        .map(|a| sorted(a.iter().map(|v| v["cron"].clone()).collect()))
        .unwrap_or(Value::Null);
    rows.push(Row::new("cron schedules", expected.crons.clone(), crons));
    for key in ["enabled", "previews_enabled"] {
        rows.push(Row::new(
            format!("workers.dev {key}"),
            expected.subdomain[key].clone(),
            live.subdomain[key].clone(),
        ));
    }
    let domains: Vec<_> = live
        .domains
        .iter()
        .filter(|d| d["service"] == expected.script)
        .collect();
    rows.push(Row::new(
        "custom domains",
        sorted(expected.domains.clone()),
        sorted(
            domains
                .iter()
                .map(|d| json!({"hostname":d["hostname"],"enabled":d["enabled"]}))
                .collect(),
        ),
    ));
    for want in &expected.domains {
        let host = want["hostname"].as_str().unwrap_or("?");
        let domain = domains.iter().find(|d| d["hostname"] == host);
        let records: Vec<_> = live.dns.iter().filter(|d| d["name"] == host).collect();
        let valid = records.len() == 1
            && domain.is_some_and(|domain| {
                records[0]["type"] == "AAAA"
                    && records[0]["content"] == "100::"
                    && records[0]["meta"]["read_only"] == true
                    && domain["id"].is_string()
                    && records[0]["meta"]["origin_worker_id"] == domain["id"]
            });
        rows.push(Row::new(
            format!("DNS {host}"),
            json!("one read-only Worker AAAA 100:: owned by custom domain"),
            json!(if valid {
                "one read-only Worker AAAA 100:: owned by custom domain".to_owned()
            } else {
                format!(
                    "{} record(s); missing or unexpected ownership/type/content",
                    records.len()
                )
            }),
        ));
    }
    rows.push(Row::new(
        "zone Worker routes for script",
        json!(0),
        json!(
            live.routes
                .iter()
                .filter(|r| r["script"] == expected.script)
                .count()
        ),
    ));
    if let Some(rulesets) = &live.rulesets {
        let count = rulesets
            .iter()
            .filter(|r| r["kind"] != "managed")
            .flat_map(|r| r["rules"].as_array().into_iter().flatten())
            .filter(|rule| {
                rule["expression"].as_str().is_some_and(|expr| {
                    expected.domains.iter().any(|d| {
                        expr.to_ascii_lowercase().contains(
                            d["hostname"]
                                .as_str()
                                .unwrap_or("?")
                                .to_ascii_lowercase()
                                .as_str(),
                        )
                    })
                })
            })
            .count();
        rows.push(Row::new(
            "non-managed rules referencing Tickets host",
            json!(0),
            json!(count),
        ));
    }
    rows
}

struct Api {
    client: Client,
    token: String,
}
impl Api {
    fn get(&self, path: &str, query: &[(String, String)], optional: bool) -> Result<Option<Value>> {
        let response = self
            .client
            .get(format!("https://api.cloudflare.com/client/v4/{path}"))
            .bearer_auth(&self.token)
            .query(query)
            .send()
            .map_err(|_| "Cloudflare GET failed (transport error)")?;
        if optional && response.status() == StatusCode::FORBIDDEN {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(format!(
                "Cloudflare GET {path}: HTTP {} (check token permissions/service status)",
                response.status()
            )
            .into());
        }
        let body: Value = response
            .json()
            .map_err(|_| "Invalid Cloudflare JSON response")?;
        if body["success"] != true || body.get("result").is_none() {
            return Err(format!(
                "Cloudflare GET {path}: unsuccessful/missing result (body withheld)"
            )
            .into());
        }
        Ok(Some(body))
    }
    fn object(&self, path: &str) -> Result<Value> {
        Ok(self.get(path, &[], false)?.ok_or("Missing API response")?["result"].clone())
    }
    fn list(
        &self,
        path: &str,
        query: &[(String, String)],
        optional: bool,
    ) -> Result<Option<Vec<Value>>> {
        let mut values = Vec::new();
        let mut page = 1;
        let mut cursor: Option<String> = None;
        let mut requests = 0;
        loop {
            requests += 1;
            let mut query = query.to_vec();
            query.push(("page".into(), page.to_string()));
            if let Some(ref cursor) = cursor {
                query.push(("cursor".into(), cursor.clone()));
            }
            let Some(body) = self.get(path, &query, optional)? else {
                return Ok(None);
            };
            values.extend(array(&body["result"])?.clone());
            let info = &body["result_info"];
            if let Some(next) = info["cursors"]["after"].as_str().filter(|s| !s.is_empty()) {
                if cursor.as_deref() == Some(next) {
                    return Err("Repeated Cloudflare pagination cursor".into());
                }
                cursor = Some(next.to_owned());
            } else if page < info["total_pages"].as_u64().unwrap_or(1) {
                page += 1;
            } else {
                break;
            }
            if requests >= 1000 || values.len() > 100_000 {
                return Err("Cloudflare pagination limit exceeded".into());
            }
        }
        Ok(Some(values))
    }
    fn required_list(&self, path: &str, query: &[(String, String)]) -> Result<Vec<Value>> {
        self.list(path, query, false)?
            .ok_or_else(|| "Missing API list".into())
    }
}

fn collect(api: &Api, account: &str, expected: &Expected) -> Result<Live> {
    let worker = format!("accounts/{account}/workers/scripts/{}", expected.script);
    let mut live = Live {
        settings: api.object(&format!("{worker}/settings"))?,
        schedules: api.object(&format!("{worker}/schedules"))?,
        subdomain: api.object(&format!("{worker}/subdomain"))?,
        domains: api.required_list(
            &format!("accounts/{account}/workers/domains"),
            &[("service".into(), expected.script.clone())],
        )?,
        namespaces: api.required_list(
            &format!("accounts/{account}/workers/durable_objects/namespaces"),
            &[],
        )?,
        ..Live::default()
    };
    let zones = api.required_list(
        "zones",
        &[
            ("name".into(), ZONE.into()),
            ("account.id".into(), account.into()),
        ],
    )?;
    if zones.len() != 1 {
        return Err("Expected exactly one worldwidewebb.co zone".into());
    }
    let zone = string(&zones[0]["id"])?;
    for domain in &expected.domains {
        live.dns.extend(api.required_list(
            &format!("zones/{zone}/dns_records"),
            &[("name".into(), string(&domain["hostname"])?.into())],
        )?);
    }
    live.routes = api.required_list(&format!("zones/{zone}/workers/routes"), &[])?;
    // List responses contain summaries; expressions require fetching each detail.
    let mut rulesets = api.list(&format!("zones/{zone}/rulesets"), &[], true)?;
    if let Some(ref mut rulesets) = rulesets {
        for ruleset in rulesets.iter_mut().filter(|r| r["kind"] != "managed") {
            let path = format!("zones/{zone}/rulesets/{}", string(&ruleset["id"])?);
            if let Some(body) = api.get(&path, &[], true)? {
                *ruleset = body["result"].clone();
                array(&ruleset["rules"])?;
            } else {
                rulesets.clear();
                live.warnings
                    .push("WARN rulesets skipped: token lacks Zone WAF Read (HTTP 403)".into());
                break;
            }
        }
    } else {
        live.warnings
            .push("WARN rulesets skipped: token lacks Zone WAF Read (HTTP 403)".into());
    }
    live.rulesets = if live.warnings.is_empty() {
        rulesets
    } else {
        None
    };
    Ok(live)
}

fn report(rows: &[Row], warnings: &[String]) -> String {
    fn cell(value: &Value) -> String {
        value
            .to_string()
            .replace('|', "&#124;")
            .replace(['\n', '\r'], " ")
    }
    let mut report =
        String::from("| Field | Expected | Live | Status |\n| --- | --- | --- | --- |\n");
    for row in rows {
        report.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            row.field.replace('|', "&#124;"),
            cell(&row.expected),
            cell(&row.live),
            if row.drift() { "DRIFT" } else { "OK" }
        ));
    }
    for warning in warnings {
        report.push_str(&format!("\n{warning}\n"));
    }
    report
}

pub fn check(root: &Path) -> super::Result {
    let expected = Expected::parse(&fs::read_to_string(root.join("wrangler.toml"))?)?;
    let token = env::var("CLOUDFLARE_API_TOKEN").map_err(|_| "Missing CLOUDFLARE_API_TOKEN")?;
    let account = env::var("CLOUDFLARE_ACCOUNT_ID").map_err(|_| "Missing CLOUDFLARE_ACCOUNT_ID")?;
    if token.is_empty() || account.is_empty() {
        return Err("Empty Cloudflare credentials".into());
    }
    let api = Api {
        client: Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()?,
        token,
    };
    let (rows, warnings) = match collect(&api, &account, &expected) {
        Ok(live) => (compare(&expected, &live), live.warnings),
        Err(error) => (
            vec![Row::new(
                "essential API checks",
                json!("readable production state"),
                json!(error.to_string()),
            )],
            vec![],
        ),
    };
    let output = report(&rows, &warnings);
    println!("{output}");
    if let Ok(path) = env::var("GITHUB_STEP_SUMMARY") {
        writeln!(
            OpenOptions::new().append(true).create(true).open(path)?,
            "{output}"
        )?;
    }
    if rows.iter().any(Row::drift) {
        return Err("Cloudflare infrastructure drift/unverified state; see table above".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn expected() -> Expected {
        Expected::parse(include_str!("../../wrangler.toml")).unwrap()
    }
    fn fixture() -> Live {
        let data: Value =
            serde_json::from_str(include_str!("../tests/fixtures/infra-matching.json")).unwrap();
        Live {
            settings: data["settings"].clone(),
            schedules: data["schedules"].clone(),
            subdomain: data["subdomain"].clone(),
            domains: array(&data["domains"]).unwrap().clone(),
            dns: array(&data["dns"]).unwrap().clone(),
            namespaces: array(&data["namespaces"]).unwrap().clone(),
            routes: array(&data["routes"]).unwrap().clone(),
            rulesets: Some(array(&data["rulesets"]).unwrap().clone()),
            warnings: vec![],
        }
    }
    fn drifts(live: &Live) -> Vec<String> {
        compare(&expected(), live)
            .into_iter()
            .filter(Row::drift)
            .map(|r| r.field)
            .collect()
    }
    #[test]
    fn matching_production_passes() {
        assert_eq!(drifts(&fixture()), Vec::<String>::new());
    }
    #[test]
    fn missing_or_disabled_observability_drifts() {
        for observability in [Value::Null, json!({"enabled":false})] {
            let mut live = fixture();
            live.settings["observability"] = observability;
            assert!(drifts(&live).iter().any(|f| f.starts_with("observability")));
        }
        for pointer in [
            "/enabled",
            "/head_sampling_rate",
            "/logs/enabled",
            "/logs/invocation_logs",
            "/logs/head_sampling_rate",
        ] {
            let mut live = fixture();
            *live.settings["observability"].pointer_mut(pointer).unwrap() = json!(0);
            assert!(
                drifts(&live)
                    .iter()
                    .any(|f| f == &format!("observability{pointer}"))
            );
        }
    }
    #[test]
    fn extra_manual_dns_or_wrong_owner_drifts() {
        let mut live = fixture();
        live.dns
            .push(json!({"name":"tickets.worldwidewebb.co","type":"A","content":"192.0.2.1"}));
        assert!(drifts(&live).iter().any(|f| f.starts_with("DNS")));
        let mut live = fixture();
        live.dns[0]["meta"]["origin_worker_id"] = json!("other-domain");
        assert!(drifts(&live).iter().any(|f| f.starts_with("DNS")));
    }
    #[test]
    fn extra_or_missing_secrets_drift_without_values() {
        let mut live = fixture();
        live.settings["bindings"]
            .as_array_mut()
            .unwrap()
            .push(json!({"name":"EXTRA_SECRET","type":"secret_text","text":"MUST_NOT_PRINT"}));
        assert!(drifts(&live).contains(&"bindings (names/types)".into()));
        assert!(!report(&compare(&expected(), &live), &[]).contains("MUST_NOT_PRINT"));
        let mut live = fixture();
        live.settings["bindings"]
            .as_array_mut()
            .unwrap()
            .retain(|b| b["name"] != SECRETS[0]);
        assert!(drifts(&live).contains(&"bindings (names/types)".into()));
    }
    #[test]
    fn bindings_schedules_domains_and_namespaces_are_checked() {
        for key in ["id", "class_name", "text"] {
            let mut live = fixture();
            let binding = live.settings["bindings"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|b| b.get(key).is_some())
                .unwrap();
            binding[key] = json!("changed");
            assert!(!drifts(&live).is_empty());
        }
        let mut live = fixture();
        live.namespaces[0]["use_sqlite"] = json!(false);
        assert!(!drifts(&live).is_empty());
        let mut live = fixture();
        live.domains[0]["enabled"] = json!(false);
        assert!(!drifts(&live).is_empty());
        let mut live = fixture();
        live.schedules["schedules"] = json!([]);
        assert!(!drifts(&live).is_empty());
    }
    #[test]
    fn rulesets_ignore_other_projects_but_check_non_managed_host_expressions() {
        let mut live = fixture();
        live.rulesets = Some(vec![
            json!({"kind":"zone","rules":[{"expression":"http.host eq \"other.worldwidewebb.co\""}]}),
            json!({"kind":"managed","rules":[{"expression":"http.host eq \"tickets.worldwidewebb.co\""}]}),
        ]);
        assert!(drifts(&live).is_empty());
        live.rulesets.as_mut().unwrap()[0]["rules"][0]["expression"] =
            json!("http.host eq \"tickets.worldwidewebb.co\"");
        assert!(!drifts(&live).is_empty());
        live.rulesets = None;
        assert!(drifts(&live).is_empty());
    }
    #[test]
    fn secret_names_match_deploy_step() {
        let ci = include_str!("../../.github/workflows/ci.yml");
        let step = ci
            .split("- name: Deploy Worker")
            .nth(1)
            .unwrap()
            .split("- name:")
            .next()
            .unwrap();
        let object = step
            .lines()
            .find(|line| line.trim_start().starts_with("jq -n '"))
            .unwrap()
            .split('\'')
            .nth(1)
            .unwrap();
        let mut deployed = Vec::new();
        for entry in object.trim_matches(['{', '}']).split(',') {
            let (name, value) = entry.trim().split_once(':').unwrap();
            assert_eq!(value.trim(), format!("env.{name}"));
            assert!(step.contains(&format!("{name}: ${{{{ secrets.TICKETS_{name} }}}}")));
            deployed.push(json!(name));
        }
        assert_eq!(
            sorted(deployed),
            sorted(SECRETS.iter().map(|s| json!(s)).collect())
        );
    }
}
