//! Mission snapshots behind the normal auth gate. No per-ticket reads.
use crate::{
    api::Reply,
    repository::{Error, Result, Statement, Store},
};
use serde_json::{Value, json};
use tickets_core::mission::{MAX_REPORT_BYTES, Mission, Report, Ticket};

/// Bound the transport before decoding, including requests without Content-Length.
#[cfg(target_arch = "wasm32")]
pub async fn receive<S: Store>(
    req: &mut worker::Request,
    store: &S,
    source: &str,
) -> worker::Result<Reply> {
    use futures_util::StreamExt;
    let reject = |status, message| Reply {
        status,
        change: None,
        body: json!({"error":{"code":"validation","message":message}}),
    };
    if req
        .headers()
        .get("Content-Length")?
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|len| len > MAX_REPORT_BYTES)
    {
        return Ok(reject(413, "report exceeds 512 KB"));
    }
    let mut bytes = Vec::new();
    let mut stream = req.stream()?;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if chunk.len() > MAX_REPORT_BYTES.saturating_sub(bytes.len()) {
            return Ok(reject(413, "report exceeds 512 KB"));
        }
        bytes.extend_from_slice(&chunk);
    }
    let raw = match String::from_utf8(bytes) {
        Ok(raw) => raw,
        Err(_) => return Ok(reject(422, "report must be UTF-8 JSON")),
    };
    Ok(put(store, source, &raw).await)
}

pub async fn put<S: Store>(store: &S, source: &str, raw: &str) -> Reply {
    let result = async {
        tickets_core::mission::validate(source, raw).map_err(Error::Validation)?;
        let rows: Vec<Value> = store.query(
            "INSERT INTO mission_reports(source,body,received_at) VALUES(?,?,strftime('%Y-%m-%dT%H:%M:%fZ','now')) ON CONFLICT(source) DO UPDATE SET body=excluded.body,received_at=excluded.received_at RETURNING source,received_at",
            vec![source.into(), raw.into()],
        ).await?;
        rows.into_iter().next().ok_or_else(|| Error::Storage("Missing report receipt".into()))
    }.await;
    match result {
        Ok(body) => Reply {
            status: 200,
            body,
            change: None,
        },
        Err(error) => {
            let (status, code, message) = match error {
                Error::Validation(message) => (
                    if raw.len() > MAX_REPORT_BYTES {
                        413
                    } else {
                        422
                    },
                    "validation",
                    message,
                ),
                _ => (500, "internal", "Could not store report".into()),
            };
            Reply {
                status,
                body: json!({"error":{"code":code,"message":message}}),
                change: None,
            }
        }
    }
}

pub async fn get<S: Store>(store: &S) -> Result<Value> {
    // A single atomic batch gives the report clock and queues one consistent snapshot.
    let ticket_sql = "SELECT t.id,p.key_prefix||'-'||t.number key,t.title,p.name project,t.status,t.updated_at,
      (SELECT json_group_array(json_object('id',l.id,'name',l.name)) FROM ticket_labels tl JOIN labels l ON l.id=tl.label_id WHERE tl.ticket_id=t.id) labels,
      (SELECT json_group_array(json_object('repo',pr.repo,'number',pr.number,'url',pr.url,'title',pr.title,'state',pr.state,'updated_at',pr.updated_at,'ci_status',coalesce((SELECT c.ci_status FROM github_pull_request_checks c WHERE c.repo=pr.repo AND c.number=pr.number AND c.head_sha=pr.head_sha ORDER BY c.ci_updated_at DESC,c.updated_at DESC LIMIT 1),'none'))) FROM ticket_pull_requests pr WHERE pr.ticket_id=t.id) pull_requests,
      (SELECT max(e.recorded_at) FROM ticket_events e JOIN ticket_subjects s ON s.id=e.subject_id WHERE s.ticket_id=t.id AND e.kind IN ('status','created') AND e.new_value='done') done_at
      FROM tickets t JOIN projects p ON p.id=t.project_id WHERE p.archived=0 AND
      (t.status='in_review' OR EXISTS(SELECT 1 FROM ticket_labels tl JOIN labels l ON l.id=tl.label_id WHERE tl.ticket_id=t.id AND l.name='waiting-on-calum' COLLATE NOCASE) OR
      t.id IN (SELECT dt.id FROM tickets dt JOIN projects dp ON dp.id=dt.project_id JOIN ticket_subjects s ON s.ticket_id=dt.id JOIN ticket_events e ON e.subject_id=s.id WHERE dp.archived=0 AND dt.status='done' AND e.kind IN ('status','created') AND e.new_value='done' AND e.recorded_at>=strftime('%Y-%m-%dT%H:%M:%fZ','now','-48 hours') GROUP BY dt.id ORDER BY max(e.recorded_at) DESC,dt.updated_at DESC,dt.id DESC LIMIT 30))
      ORDER BY done_at DESC,t.updated_at DESC,t.id DESC";
    let mut sets = store.batch(vec![
        Statement::new("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now') generated_at,strftime('%Y-%m-%dT%H:%M:%fZ','now','-48 hours') recent_since",vec![]),
        Statement::new("SELECT source,body,received_at,received_at<strftime('%Y-%m-%dT%H:%M:%fZ','now','-10 minutes') stale FROM mission_reports ORDER BY source",vec![]),
        Statement::new(ticket_sql,vec![]),
    ]).await?.into_iter();
    let clock = sets.next().unwrap_or_default();
    let generated_at = clock
        .first()
        .and_then(|r| r["generated_at"].as_str())
        .ok_or_else(|| Error::Storage("Missing mission clock".into()))?
        .to_owned();
    let recent_since = clock[0]["recent_since"].as_str().unwrap_or_default();
    let reports = sets
        .next()
        .unwrap_or_default()
        .into_iter()
        .map(|r| {
            Ok(Report {
                source: r["source"].as_str().unwrap_or_default().into(),
                received_at: r["received_at"].as_str().unwrap_or_default().into(),
                stale: r["stale"] == 1,
                report: serde_json::from_str(r["body"].as_str().unwrap_or_default())
                    .map_err(|e| Error::Storage(e.to_string()))?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let mut mission = Mission {
        generated_at,
        reports,
        waiting_on_calum: vec![],
        in_review: vec![],
        recently_done: vec![],
    };
    for mut row in sets.next().unwrap_or_default() {
        for field in ["labels", "pull_requests"] {
            row[field] = serde_json::from_str(row[field].as_str().unwrap_or("[]"))
                .map_err(|e| Error::Storage(e.to_string()))?;
        }
        let done_at = row["done_at"].as_str().map(str::to_owned);
        let ticket: Ticket =
            serde_json::from_value(row).map_err(|e| Error::Storage(e.to_string()))?;
        if ticket
            .labels
            .iter()
            .any(|l| l.name.eq_ignore_ascii_case("waiting-on-calum"))
        {
            mission.waiting_on_calum.push(ticket.clone());
        }
        if ticket.status == "in_review" {
            mission.in_review.push(ticket.clone());
        }
        if ticket.status == "done"
            && done_at.as_deref().is_some_and(|date| date >= recent_since)
            && mission.recently_done.len() < 30
        {
            mission.recently_done.push(ticket);
        }
    }
    serde_json::to_value(mission).map_err(|e| Error::Storage(e.to_string()))
}
