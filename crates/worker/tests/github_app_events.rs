#![cfg(not(target_arch = "wasm32"))]
mod common;
use common::Sqlite;
use futures::executor::block_on;
use serde_json::{Value, json};
use tickets_worker::{api, github_app_events as events, repository::Store};

#[test]
fn overview_exposes_only_public_metadata_and_sorted_repositories() {
    block_on(async {
        let db = Sqlite::new();
        assert_eq!(
            events::overview(&db).await.unwrap(),
            json!({"registered":false,"app_slug":null,"app_name":null,"html_url":null,"installations":[]})
        );
        db.0.execute_batch("INSERT INTO github_app VALUES(1,42,'fixture','Fixture','owner','client','https://github.com/apps/fixture','not-for-output','2026-10-30T00:00:00Z');").unwrap();
        let installation =
            json!({"id":7,"account":{"login":"example"},"repository_selection":"all"});
        events::installation(&db,"installation",&json!({"action":"created","installation":installation,"repositories":[{"full_name":"example/z"},{"full_name":"Example/A"}]})).await.unwrap();
        let reply = api::handle(&db, "GET", "/api/v1/github/app", &[], Value::Null).await;
        assert_eq!(reply.status, 200);
        assert_eq!(
            reply.body,
            json!({"registered":true,"app_slug":"fixture","app_name":"Fixture","html_url":"https://github.com/apps/fixture","installations":[{"account_login":"example","repository_selection":"all","repos":["example/a","example/z"]}]})
        );
    });
}

#[test]
fn check_results_do_not_change_ticket_status_and_only_current_head_is_exposed() {
    block_on(async {
        let db = Sqlite::new();
        db.0.execute_batch("INSERT INTO projects(name,key_prefix,color) VALUES('Example','TIK','#5e6ad2'); INSERT INTO tickets(project_id,number,title,status) VALUES(1,1,'Example','todo');").unwrap();
        let ingest = |sha: &str, time: &str| {
            vec![
                json!("example/repo"),
                json!(1),
                json!("https://github.com/example/repo/pull/1"),
                json!("[TIK-1]"),
                json!("open"),
                json!(sha),
                json!(time),
                json!(r#"[{"prefix":"TIK","number":1,"closes":true}]"#),
            ]
        };
        tickets_worker::mutation::pull_request(&db, ingest("a", "2026-10-30T00:00:00.000Z"))
            .await
            .unwrap();
        events::save_check(&db, "example/repo", 1, "a", "success", 101)
            .await
            .unwrap();
        events::save_check(&db, "example/repo", 1, "a", "neutral", 100)
            .await
            .unwrap();
        events::save_ci(&db,"example/repo",&json!({"head_sha":"a","updated_at":"2026-10-30T12:00:00Z","conclusion":"failure","pull_requests":[{"number":1}]})).await.unwrap();
        let ticket = api::handle(&db, "GET", "/api/v1/tickets/1", &[], Value::Null).await;
        assert_eq!(ticket.status, 200);
        assert_eq!(ticket.body["status"], "in_review");
        assert_eq!(ticket.body["pull_requests"][0]["tickets_check"], "success");
        assert_eq!(ticket.body["pull_requests"][0]["tickets_check_run_id"], 101);
        assert!(ticket.body["pull_requests"][0]["checks_updated_at"].is_string());
        assert_eq!(ticket.body["pull_requests"][0]["ci_status"], "failure");
        // GitHub may omit PRs (for example on fork checks); match known head snapshots.
        events::save_ci(&db,"example/repo",&json!({"head_sha":"a","updated_at":"2026-10-30T12:01:00Z","conclusion":"success","pull_requests":[]})).await.unwrap();
        let ticket = api::handle(&db, "GET", "/api/v1/tickets/1", &[], Value::Null).await;
        assert_eq!(ticket.body["pull_requests"][0]["ci_status"], "success");
        // User status wins; a check update does not reapply PR triggers.
        db.0.execute("UPDATE tickets SET status='todo' WHERE id=1", [])
            .unwrap();
        events::save_check(&db, "example/repo", 1, "a", "neutral", 102)
            .await
            .unwrap();
        let rows: Vec<Value> = db
            .query("SELECT status FROM tickets", vec![])
            .await
            .unwrap();
        assert_eq!(rows[0]["status"], "todo");
        tickets_worker::mutation::pull_request(&db, ingest("b", "2026-10-30T00:00:01.000Z"))
            .await
            .unwrap();
        let ticket = api::handle(&db, "GET", "/api/v1/tickets/1", &[], Value::Null).await;
        assert!(ticket.body["pull_requests"][0]["tickets_check"].is_null());
        assert!(ticket.body["pull_requests"][0]["ci_status"].is_null());
    });
}
