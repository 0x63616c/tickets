#![cfg(not(target_arch = "wasm32"))]
mod common;
use common::Sqlite;
use futures::executor::block_on;
use serde_json::{Value, json};
use tickets_worker::api::handle;

async fn call(db: &Sqlite, method: &str, path: &str, body: Value, status: u16) -> Value {
    let (path, query) = path.split_once('?').unwrap_or((path, ""));
    let query = query
        .split('&')
        .filter_map(|s| s.split_once('='))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect::<Vec<_>>();
    let result = handle(db, method, path, &query, body).await;
    assert_eq!(result.status, status, "{}", result.body);
    result.body
}
#[test]
fn mutations_reads_history_claim_and_validation() {
    block_on(async {
        let db = Sqlite::new();
        call(
            &db,
            "POST",
            "/api/v1/projects",
            json!({"name":"Example","key_prefix":"EX","color":"#123456"}),
            201,
        )
        .await;
        let ticket = call(&db,"POST","/api/v1/tickets",json!({"project_id":1,"title":"Grid: home","status":"todo","type":"feat","scope":" UI "}),201).await;
        assert_eq!(ticket["type"], "feat");
        assert_eq!(ticket["scope"], "ui");
        assert_eq!(ticket["pr_title"], "feat(ui): Grid: home [EX-1]");
        for path in ["/api/v1/tickets/1", "/api/v1/tickets/EX-1"] {
            assert_eq!(
                call(&db, "GET", path, Value::Null, 200).await["scope"],
                "ui"
            );
        }
        for path in ["/api/v1/tickets", "/api/v1/search?q=Grid"] {
            assert_eq!(
                call(&db, "GET", path, Value::Null, 200).await["data"][0]["type"],
                "feat"
            );
        }
        for method in ["GET", "POST"] {
            let path = if method == "GET" {
                "/api/v1/next"
            } else {
                "/api/v1/next/claim"
            };
            let picked = call(&db, method, path, json!({}), 200).await;
            assert_eq!(picked["ticket"]["type"], "feat");
            assert_eq!(picked["ticket"]["scope"], "ui");
            assert_eq!(picked["ticket"]["pr_title"], "feat(ui): Grid: home [EX-1]");
        }
        call(
            &db,
            "PATCH",
            "/api/v1/tickets/1",
            json!({"type":"fix","scope":" API/Client._- "}),
            200,
        )
        .await;
        let history = call(&db, "GET", "/api/v1/tickets/1/history", Value::Null, 200).await;
        for (field, old, new) in [("type", "feat", "fix"), ("scope", "ui", "api/client._-")] {
            assert!(
                history["data"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|h| h["field"] == field && h["old_value"] == old && h["new_value"] == new)
            );
        }
        call(
            &db,
            "PATCH",
            "/api/v1/tickets/1",
            json!({"type":"fix","scope":" API/CLIENT._- "}),
            200,
        )
        .await;
        assert_eq!(
            call(&db, "GET", "/api/v1/tickets/1/history", Value::Null, 200).await["data"],
            history["data"]
        );
        let cleared = call(
            &db,
            "PATCH",
            "/api/v1/tickets/1",
            json!({"type":null,"scope":"  "}),
            200,
        )
        .await;
        assert!(cleared["type"].is_null());
        assert!(cleared["scope"].is_null());
        assert_eq!(cleared["pr_title"], "chore: Grid: home [EX-1]");
        call(
            &db,
            "PUT",
            "/api/v1/tickets/by-external?id=example:1",
            json!({"project_id":1,"title":"Plain","type":"build","scope":" CI "}),
            200,
        )
        .await;
        let preserved = call(
            &db,
            "PUT",
            "/api/v1/tickets/by-external?id=example:1",
            json!({"project_id":1,"title":"Plain"}),
            200,
        )
        .await;
        assert_eq!(preserved["type"], "build");
        assert_eq!(preserved["scope"], "ci");
        let cleared = call(
            &db,
            "PUT",
            "/api/v1/tickets/by-external?id=example:1",
            json!({"project_id":1,"title":"Plain","type":null,"scope":null}),
            200,
        )
        .await;
        assert!(cleared["type"].is_null());
        assert!(cleared["scope"].is_null());
        for (method, path) in [
            ("POST", "/api/v1/tickets"),
            ("PATCH", "/api/v1/tickets/1"),
            ("PUT", "/api/v1/tickets/by-external?id=example:2"),
        ] {
            for bad in [
                json!({"type":"Feature"}),
                json!({"type":"Feat"}),
                json!({"type":""}),
                json!({"type":2}),
                json!({"scope":"a b"}),
                json!({"scope":"-ui"}),
                json!({"scope":"ui!"}),
                json!({"scope":"a".repeat(33)}),
                json!({"scope":false}),
            ] {
                let mut input = json!({"project_id":1,"title":"Plain"});
                input
                    .as_object_mut()
                    .unwrap()
                    .extend(bad.as_object().unwrap().clone());
                let error = call(&db, method, path, input, 422).await;
                assert!(error["error"]["message"].as_str().unwrap().contains(
                    if bad.get("type").is_some() {
                        "type"
                    } else {
                        "scope"
                    }
                ));
            }
        }
    });
}

#[test]
fn migration_backfills_prefixes_and_labels_without_rewriting_typed_rows() {
    let db = rusqlite::Connection::open_in_memory().unwrap();
    db.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
    let mut files = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    files.sort();
    for file in files
        .iter()
        .filter(|p| p.file_name().unwrap().to_str().unwrap() < "0021")
    {
        db.execute_batch(&std::fs::read_to_string(file).unwrap())
            .unwrap();
    }
    db.execute_batch(
        "INSERT INTO projects(id,name,key_prefix,color) VALUES(1,'Example','EX','#123456');",
    )
    .unwrap();
    let fixtures = [
        ("Feat(UI): x", Some("feat"), Some("ui"), "x"),
        ("fix: x", Some("fix"), None, "x"),
        ("feat(ui)!: x", Some("feat"), Some("ui"), "x"),
        ("Feature: x", None, None, "Feature: x"),
        ("chore:x", Some("chore"), None, "x"),
        (
            "docs: Keep Case: later",
            Some("docs"),
            None,
            "Keep Case: later",
        ),
        ("CI!: Job", Some("ci"), None, "Job"),
        (
            "refactor(API/Client._-): Keep",
            Some("refactor"),
            Some("api/client._-"),
            "Keep",
        ),
        ("feat(-ui): x", None, None, "feat(-ui): x"),
        ("feat(): x", None, None, "feat(): x"),
        ("feat(ui)!!: x", None, None, "feat(ui)!!: x"),
        ("Plain", None, None, "Plain"),
        ("From label", Some("test"), None, "From label"),
        ("perf: Prefix wins", Some("perf"), None, "Prefix wins"),
        (
            "Colon later: retained",
            Some("build"),
            None,
            "Colon later: retained",
        ),
    ];
    for (i, (title, _, _, _)) in fixtures.iter().enumerate() {
        db.execute(
            "INSERT INTO tickets(id,project_id,number,title) VALUES(?,1,?,?)",
            rusqlite::params![(i + 1) as i64, (i + 1) as i64, title],
        )
        .unwrap();
    }
    db.execute_batch("INSERT INTO labels(id,name,color) VALUES(101,'type:test','#123456'),(102,'type:build','#123456'); INSERT INTO ticket_labels VALUES(13,101),(14,101),(15,102),(4,101);").unwrap();
    db.execute_batch("INSERT INTO ticket_history(subject_id,ticket_id,actor_kind,actor_id,actor_label,field,new_value) SELECT id,ticket_id,'system','fixture','System','title',json_quote('Original') FROM ticket_subjects WHERE ticket_id=1;").unwrap();
    let history_id: i64 = db
        .query_row(
            "SELECT id FROM unified_ticket_history WHERE ticket_id=1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let sql = include_str!("../../../migrations/0021_ticket_type_scope.sql");
    db.execute_batch(sql).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT id FROM unified_ticket_history WHERE ticket_id=1",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        history_id
    );
    for (i, (_, kind, scope, title)) in fixtures.iter().enumerate() {
        let actual = db
            .query_row(
                "SELECT type,scope,title FROM tickets WHERE id=?",
                [(i + 1) as i64],
                |r| {
                    Ok((
                        r.get::<_, Option<String>>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            actual,
            (
                kind.map(str::to_owned),
                scope.map(str::to_owned),
                title.to_string()
            ),
            "fixture {}",
            i + 1
        );
    }
    db.execute(
        "UPDATE tickets SET type='fix',scope='core',title='feat(ui): already typed' WHERE id=12",
        [],
    )
    .unwrap();
    let before: Vec<(i64, String)> = db
        .prepare("SELECT id,title||coalesce(type,'')||coalesce(scope,'')||updated_at FROM tickets ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let version: i64 = db
        .query_row("SELECT version FROM workspace_version", [], |r| r.get(0))
        .unwrap();
    let history_count: i64 = db
        .query_row("SELECT count(*) FROM ticket_history", [], |r| r.get(0))
        .unwrap();
    db.execute_batch(
        sql.split("-- Backfill is independently repeatable:")
            .nth(1)
            .unwrap()
            .split_once('\n')
            .unwrap()
            .1,
    )
    .unwrap();
    let after: Vec<(i64, String)> = db
        .prepare("SELECT id,title||coalesce(type,'')||coalesce(scope,'')||updated_at FROM tickets ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(before, after);
    assert_eq!(
        db.query_row("SELECT version FROM workspace_version", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        version
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM ticket_history", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        history_count
    );
    // Existing FTS triggers follow the plain title; all old history/sequence rows survive.
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM tickets_search WHERE tickets_search MATCH 'Prefix'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert!(
        db.execute("UPDATE tickets SET type='feature' WHERE id=1", [])
            .is_err()
    );
}
