#![cfg(not(target_arch = "wasm32"))]
mod common;
use common::Sqlite;
use futures::executor::block_on;
use serde_json::{Value, json};
use tickets_worker::{api::handle, repository::Store};

async fn event(
    db: &Sqlite,
    number: i64,
    state: &str,
    title: &str,
    body: &str,
    time: &str,
) -> Vec<Value> {
    tickets_worker::mutation::pull_request(
        db,
        vec![
            json!("owner/repo"),
            json!(number),
            json!(format!("https://github.com/owner/repo/pull/{number}")),
            json!(title),
            json!(state),
            json!("abc123"),
            json!(time),
            json!(serde_json::to_string(&tickets_core::pull_request::links(title, body)).unwrap()),
        ],
    )
    .await
    .unwrap()
}
fn setup(status: &str) -> Sqlite {
    let db = Sqlite::new();
    db.0.execute_batch("INSERT INTO projects(name,key_prefix,color) VALUES('Tickets','TIK','#123456'); INSERT INTO tickets(project_id,number,title) VALUES(1,1,'Ticket');").unwrap();
    db.0.execute("UPDATE tickets SET status=?", [status])
        .unwrap();
    db
}
fn status(db: &Sqlite) -> String {
    db.0.query_row("SELECT status FROM tickets WHERE id=1", [], |r| r.get(0))
        .unwrap()
}
#[test]
fn all_status_transitions_and_replays() {
    block_on(async {
        for initial in [
            "backlog",
            "todo",
            "in_progress",
            "in_review",
            "done",
            "canceled",
        ] {
            for state in ["open", "draft", "merged", "closed"] {
                for closing in [true, false] {
                    let db = setup(initial);
                    let body = if closing {
                        "Closes TIK-1"
                    } else {
                        "Refs TIK-1"
                    };
                    event(&db, 1, state, "", body, "2026-10-29T01:00:00Z").await;
                    let expected = match (state, initial, closing) {
                        ("merged", s, true) if s != "canceled" => "done",
                        ("open", "backlog" | "todo" | "in_progress", _) => "in_review",
                        ("draft", "backlog" | "todo", _) => "in_progress",
                        ("closed", "in_progress" | "in_review", _) => "todo",
                        _ => initial,
                    };
                    assert_eq!(status(&db), expected, "{initial}/{state}/{closing}");
                    let rows:Vec<Value>=db.query("SELECT actor_kind,field,old_value,new_value FROM ticket_history ORDER BY id",vec![]).await.unwrap();
                    if expected != initial {
                        assert_eq!(rows.len(), 1);
                        assert_eq!(rows[0]["actor_kind"], "webhook");
                        assert_eq!(rows[0]["field"], "status");
                        assert_eq!(
                            rows[0]["old_value"],
                            serde_json::to_string(initial).unwrap()
                        );
                        assert_eq!(
                            rows[0]["new_value"],
                            serde_json::to_string(expected).unwrap()
                        );
                    } else {
                        assert!(rows.is_empty());
                    }
                    let history_count = rows.len();

                    let comments: i64 =
                        db.0.query_row("SELECT count(*) FROM comments", [], |r| r.get(0))
                            .unwrap();
                    assert!(
                        event(&db, 1, state, "", body, "2026-10-29T01:00:00Z")
                            .await
                            .is_empty()
                    );
                    assert_eq!(
                        db.0.query_row("SELECT count(*) FROM ticket_history", [], |r| r
                            .get::<_, i64>(0))
                            .unwrap(),
                        history_count as i64
                    );
                    assert_eq!(
                        db.0.query_row("SELECT count(*) FROM comments", [], |r| r.get::<_, i64>(0))
                            .unwrap(),
                        comments
                    );
                }
            }
        }
    });
}
#[test]
fn edits_stale_events_multiple_prs_and_key_api() {
    block_on(async {
        let db = setup("todo");
        event(&db, 1, "open", "[TIK-1]", "", "2026-10-29T01:00:00Z").await;
        event(&db, 2, "draft", "", "Refs TIK-1", "2026-10-29T01:00:00Z").await;
        event(&db, 1, "closed", "[TIK-1]", "", "2026-10-29T02:00:00Z").await;
        assert_eq!(status(&db), "in_review");
        event(&db, 2, "closed", "", "Refs TIK-1", "2026-10-29T02:00:00Z").await;
        assert_eq!(status(&db), "todo");
        event(&db, 1, "open", "[TIK-1]", "", "2026-10-29T03:00:00Z").await;
        assert_eq!(status(&db), "in_review");
        event(&db, 1, "open", "No key", "", "2026-10-29T04:00:00Z").await;
        event(&db, 1, "open", "[TIK-1]", "", "2026-10-29T03:00:00Z").await;
        let response = handle(&db, "GET", "/api/v1/tickets/TIK-1", &[], Value::Null).await;
        assert_eq!(response.status, 200);
        assert_eq!(response.body["status"], "in_review");
        assert_eq!(response.body["pull_requests"].as_array().unwrap().len(), 1);
        assert_eq!(response.body["pull_requests"][0]["head_sha"], "abc123");
        assert_eq!(
            event(&db, 3, "open", "[UNKNOWN-1]", "", "2026-10-29T04:00:00Z").await[0]["matches"],
            0
        );
        event(&db, 2, "merged", "[TIK-1]", "", "2026-10-29T05:00:00Z").await;
        event(&db, 2, "open", "[TIK-1]", "", "2026-10-29T05:00:00Z").await;
        assert_eq!(status(&db), "done");
    });
}

#[test]
fn draft_review_edit_and_many_keys_are_atomic() {
    block_on(async {
        let db = setup("backlog");
        event(&db, 1, "draft", "[TIK-1]", "", "2026-10-29T01:00:00Z").await;
        assert_eq!(status(&db), "in_progress");
        event(&db, 1, "open", "[TIK-1]", "", "2026-10-29T02:00:00Z").await;
        assert_eq!(status(&db), "in_review");
        event(&db, 1, "draft", "[TIK-1]", "", "2026-10-29T03:00:00Z").await;
        assert_eq!(status(&db), "in_review");
        event(
            &db,
            1,
            "open",
            "[TIK-1]",
            "Refs TIK-1",
            "2026-10-29T04:00:00Z",
        )
        .await;
        event(
            &db,
            1,
            "merged",
            "[TIK-1]",
            "Refs TIK-1",
            "2026-10-29T05:00:00Z",
        )
        .await;
        assert_eq!(status(&db), "in_review");
        for n in 2..=100 {
            db.0.execute(
                "INSERT INTO tickets(project_id,number,title) VALUES(1,?,'Many keys')",
                [n],
            )
            .unwrap();
        }
        let title = (1..=100)
            .map(|n| format!("[TIK-{n}]"))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            event(&db, 2, "open", &title, "", "2026-10-29T06:00:00Z").await[0]["matches"],
            100
        );
        assert_eq!(
            db.0.query_row(
                "SELECT count(*) FROM ticket_pull_requests WHERE number=2",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            100
        );
        // Removing one key preserves the others without reallocating tickets.
        event(&db, 2, "open", "[TIK-2]", "", "2026-10-29T07:00:00Z").await;
        assert_eq!(
            db.0.query_row(
                "SELECT count(*) FROM ticket_pull_requests WHERE number=2",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        db.0.execute("DELETE FROM tickets WHERE id=2", []).unwrap();
        assert_eq!(
            db.0.query_row(
                "SELECT count(*) FROM ticket_pull_requests WHERE ticket_id=2",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
    });
}
