#![cfg(not(target_arch = "wasm32"))]
mod common;
use common::Sqlite;
use futures::executor::block_on;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::cell::RefCell;
use tickets_worker::{
    api::handle,
    repository::{Result, Store},
};

struct Capture<'a> {
    db: &'a Sqlite,
    statements: RefCell<Vec<String>>,
}
impl Store for Capture<'_> {
    async fn query<T: DeserializeOwned>(&self, sql: &str, params: Vec<Value>) -> Result<Vec<T>> {
        self.statements.borrow_mut().push(sql.into());
        self.db.query(sql, params).await
    }
}
#[test]
fn list_plans_use_covering_counts_and_indexed_relationships() {
    block_on(async {
        let db = Sqlite::new();
        db.0.execute_batch("INSERT INTO projects(name,key_prefix,color) VALUES('Fixture','FIX','#5e6ad2'); INSERT INTO tickets(project_id,number,title) VALUES(1,1,'Fixture');").unwrap();
        let capture = Capture {
            db: &db,
            statements: RefCell::new(vec![]),
        };
        for (path, index) in [
            ("/api/v1/projects", "tickets_project_status_activity"),
            ("/api/v1/tickets", "comments_visible_ticket"),
        ] {
            let reply = handle(&capture, "GET", path, &[], Value::Null).await;
            assert_eq!(reply.status, 200, "{}", reply.body);
            let sql = capture.statements.borrow().last().unwrap().clone();
            let mut statement = db.0.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
            let plan = statement
                .query_map([0, 51], |row| row.get::<_, String>(3))
                .unwrap()
                .collect::<std::result::Result<Vec<_>, _>>()
                .unwrap()
                .join("\n");
            assert!(plan.contains(index), "{plan}");
            if path.ends_with("tickets") {
                assert!(sql.contains("JOIN projects ticket_project"));
                assert!(plan.contains("sqlite_autoindex_ticket_labels_1"), "{plan}");
                assert!(!plan.contains("SCAN comments"), "{plan}");
            }
        }
        let reply = handle(
            &capture,
            "GET",
            "/api/v1/tickets",
            &[("label".into(), "1".into())],
            Value::Null,
        )
        .await;
        assert_eq!(reply.status, 200);
        let sql = capture.statements.borrow().last().unwrap().clone();
        let mut statement = db.0.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
        let plan = statement
            .query_map([0, 1, 51], |r| r.get::<_, String>(3))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap()
            .join("\n");
        assert!(plan.contains("ticket_labels_label_ticket"), "{plan}");
    });
}
#[test]
fn old_writer_relationship_and_history_changes_advance_version() {
    let db = Sqlite::new();
    let version = || {
        db.0.query_row(
            "SELECT version FROM workspace_version WHERE id=1",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
    };
    let mut previous = version();
    for sql in [
        "INSERT INTO projects(name,key_prefix,color) VALUES('Fixture','FIX','#5e6ad2')",
        "INSERT INTO tickets(project_id,number,title) VALUES(1,1,'One'),(1,2,'Two')",
        "UPDATE tickets SET title='Changed' WHERE id=1",
        "INSERT INTO labels(name,color) VALUES('bug','#5e6ad2')",
        "INSERT INTO ticket_labels VALUES(1,1)",
        "INSERT INTO ticket_dependencies VALUES(1,2)",
        "INSERT INTO comments(ticket_id,author,body,author_kind,author_id) VALUES(1,'Fixture','Comment','user','1')",
        "UPDATE comments SET deleted_at='2026-10-30T00:00:00.000Z' WHERE id=1",
        "DELETE FROM ticket_dependencies WHERE blocker_id=1",
        "DELETE FROM ticket_labels WHERE ticket_id=1",
        "UPDATE analytics_coverage SET started_at='2026-10-29T00:00:00.000Z' WHERE id=1",
        "UPDATE projects SET archived=1 WHERE id=1",
        "DELETE FROM tickets WHERE id=2",
    ] {
        db.0.execute_batch(sql).unwrap();
        assert!(version() > previous, "{sql}");
        previous = version();
    }
    let response = block_on(handle(&db, "GET", "/api/v1/projects", &[], json!(null)));
    assert_eq!(response.status, 200);
    assert_eq!(response.body["data"][0]["open_ticket_count"], 1);
}

#[test]
fn ci_versions_are_limited_to_linked_current_heads() {
    block_on(async {
        let db = Sqlite::new();
        let version = || {
            db.0.query_row(
                "SELECT version FROM workspace_version WHERE id=1",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
        };
        db.0.execute_batch("INSERT INTO projects(name,key_prefix,color) VALUES('Fixture','FIX','#5e6ad2'); INSERT INTO tickets(project_id,number,title) VALUES(1,1,'One');").unwrap();
        let before = version();
        db.0.execute_batch("INSERT INTO github_pull_request_checks(repo,number,head_sha,ci_status) VALUES('example/repo',1,'old','success');").unwrap();
        assert_eq!(version(), before, "unlinked CI must not invalidate lists");
        db.0.execute_batch("INSERT INTO ticket_pull_requests(ticket_id,repo,number,url,title,state,closes,head_sha,updated_at) VALUES(1,'example/repo',1,'https://example.test/pr/1','Example','open',1,'current','2026-10-30T00:00:00.000Z');").unwrap();
        let linked = version();
        db.0.execute_batch(
            "UPDATE github_pull_request_checks SET ci_status='failure' WHERE head_sha='old';",
        )
        .unwrap();
        assert_eq!(
            version(),
            linked,
            "stale-head CI must not invalidate current lists"
        );
        db.0.execute_batch("INSERT INTO github_pull_request_checks(repo,number,head_sha,ci_status) VALUES('example/repo',1,'current','success');").unwrap();
        assert!(version() > linked);
    });
}

#[test]
fn project_activity_range_uses_project_updated_index_without_sorting() {
    let db = Sqlite::new();
    let mut statement = db.0.prepare("EXPLAIN QUERY PLAN SELECT id FROM tickets WHERE project_id=? AND updated_at>=? ORDER BY updated_at").unwrap();
    let plan = statement
        .query_map(rusqlite::params![1, "2026-01-01"], |row| {
            row.get::<_, String>(3)
        })
        .unwrap()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap()
        .join("\n");
    assert!(plan.contains("tickets_project_updated_at"), "{plan}");
    assert!(!plan.contains("TEMP B-TREE"), "{plan}");
}
