#![cfg(not(target_arch = "wasm32"))]
mod common;
use common::Sqlite;
use futures::executor::block_on;
use serde_json::{Value, json};
use tickets_core::history::{self, Actor};
use tickets_worker::{
    api::handle_as,
    mutation,
    repository::{Repository, Store},
};

async fn call(db: &Sqlite, actor: &Actor, method: &str, path: &str, body: Value) -> Value {
    let r = handle_as(db, actor, method, path, &[], body).await;
    assert!(r.status < 300, "{method} {path}: {}", r.body);
    r.body
}
async fn seed(db: &Sqlite) {
    for prefix in ["EX", "DST"] {
        call(
            db,
            &Actor::user(1),
            "POST",
            "/api/v1/projects",
            json!({"name":prefix,"key_prefix":prefix,"color":"#123456"}),
        )
        .await;
    }
}
async fn history(db: &Sqlite, id: i64) -> Vec<Value> {
    call(
        db,
        &Actor::user(1),
        "GET",
        &format!("/api/v1/tickets/{id}/history"),
        Value::Null,
    )
    .await["data"]
        .as_array()
        .unwrap()
        .clone()
}
fn count(db: &Sqlite) -> i64 {
    db.0.query_row("SELECT count(*) FROM ticket_history", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn api_fields_relationships_cascades_and_actors() {
    block_on(async {
        let db = Sqlite::new();
        seed(&db).await;
        let user = Actor::user(1);
        let token = Actor::api_token(7);
        call(&db,&user,"POST","/api/v1/tickets",json!({"project_id":1,"title":"Initial","body":"Before","assignee":"agent","priority":2})).await;
        let born = history(&db, 1).await;
        assert_eq!(born.len(), 1);
        assert_eq!(born[0]["field"], "created");
        assert_eq!(born[0]["actor_kind"], "user");
        assert_eq!(born[0]["actor_label"], "Owner");
        call(&db,&token,"PATCH","/api/v1/tickets/1",json!({"title":"After","body":"<script>bad</script>","status":"todo","priority":1,"assignee":null,"project_id":2})).await;
        let entries = history(&db, 1).await;
        for field in [
            "title",
            "description",
            "status",
            "priority",
            "assignee",
            "project",
        ] {
            let row = entries.iter().find(|r| r["field"] == field).unwrap();
            assert_eq!(row["actor_kind"], "api_token");
            assert_eq!(row["actor_id"], "7");
            assert_eq!(row["actor_label"], "API token #7");
        }
        assert_eq!(
            entries.iter().find(|r| r["field"] == "assignee").unwrap()["old_value"],
            "agent"
        );
        assert_eq!(
            entries.iter().find(|r| r["field"] == "assignee").unwrap()["new_value"],
            Value::Null
        );
        let n = count(&db);
        call(
            &db,
            &token,
            "PATCH",
            "/api/v1/tickets/1",
            json!({"title":"After","priority":1,"assignee":null,"project_id":2,"sort_order":123}),
        )
        .await;
        assert_eq!(count(&db), n);
        call(
            &db,
            &token,
            "POST",
            "/api/v1/labels",
            json!({"name":"Feature","color":"#abcdef"}),
        )
        .await;
        call(
            &db,
            &token,
            "POST",
            "/api/v1/tickets/1/labels/2",
            Value::Null,
        )
        .await;
        call(
            &db,
            &token,
            "DELETE",
            "/api/v1/tickets/1/labels/2",
            Value::Null,
        )
        .await;
        call(
            &db,
            &token,
            "POST",
            "/api/v1/tickets/1/labels/2",
            Value::Null,
        )
        .await;
        call(&db, &user, "DELETE", "/api/v1/labels/2", Value::Null).await;
        let entries = history(&db, 1).await;
        assert_eq!(
            entries
                .iter()
                .filter(|r| r["field"] == "label_added")
                .count(),
            2
        );
        assert_eq!(
            entries
                .iter()
                .filter(|r| r["field"] == "label_removed")
                .count(),
            2
        );
        assert_eq!(entries[0]["old_value"]["name"], "Feature");
        call(
            &db,
            &user,
            "POST",
            "/api/v1/tickets",
            json!({"project_id":1,"title":"Blocked"}),
        )
        .await;
        call(
            &db,
            &user,
            "POST",
            "/api/v1/tickets/1/dependencies/2",
            Value::Null,
        )
        .await;
        assert_eq!(history(&db, 2).await[0]["field"], "blocker_added");
        call(
            &db,
            &user,
            "DELETE",
            "/api/v1/tickets/1/dependencies/2",
            Value::Null,
        )
        .await;
        assert_eq!(history(&db, 2).await[0]["field"], "blocker_removed");
        call(
            &db,
            &user,
            "POST",
            "/api/v1/tickets/1/dependencies/2",
            Value::Null,
        )
        .await;
        call(&db, &user, "DELETE", "/api/v1/tickets/1", Value::Null).await;
        assert_eq!(history(&db, 2).await[0]["field"], "blocker_removed");
        assert_eq!(
            db.0.query_row(
                "SELECT count(*) FROM ticket_history WHERE ticket_id=1 AND field='deleted'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        assert_eq!(
            db.0.query_row("SELECT count(*) FROM ticket_mutation_context", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            db.0.query_row("SELECT count(*) FROM ticket_mutation_before", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    });
}

#[test]
fn failure_rolls_back_domain_history_and_analytics() {
    block_on(async {
        let db = Sqlite::new();
        seed(&db).await;
        let actor = Actor::user(1);
        call(
            &db,
            &actor,
            "POST",
            "/api/v1/tickets",
            json!({"project_id":1,"title":"Before"}),
        )
        .await;
        db.0.execute_batch("CREATE TRIGGER fail_history BEFORE INSERT ON ticket_history WHEN NEW.field='status' BEGIN SELECT RAISE(ABORT,'fixture history failure'); END;").unwrap();
        let r = handle_as(
            &db,
            &actor,
            "PATCH",
            "/api/v1/tickets/1",
            &[],
            json!({"status":"done","title":"After"}),
        )
        .await;
        assert_eq!(r.status, 500);
        assert_eq!(count(&db), 1);
        let saved = call(&db, &actor, "GET", "/api/v1/tickets/1", Value::Null).await;
        assert_eq!(saved["title"], "Before");
        assert_eq!(saved["status"], "backlog");
        assert_eq!(
            db.0.query_row(
                "SELECT count(*) FROM ticket_events WHERE kind='status'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        db.0.execute_batch("DROP TRIGGER fail_history;").unwrap();
        call(
            &db,
            &actor,
            "POST",
            "/api/v1/tickets",
            json!({"project_id":1,"title":"Other"}),
        )
        .await;
        call(
            &db,
            &actor,
            "POST",
            "/api/v1/tickets/1/dependencies/2",
            Value::Null,
        )
        .await;
        let n = count(&db);
        for path in [
            "/api/v1/tickets/2/dependencies/1",
            "/api/v1/tickets/1/labels/999",
        ] {
            assert!(
                handle_as(&db, &actor, "POST", path, &[], Value::Null)
                    .await
                    .status
                    >= 400
            );
        }
        assert_eq!(count(&db), n);
        // SQL failure after the snapshot is captured, including a failed creation.
        assert!(
            mutation::execute::<_, Value>(
                &db,
                &actor,
                mutation::Scope::create(),
                "INSERT INTO tickets(project_id,number,title) VALUES(999,1,'bad')",
                vec![]
            )
            .await
            .is_err()
        );
        assert_eq!(count(&db), n);
        call(
            &db,
            &actor,
            "PATCH",
            "/api/v1/tickets/1",
            json!({"status":"done"}),
        )
        .await;
        assert_eq!(history(&db, 1).await[0]["old_value"], "backlog");
    });
}

#[test]
fn importer_replays_and_typed_repository_share_the_executor() {
    block_on(async {
        let db = Sqlite::new();
        seed(&db).await;
        let actor = Actor::api_token(3);
        let q = vec![("id".into(), "github:example/repo:1".into())];
        for title in ["Imported", "Imported", "Edited"] {
            assert_eq!(
                handle_as(
                    &db,
                    &actor,
                    "PUT",
                    "/api/v1/tickets/by-external",
                    &q,
                    json!({"project_id":1,"title":title})
                )
                .await
                .status,
                200
            );
        }
        let rows = history(&db, 1).await;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["field"], "title");
        assert_eq!(rows[1]["field"], "created");
        let r = Repository::new(&db);
        let mut t = r.create_ticket(1, "Typed", "body").await.unwrap();
        t.title = "Typed edit".into();
        r.save_ticket(&t).await.unwrap();
        let l = r.create_label("Typed label", "#abcdef").await.unwrap();
        r.add_label(t.id, l.id).await.unwrap();
        r.remove_label(t.id, l.id).await.unwrap();
        let edge = tickets_core::domain::Dependency {
            blocker_id: 1,
            blocked_id: t.id,
        };
        r.add_dependency(edge).await.unwrap();
        r.remove_dependency(edge).await.unwrap();
        let rows = history(&db, t.id).await;
        for field in [
            "created",
            "title",
            "label_added",
            "label_removed",
            "blocker_added",
            "blocker_removed",
        ] {
            assert!(rows.iter().any(|r| r["field"] == field), "{field}");
        }
        assert!(rows.iter().all(|r| r["actor_kind"] == "system"));
        r.delete_ticket(t.id).await.unwrap();
    });
}

#[test]
fn cursor_validation_retention_and_reserved_claim_kinds() {
    block_on(async {
        let db = Sqlite::new();
        seed(&db).await;
        let actor = Actor::user(1);
        call(
            &db,
            &actor,
            "POST",
            "/api/v1/tickets",
            json!({"project_id":1,"title":"One"}),
        )
        .await;
        for title in ["Two", "Three", "Four"] {
            call(
                &db,
                &actor,
                "PATCH",
                "/api/v1/tickets/1",
                json!({"title":title}),
            )
            .await;
        }
        let first = handle_as(
            &db,
            &actor,
            "GET",
            "/api/v1/tickets/EX-1/history",
            &[("limit".into(), "2".into())],
            Value::Null,
        )
        .await;
        assert_eq!(first.status, 200);
        let cursor = first.body["next_cursor"].as_str().unwrap();
        call(
            &db,
            &actor,
            "PATCH",
            "/api/v1/tickets/1",
            json!({"title":"Concurrent"}),
        )
        .await;
        let second = handle_as(
            &db,
            &actor,
            "GET",
            "/api/v1/tickets/1/history",
            &[
                ("limit".into(), "2".into()),
                ("cursor".into(), cursor.into()),
            ],
            Value::Null,
        )
        .await;
        assert_eq!(second.body["data"].as_array().unwrap().len(), 2);
        assert!(second.body["next_cursor"].is_null());
        assert!(second.body["data"][0]["id"].as_i64().unwrap() < cursor.parse().unwrap());
        for q in [
            vec![("limit".into(), "101".into())],
            vec![("cursor".into(), "0".into())],
            vec![("status".into(), "todo".into())],
            vec![("limit".into(), "2".into()), ("limit".into(), "2".into())],
        ] {
            assert_eq!(
                handle_as(
                    &db,
                    &actor,
                    "GET",
                    "/api/v1/tickets/1/history",
                    &q,
                    Value::Null
                )
                .await
                .status,
                400
            );
        }
        for field in history::CLAIM_FIELDS {
            db.0.execute("INSERT INTO ticket_history(subject_id,ticket_id,actor_kind,actor_id,actor_label,field,old_value,new_value) VALUES(1,1,'system','claims','System',?,NULL,'{}')",[field]).unwrap();
        }
        for sql in [
            "UPDATE ticket_history SET actor_label='changed'",
            "DELETE FROM ticket_history",
        ] {
            assert!(db.0.execute(sql, []).is_err());
        }
        call(&db, &actor, "DELETE", "/api/v1/tickets/1", Value::Null).await;
        assert_eq!(
            handle_as(
                &db,
                &actor,
                "GET",
                "/api/v1/tickets/1/history",
                &[],
                Value::Null
            )
            .await
            .status,
            404
        );
        call(
            &db,
            &actor,
            "POST",
            "/api/v1/tickets",
            json!({"project_id":1,"title":"Reused ID"}),
        )
        .await;
        let fresh = history(&db, 1).await;
        assert_eq!(fresh.len(), 1);
        assert_ne!(fresh[0]["subject_id"], 1);
    });
}

#[test]
fn sql_diff_matches_core_reference_for_all_fields() {
    block_on(async {
        let db = Sqlite::new();
        let old = json!({"status":"todo","priority":2,"assignee":"agent","title":"a","body":"before","project":{"id":1,"name":"One"},"labels":[{"id":1,"name":"a"},{"id":3,"name":"keep"}],"blockers":[{"id":1}],"blocks":[{"id":2}]});
        let new = json!({"status":"done","priority":0,"assignee":null,"title":"b","body":"after","project":{"id":2,"name":"Two"},"labels":[{"id":3,"name":"renamed"},{"id":2,"name":"b"}],"blockers":[{"id":2}],"blocks":[]});
        for (a, b) in [
            (Some(&old), Some(&new)),
            (Some(&old), Some(&old)),
            (None, Some(&new)),
            (Some(&old), None),
        ] {
            let sql = format!(
                "WITH pairs AS (SELECT 1 subject_id,1 ticket_id,? old,? new) SELECT field,old_value,new_value FROM ({})",
                history::diff_sql()
            );
            let mut actual: Vec<Value> = db
                .query(
                    &sql,
                    vec![
                        a.map(Value::to_string).into(),
                        b.map(Value::to_string).into(),
                    ],
                )
                .await
                .unwrap();
            for row in &mut actual {
                for field in ["old_value", "new_value"] {
                    if let Some(s) = row[field].as_str() {
                        row[field] = serde_json::from_str(s).unwrap();
                    }
                }
            }
            let mut expected = history::diff(a, b);
            actual.sort_by_key(Value::to_string);
            expected.sort_by_key(Value::to_string);
            assert_eq!(actual, expected);
        }
    });
}

#[test]
fn additive_migration_baseline_and_old_worker_compatibility() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
    let mut paths = std::fs::read_dir(dir)
        .unwrap()
        .map(|p| p.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "sql"))
        .collect::<Vec<_>>();
    paths.sort();
    for path in paths.iter().take_while(|p| {
        !p.file_name()
            .unwrap()
            .to_string_lossy()
            .contains("ticket_history")
    }) {
        conn.execute_batch(&std::fs::read_to_string(path).unwrap())
            .unwrap();
    }
    conn.execute_batch("INSERT INTO projects(name,key_prefix,color) VALUES('Fixture','EX','#123456'); INSERT INTO tickets(project_id,number,title) VALUES(1,1,'Existing'),(1,2,'Second');").unwrap();
    let analytics: i64 = conn
        .query_row("SELECT count(*) FROM ticket_events", [], |r| r.get(0))
        .unwrap();
    let path = paths
        .iter()
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .contains("ticket_history")
        })
        .unwrap();
    conn.execute_batch(&std::fs::read_to_string(path).unwrap())
        .unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM ticket_history WHERE field='created' AND actor_id='migration'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        2
    );
    assert_eq!(
        conn.query_row("SELECT count(*) FROM ticket_events", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        analytics
    );
    conn.execute_batch("INSERT INTO tickets(project_id,number,title) VALUES(1,3,'Old Worker'); UPDATE tickets SET status='done' WHERE id=3; DELETE FROM tickets WHERE id=3;").unwrap();
    assert_eq!(
        conn.query_row("SELECT count(*) FROM ticket_events", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        analytics + 3
    );
}

#[test]
fn old_values_are_read_inside_the_batch_after_a_concurrent_edit() {
    struct Concurrent {
        db: Sqlite,
        intervene: std::cell::Cell<bool>,
    }
    impl Store for Concurrent {
        async fn query<T: serde::de::DeserializeOwned>(
            &self,
            sql: &str,
            params: Vec<Value>,
        ) -> tickets_worker::repository::Result<Vec<T>> {
            self.db.query(sql, params).await
        }
        async fn batch(
            &self,
            statements: Vec<tickets_worker::repository::Statement>,
        ) -> tickets_worker::repository::Result<Vec<Vec<Value>>> {
            if self.intervene.replace(false) {
                // Another committed mutation after HTTP resource lookup, before
                // this request obtains its atomic batch.
                call(
                    &self.db,
                    &Actor::api_token(2),
                    "PATCH",
                    "/api/v1/tickets/1",
                    json!({"priority":3}),
                )
                .await;
            }
            self.db.batch(statements).await
        }
    }
    block_on(async {
        let db = Sqlite::new();
        seed(&db).await;
        call(
            &db,
            &Actor::user(1),
            "POST",
            "/api/v1/tickets",
            json!({"project_id":1,"title":"Concurrent","priority":2}),
        )
        .await;
        let store = Concurrent {
            db,
            intervene: std::cell::Cell::new(true),
        };
        let reply = handle_as(
            &store,
            &Actor::user(1),
            "PATCH",
            "/api/v1/tickets/1",
            &[],
            json!({"priority":1}),
        )
        .await;
        assert_eq!(reply.status, 200);
        let rows = history(&store.db, 1).await;
        assert_eq!(rows[0]["old_value"], 3);
        assert_eq!(rows[0]["new_value"], 1);
        assert_eq!(rows[1]["old_value"], 2);
        assert_eq!(rows[1]["new_value"], 3);
    });
}
