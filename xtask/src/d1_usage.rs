//! Read Cloudflare analytics, never D1 itself. A failed job is the alert channel.
use serde_json::{Value, json};
use std::{env, fs::OpenOptions, io::Write, process::Command, time::Duration};

const THRESHOLD: u64 = 3_000_000;
const QUERY: &str = r#"query DailyD1Reads($account: string!, $day: Date!) {
  viewer { accounts(filter: {accountTag: $account}) {
    d1AnalyticsAdaptiveGroups(limit: 1, filter: {date_geq: $day, date_leq: $day}) {
      sum { rowsRead }
    }
  } }
}"#;

fn rows_read(value: &Value) -> Result<u64, &'static str> {
    if value
        .get("errors")
        .is_some_and(|e| !e.is_null() && !e.as_array().is_some_and(Vec::is_empty))
    {
        return Err(
            "Cloudflare GraphQL error; check Account Analytics Read permission and service status",
        );
    }
    let accounts = value["data"]["viewer"]["accounts"]
        .as_array()
        .ok_or("Missing analytics accounts")?;
    if accounts.len() != 1 {
        return Err("Expected exactly one analytics account");
    }
    let groups = accounts[0]["d1AnalyticsAdaptiveGroups"]
        .as_array()
        .ok_or("Missing D1 analytics")?;
    // Empty results could mean no usage, delayed analytics or lack of visibility.
    // Fail visibly rather than reporting an unverified zero.
    if groups.len() != 1 {
        return Err("No daily D1 analytics available yet");
    }
    groups[0]["sum"]["rowsRead"]
        .as_u64()
        .ok_or("Invalid daily rowsRead")
}

pub fn check() -> super::Result {
    let token = env::var("CLOUDFLARE_API_TOKEN").map_err(|_| "Missing CLOUDFLARE_API_TOKEN")?;
    let account = env::var("CLOUDFLARE_ACCOUNT_ID").map_err(|_| "Missing CLOUDFLARE_ACCOUNT_ID")?;
    if token.is_empty() || account.is_empty() {
        return Err("Empty Cloudflare credentials".into());
    }
    let date = Command::new("date").args(["-u", "+%F"]).output()?;
    if !date.status.success() {
        return Err("Could not determine UTC day".into());
    }
    let day = String::from_utf8(date.stdout)?.trim().to_owned();
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?
        .post("https://api.cloudflare.com/client/v4/graphql")
        .bearer_auth(token)
        .json(&json!({"query":QUERY,"variables":{"account":account,"day":day}}))
        .send()?;
    if !response.status().is_success() {
        return Err(format!(
            "Cloudflare analytics returned HTTP {}; check credentials/service status",
            response.status()
        )
        .into());
    }
    let rows = rows_read(&response.json()?)?;
    let summary =
        format!("D1 account rows read on {day} UTC: {rows} (alert threshold {THRESHOLD}).");
    println!("{summary}");
    if let Ok(path) = env::var("GITHUB_STEP_SUMMARY") {
        writeln!(
            OpenOptions::new().append(true).create(true).open(path)?,
            "{summary}"
        )?;
    }
    evaluate(rows)
}

fn evaluate(rows: u64) -> super::Result {
    if rows >= THRESHOLD {
        return Err(format!("D1 daily read alert: {rows} >= {THRESHOLD}; investigate before the 5M Free limit. See docs/d1-operations.md").into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn data(rows: Value) -> Value {
        json!({"data":{"viewer":{"accounts":[{"d1AnalyticsAdaptiveGroups":[{"sum":{"rowsRead":rows}}]}]}}})
    }
    #[test]
    fn reads_rows_not_query_count_and_preserves_threshold() {
        for rows in [0, THRESHOLD - 1, THRESHOLD, 5_000_000] {
            assert_eq!(rows_read(&data(json!(rows))), Ok(rows));
            assert_eq!(evaluate(rows).is_err(), rows >= THRESHOLD);
        }
        for errors in [Value::Null, json!([])] {
            let mut value = data(json!(123));
            value["errors"] = errors;
            assert_eq!(rows_read(&value), Ok(123));
        }
        assert!(!QUERY.contains("databaseId")); // Free quota is account-wide.
        assert!(!QUERY.contains("dimensions")); // Aggregate all databases into one group.
    }
    #[test]
    fn missing_partial_or_invalid_analytics_never_reports_zero() {
        for value in [
            json!({}),
            json!({"errors":[{"message":"private upstream text"}]}),
            json!({"errors":[{}],"data":{"viewer":{"accounts":[]}}}),
            json!({"data":{"viewer":{"accounts":[]}}}),
            json!({"data":{"viewer":{"accounts":[{"d1AnalyticsAdaptiveGroups":[]}]}}}),
            data(Value::Null),
            data(json!(-1)),
            data(json!("3000000")),
        ] {
            assert!(rows_read(&value).is_err());
        }
    }
}
