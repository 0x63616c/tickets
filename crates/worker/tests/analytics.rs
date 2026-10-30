#![cfg(not(target_arch = "wasm32"))]
mod common;
use common::Sqlite;
use futures::executor::block_on;
use serde_json::{Value, json};
use tickets_worker::{
    api::handle,
    repository::{Result, Store},
};

fn seed(db: &Sqlite) {
    db.0.execute_batch("INSERT INTO projects(name,key_prefix,color) VALUES('Example','EX','#5e6ad2'),('Archive','OLD','#8a8f98'); UPDATE projects SET archived=1 WHERE id=2;").unwrap();
}
async fn home(db: &impl Store, q: &[(&str, &str)]) -> Value {
    let q = q
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect::<Vec<_>>();
    let r = handle(db, "GET", "/api/v1/analytics/home", &q, Value::Null).await;
    assert_eq!(r.status, 200, "{}", r.body);
    // Runtime and web share the same typed response contract.
    serde_json::from_value::<tickets_core::analytics::Home>(r.body.clone()).unwrap();
    r.body
}
fn count(db: &Sqlite, table: &str) -> i64 {
    db.0.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}
#[test]
fn inventory_coverage_imports_and_exact_completion_cohorts() {
    block_on(async {
        let db = Sqlite::new();
        seed(&db);
        db.0.execute_batch("INSERT INTO tickets(project_id,number,title,status) VALUES(1,1,'Review','in_review'),(1,2,'Work','in_progress'),(2,1,'Archived blocker','todo'); INSERT INTO ticket_dependencies VALUES(3,1); INSERT INTO tickets(project_id,number,title,status,external_id) VALUES(1,3,'Imported done','done','github:example/repo:1'); UPDATE tickets SET status='done' WHERE id=2; UPDATE tickets SET status='todo' WHERE id=2; UPDATE tickets SET status='done' WHERE id=2; UPDATE tickets SET status='in_progress' WHERE id=2;").unwrap();
        let h = home(&db, &[]).await;
        assert_eq!(h["in_flight"], 2);
        assert_eq!(h["in_review"], 1);
        assert_eq!(h["blocked"], 1);
        assert_eq!(h["blocked_queue"][0]["blocker_key"], "OLD-1");
        assert_eq!(h["projects"].as_array().unwrap().len(), 1);
        assert_eq!(h["weeks"][0]["coverage"], "none");
        assert!(h["weeks"][0]["completed"].is_null());
        assert_eq!(h["weeks"][7]["created"], 2);
        assert_eq!(h["weeks"][7]["imported"], 1);
        assert_eq!(h["weeks"][7]["completed"], 1);
        assert_eq!(h["weeks"][7]["coverage"], "partial");
        let c = home(&db, &[("metric", "completed")]).await;
        assert_eq!(c["cohort_total"], 1);
        assert_eq!(c["cohort"][0]["status"], "in_progress");
        let b = home(&db, &[("metric", "blocked")]).await;
        assert_eq!(b["cohort_total"], h["blocked"]);
        db.0.execute("UPDATE tickets SET status='canceled' WHERE id=3", [])
            .unwrap();
        assert_eq!(home(&db, &[]).await["blocked"], 0);
    });
}
#[test]
fn inventory_matches_later_presentation_without_changing_history() {
    block_on(async {
        let db = Sqlite::new();
        seed(&db);
        db.0.execute_batch("INSERT INTO tickets(project_id,number,title,status) VALUES(1,1,'Later','todo'),(1,2,'Active later','in_progress'),(1,3,'Todo','todo'); INSERT INTO labels(name,color) VALUES('LaTeR','#888888'); INSERT INTO ticket_labels VALUES(1,1),(2,1);").unwrap();
        let h = home(&db, &[]).await;
        assert_eq!(h["projects"][0]["backlog"], 1);
        assert_eq!(h["projects"][0]["todo"], 1);
        assert_eq!(h["projects"][0]["in_progress"], 1);
        let backlog = home(&db, &[("metric", "inventory"), ("status", "backlog")]).await;
        assert_eq!(backlog["cohort_total"], 1);
        assert_eq!(backlog["cohort"][0]["key"], "EX-1");
        let todo = home(&db, &[("metric", "inventory"), ("status", "todo")]).await;
        assert_eq!(todo["cohort_total"], 1);
        assert_eq!(todo["cohort"][0]["key"], "EX-3");
        assert_eq!(h["weeks"][7]["created"], 3);
        assert_eq!(
            db.0.query_row(
                "SELECT count(*) FROM ticket_events WHERE kind='status'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
    });
}

#[test]
fn events_are_atomic_ignore_edits_and_survive_deletion_and_id_reuse() {
    block_on(async {
        let db = Sqlite::new();
        seed(&db);
        db.0.execute_batch(
            "INSERT INTO tickets(project_id,number,title,status) VALUES(1,1,'Original','todo');",
        )
        .unwrap();
        let initial = count(&db, "ticket_events");
        db.0.execute_batch("UPDATE tickets SET title='Edited',sort_order=42,status='todo';")
            .unwrap();
        assert_eq!(count(&db, "ticket_events"), initial);
        db.0.execute_batch("BEGIN; UPDATE tickets SET status='done'; ROLLBACK;")
            .unwrap();
        assert_eq!(count(&db, "ticket_events"), initial);
        db.0.execute_batch("UPDATE tickets SET status='done'; DELETE FROM tickets; INSERT INTO tickets(id,project_id,number,title) VALUES(1,1,2,'Replacement');").unwrap();
        let h = home(&db, &[("metric", "completed")]).await;
        assert_eq!(h["cohort_total"], 1);
        assert!(h["cohort"][0]["ticket_id"].is_null());
        assert!(h["cohort"][0]["title"].is_null());
        assert_eq!(count(&db, "ticket_subjects"), 2);
        // Event failures roll the corresponding domain mutation back.
        db.0.execute_batch("CREATE TRIGGER reject_history BEFORE INSERT ON ticket_events BEGIN SELECT RAISE(ABORT,'fixture history failure'); END;").unwrap();
        assert!(
            db.0.execute("UPDATE tickets SET status='done'", [])
                .is_err()
        );
        assert_eq!(
            db.0.query_row("SELECT status FROM tickets", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "backlog"
        );
    });
}
#[test]
fn migration_does_not_invent_completion_or_historical_project() {
    let db = rusqlite::Connection::open_in_memory().unwrap();
    db.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
    let mut files = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|s| s == "sql"))
        .collect::<Vec<_>>();
    files.sort();
    for file in files
        .iter()
        .filter(|p| p.file_name().unwrap().to_str().unwrap() < "0011")
    {
        db.execute_batch(&std::fs::read_to_string(file).unwrap())
            .unwrap();
    }
    db.execute_batch("INSERT INTO projects(name,key_prefix,color) VALUES('Example','EX','#123456'); INSERT INTO tickets(project_id,number,title,status,created_at) VALUES(1,1,'Old done','done','2025-01-01T00:00:00.000Z');").unwrap();
    db.execute_batch(include_str!("../../../migrations/0011_home_history.sql"))
        .unwrap();
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM ticket_events WHERE kind='status'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(db.query_row("SELECT recorded_at FROM ticket_events WHERE kind='recovered_created' AND project_id IS NULL",[],|r|r.get::<_,String>(0)).unwrap(),"2025-01-01T00:00:00.000Z");
}
#[test]
fn weekly_boundaries_deduplication_and_project_at_completion() {
    block_on(async {
        let db = Sqlite::new();
        seed(&db);
        db.0.execute_batch("UPDATE projects SET archived=0; INSERT INTO tickets(project_id,number,title) VALUES(1,1,'Moving'); UPDATE tickets SET status='done'; UPDATE tickets SET project_id=2,status='todo'; UPDATE tickets SET status='done'; UPDATE analytics_coverage SET started_at='2020-01-01T00:00:00.000Z';").unwrap();
        let all = home(&db, &[]).await;
        let start = all["weeks"][6]["start"].as_str().unwrap();
        db.0.execute(
            "UPDATE ticket_events SET recorded_at=? WHERE kind='status' AND project_id=1",
            [start],
        )
        .unwrap();
        let h = home(&db, &[]).await;
        assert_eq!(h["weeks"][5]["completed"], 0);
        assert_eq!(h["weeks"][6]["completed"], 1);
        assert_eq!(h["weeks"][7]["completed"], 1);
        assert_eq!(
            home(
                &db,
                &[("project", "1"), ("metric", "completed"), ("week", "6")]
            )
            .await["cohort_total"],
            1
        );
        assert_eq!(
            home(&db, &[("project", "1"), ("metric", "completed")]).await["cohort_total"],
            0
        );
        assert_eq!(
            home(&db, &[("project", "2"), ("metric", "completed")]).await["cohort_total"],
            1
        );
    });
}
#[test]
fn webhook_replays_state_observations_and_status_history() {
    block_on(async {
        let db = Sqlite::new();
        seed(&db);
        db.0.execute_batch(
            "INSERT INTO tickets(project_id,number,title) VALUES(1,1,'One'),(1,2,'Two');",
        )
        .unwrap();
        for (state, time, title, links) in [
            ("open", "2026-10-29T01:00:00Z", "Title", "Closes EX-1, EX-2"),
            ("open", "2026-10-29T01:00:00Z", "Title", "Closes EX-1, EX-2"),
            (
                "merged",
                "2026-10-29T02:00:00Z",
                "Title",
                "Closes EX-1, EX-2",
            ),
            ("open", "2026-10-29T01:00:00Z", "Stale", "Closes EX-1"),
            (
                "merged",
                "2026-10-29T03:00:00Z",
                "New title",
                "Closes EX-1, EX-2",
            ),
        ] {
            db.query::<Value>(
                tickets_core::pull_request::UPSERT,
                vec![
                    json!("example/repo"),
                    json!(1),
                    json!("https://github.com/example/repo/pull/1"),
                    json!(title),
                    json!(state),
                    json!("abc123"),
                    json!(time),
                    json!(
                        serde_json::to_string(&tickets_core::pull_request::links(title, links))
                            .unwrap()
                    ),
                ],
            )
            .await
            .unwrap();
        }
        assert_eq!(count(&db, "pull_request_history"), 2);
        assert_eq!(
            db.0.query_row(
                "SELECT count(*) FROM ticket_events WHERE kind='status' AND new_value='done'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            2
        );
        assert_eq!(home(&db, &[]).await["weeks"][7]["completed"], 2);
    });
}
#[test]
fn validates_parameters_and_pages_with_one_bounded_query() {
    struct Counting(Sqlite, std::cell::Cell<usize>);
    impl Store for Counting {
        async fn query<T: serde::de::DeserializeOwned>(
            &self,
            sql: &str,
            params: Vec<Value>,
        ) -> Result<Vec<T>> {
            assert!(params.len() <= 100);
            self.1.set(self.1.get() + 1);
            self.0.query(sql, params).await
        }
    }
    block_on(async {
        let db = Counting(Sqlite::new(), std::cell::Cell::new(0));
        seed(&db.0);
        for n in 1..=150 {
            db.0.0.execute("INSERT INTO tickets(project_id,number,title,status) VALUES(1,?,'Fixture','in_review')",[n]).unwrap();
        }
        let h = home(&db, &[("metric", "review")]).await;
        assert_eq!(db.1.get(), 1); // + the common auth gate = two D1 calls.
        assert_eq!(h["cohort"].as_array().unwrap().len(), 50);
        assert_eq!(h["cohort_total"], 150);
        assert_eq!(h["next_cursor"], 50);
        let h = home(&db, &[("metric", "review"), ("cursor", "100")]).await;
        assert_eq!(h["cohort"][0]["subject_id"], 101);
        assert!(h["next_cursor"].is_null());
        for q in [
            vec![("week", "8")],
            vec![("tz", "invalid")],
            vec![("metric", "review"), ("metric", "review")],
            vec![("project", "2")],
            vec![("status", "todo")],
            vec![("week", "0")],
        ] {
            let q = q
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect::<Vec<_>>();
            assert_eq!(
                handle(&db, "GET", "/api/v1/analytics/home", &q, Value::Null)
                    .await
                    .status,
                422
            );
        }
    });
}

#[test]
fn api_creation_patch_and_external_upsert_share_authoritative_history() {
    block_on(async {
        let db = Sqlite::new();
        seed(&db);
        let native = handle(
            &db,
            "POST",
            "/api/v1/tickets",
            &[],
            json!({"project_id":1,"title":"Native","status":"in_progress"}),
        )
        .await;
        assert_eq!(native.status, 201);
        let q = vec![("id".into(), "github:example/repo:42".into())];
        let body = json!({"project_id":1,"title":"Imported","status":"done"});
        assert_eq!(
            handle(&db, "PUT", "/api/v1/tickets/by-external", &q, body.clone())
                .await
                .status,
            200
        );
        let events = count(&db, "ticket_events");
        assert_eq!(
            handle(&db, "PUT", "/api/v1/tickets/by-external", &q, body)
                .await
                .status,
            200
        );
        assert_eq!(count(&db, "ticket_events"), events);
        assert_eq!(
            handle(
                &db,
                "PATCH",
                "/api/v1/tickets/1",
                &[],
                json!({"status":"done"})
            )
            .await
            .status,
            200
        );
        assert_eq!(
            handle(
                &db,
                "PATCH",
                "/api/v1/tickets/2",
                &[],
                json!({"status":"todo"})
            )
            .await
            .status,
            200
        );
        assert_eq!(
            handle(
                &db,
                "PATCH",
                "/api/v1/tickets/2",
                &[],
                json!({"status":"done"})
            )
            .await
            .status,
            200
        );
        let h = home(&db, &[]).await;
        assert_eq!(h["weeks"][7]["created"], 1);
        assert_eq!(h["weeks"][7]["imported"], 1);
        assert_eq!(h["weeks"][7]["completed"], 2);
    });
}
