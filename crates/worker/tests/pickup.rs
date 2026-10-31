#![cfg(not(target_arch = "wasm32"))]
mod common;
use common::Sqlite;
use futures::executor::block_on;
use serde_json::{Value, json};
use tickets_core::{history::Actor, pickup::Rules};
use tickets_worker::{
    api::{Reply, handle_as},
    pickup::sweep,
};

async fn call(db: &Sqlite, method: &str, path: &str, body: Value) -> Reply {
    handle_as(db, &Actor::api_token(1), method, path, &[], body).await
}
async fn ok(db: &Sqlite, method: &str, path: &str, body: Value) -> Value {
    let r = call(db, method, path, body).await;
    assert!(r.status < 300, "{method} {path}: {} {}", r.status, r.body);
    r.body
}
async fn seed(db: &Sqlite) -> Value {
    ok(
        db,
        "POST",
        "/api/v1/projects",
        json!({"name":"Example","key_prefix":"EX","color":"#123456"}),
    )
    .await;
    ticket(db, "One", 2).await
}
async fn ticket(db: &Sqlite, title: &str, priority: i64) -> Value {
    ok(
        db,
        "POST",
        "/api/v1/tickets",
        json!({"project_id":1,"title":title,"status":"todo","priority":priority}),
    )
    .await
}
async fn fields(db: &Sqlite) -> Vec<String> {
    ok(db, "GET", "/api/v1/tickets/1/history", Value::Null).await["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["field"].as_str().unwrap().into())
        .collect()
}
#[test]
fn lifecycle_fencing_history_and_manual_status() {
    block_on(async {
        let db = Sqlite::new();
        seed(&db).await;
        let preview = ok(&db, "GET", "/api/v1/next", Value::Null).await;
        assert_eq!(preview["ticket"]["id"], 1);
        assert!(
            preview["reason"]
                .as_str()
                .unwrap()
                .starts_with("High priority")
        );
        assert_eq!(fields(&db).await, vec!["created"]);
        let claim = ok(&db, "POST", "/api/v1/next/claim", json!({})).await;
        assert_eq!(claim["ticket"]["status"], "in_progress");
        assert_eq!(claim["ticket"]["claimed_by"], "api_token:1");
        let lease = json!({"claim_id":claim["ticket"]["claim_id"]});
        assert_eq!(
            call(&db, "POST", "/api/v1/next/claim", json!({}))
                .await
                .status,
            204
        );
        let other = handle_as(
            &db,
            &Actor::api_token(2),
            "POST",
            "/api/v1/tickets/1/claim/heartbeat",
            &[],
            lease.clone(),
        )
        .await;
        assert_eq!(other.status, 409);
        for _ in 0..2 {
            let renewed = call(
                &db,
                "POST",
                "/api/v1/tickets/EX-1/claim/heartbeat",
                lease.clone(),
            )
            .await;
            assert_eq!(renewed.status, 200);
            assert!(
                renewed.change.is_none(),
                "heartbeats must not notify live rooms"
            );
        }
        assert_eq!(
            fields(&db)
                .await
                .iter()
                .filter(|f| *f == "heartbeat")
                .count(),
            2
        );
        ok(
            &db,
            "POST",
            "/api/v1/tickets/1/claim/release",
            lease.clone(),
        )
        .await;
        assert_eq!(
            ok(&db, "GET", "/api/v1/tickets/1", Value::Null).await["status"],
            "todo"
        );
        let next = ok(&db, "POST", "/api/v1/next/claim", json!({})).await;
        assert_ne!(next["ticket"]["claim_id"], lease["claim_id"]);
        assert_eq!(
            call(&db, "POST", "/api/v1/tickets/1/claim/release", lease)
                .await
                .status,
            409
        );
        ok(
            &db,
            "PATCH",
            "/api/v1/tickets/1",
            json!({"status":"in_review"}),
        )
        .await;
        let released = ok(
            &db,
            "POST",
            "/api/v1/tickets/1/claim/release",
            json!({"claim_id":next["ticket"]["claim_id"]}),
        )
        .await;
        assert_eq!(released["status"], "in_review");
        assert!(released["claimed_by"].is_null());
        let f = fields(&db).await;
        assert_eq!(f.iter().filter(|f| *f == "claim").count(), 2);
        assert_eq!(f.iter().filter(|f| *f == "release").count(), 2);
    });
}
#[test]
fn expiry_and_rollback() {
    block_on(async {
        let db = Sqlite::new();
        seed(&db).await;
        let claim = ok(&db, "POST", "/api/v1/next/claim", Value::Null).await;
        // Test clock fixture only; production lease writes always use mutation::execute.
        db.0.execute(
            "UPDATE tickets SET claim_expires_at='2000-01-01T00:00:00.000Z'",
            [],
        )
        .unwrap();
        assert_eq!(
            call(
                &db,
                "POST",
                "/api/v1/tickets/1/claim/heartbeat",
                json!({"claim_id":claim["ticket"]["claim_id"]})
            )
            .await
            .status,
            409
        );
        assert_eq!(sweep(&db).await.unwrap().len(), 1);
        assert!(sweep(&db).await.unwrap().is_empty());
        assert_eq!(
            ok(&db, "GET", "/api/v1/tickets/1", Value::Null).await["status"],
            "todo"
        );
        assert!(fields(&db).await.contains(&"expiry".into()));
        db.0.execute_batch("CREATE TRIGGER reject_claim_history BEFORE INSERT ON ticket_history WHEN NEW.field='claim' BEGIN SELECT RAISE(ABORT,'fixture history failure'); END;").unwrap();
        assert_eq!(
            call(&db, "POST", "/api/v1/next/claim", Value::Null)
                .await
                .status,
            500
        );
        assert_eq!(
            ok(&db, "GET", "/api/v1/tickets/1", Value::Null).await["status"],
            "todo"
        );
        let scratch: i64 =
            db.0.query_row("SELECT count(*) FROM ticket_mutation_context", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(scratch, 0);
    });
}
#[test]
fn rules_ranking_filters_and_cap() {
    block_on(async {
        let db = Sqlite::new();
        seed(&db).await;
        ticket(&db, "Urgent", 1).await;
        ticket(&db, "Low blocker", 4).await;
        ok(&db, "POST", "/api/v1/tickets/3/dependencies/2", Value::Null).await;
        let preview = ok(&db, "GET", "/api/v1/next", Value::Null).await;
        assert_eq!(preview["ticket"]["id"], 3);
        assert!(
            preview["reason"]
                .as_str()
                .unwrap()
                .contains("unblocks EX-2")
        );
        // Seeded skip label, open prerequisite, open PR and Backlog each exclude a ticket.
        ok(&db, "POST", "/api/v1/tickets/3/labels/1", Value::Null).await;
        assert_eq!(
            ok(&db, "GET", "/api/v1/next", Value::Null).await["ticket"]["id"],
            1
        );
        db.0.execute("INSERT INTO ticket_pull_requests(ticket_id,repo,number,url,title,state,closes,head_sha,updated_at) VALUES(1,'example/repo',1,'https://github.com/example/repo/pull/1','PR','open',1,'abc','2026-01-01')",[]).unwrap();
        ok(&db, "PATCH", "/api/v1/tickets/1", json!({"status":"todo"})).await;
        assert_eq!(
            call(&db, "GET", "/api/v1/next", Value::Null).await.status,
            204
        );
        let mut rules = Rules {
            skip_open_pr: false,
            concurrency_cap: 1,
            ..Rules::default()
        };
        ok(&db, "PUT", "/api/v1/pickup-rules", json!(rules)).await;
        ok(&db, "POST", "/api/v1/next/claim", Value::Null).await;
        rules.skip_blocked = false;
        rules.excluded_labels.clear();
        ok(&db, "PUT", "/api/v1/pickup-rules", json!(rules)).await;
        assert_eq!(
            call(&db, "GET", "/api/v1/next", Value::Null).await.status,
            204
        );
        let query = vec![("project".into(), "1".into())];
        let override_rules = Rules {
            concurrency_cap: 2,
            skip_blocked: false,
            excluded_labels: vec![],
            ..Rules::default()
        };
        let saved = handle_as(
            &db,
            &Actor::user(1),
            "PUT",
            "/api/v1/pickup-rules",
            &query,
            json!(override_rules),
        )
        .await;
        assert_eq!(saved.status, 200);
        assert_eq!(saved.body["source"], "project");
        assert_eq!(
            ok(&db, "GET", "/api/v1/next", Value::Null).await["ticket"]["id"],
            3
        );
        let deleted = handle_as(
            &db,
            &Actor::user(1),
            "DELETE",
            "/api/v1/pickup-rules",
            &query,
            Value::Null,
        )
        .await;
        assert_eq!(deleted.body["source"], "workspace");
        assert_eq!(
            call(&db, "GET", "/api/v1/next", Value::Null).await.status,
            204
        );
        assert_eq!(
            call(&db, "DELETE", "/api/v1/pickup-rules", Value::Null)
                .await
                .status,
            422
        );
        assert_eq!(
            call(
                &db,
                "PUT",
                "/api/v1/pickup-rules",
                json!({"concurrency_cap":0})
            )
            .await
            .status,
            422
        );
    });
}

#[test]
fn two_connections_never_claim_same_ticket_or_exceed_cap() {
    for cap in [1, 3] {
        let path = std::env::temp_dir().join(format!("tik61-{}-{cap}.sqlite", std::process::id()));
        let db = Sqlite::new();
        block_on(async {
            seed(&db).await;
            ticket(&db, "Two", 2).await;
            ok(
                &db,
                "PUT",
                "/api/v1/pickup-rules",
                json!(Rules {
                    concurrency_cap: cap,
                    ..Rules::default()
                }),
            )
            .await;
        });
        db.0.execute("VACUUM INTO ?", [path.to_str().unwrap()])
            .unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let threads: Vec<_> = (0..2)
            .map(|_| {
                let path = path.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let connection = rusqlite::Connection::open(path).unwrap();
                    connection
                        .busy_timeout(std::time::Duration::from_secs(5))
                        .unwrap();
                    connection.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
                    let db = Sqlite(connection);
                    barrier.wait();
                    block_on(call(&db, "POST", "/api/v1/next/claim", Value::Null))
                })
            })
            .collect();
        let replies: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
        let ids: Vec<_> = replies
            .iter()
            .filter(|r| r.status == 200)
            .map(|r| r.body["ticket"]["id"].as_i64().unwrap())
            .collect();
        assert_eq!(
            ids.len(),
            if cap == 1 { 1 } else { 2 },
            "{:?}",
            replies
                .iter()
                .map(|r| (&r.status, &r.body))
                .collect::<Vec<_>>()
        );
        if ids.len() == 2 {
            assert_ne!(ids[0], ids[1]);
        }
        assert!(replies.iter().all(|r| [200, 204].contains(&r.status)));
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn sql_matches_core_priority_age_and_id_order() {
    block_on(async {
        use tickets_core::pickup::{Candidate, compare};
        let db = Sqlite::new();
        seed(&db).await;
        let mut candidates = vec![];
        for (index, priority) in [0, 4, 3, 2, 1, 1].into_iter().enumerate() {
            let id = if index == 0 {
                1
            } else {
                ticket(&db, "Rank", priority).await["id"].as_i64().unwrap()
            };
            let created = if index == 5 {
                "2025-01-01T00:00:00.000Z"
            } else {
                "2026-01-01T00:00:00.000Z"
            };
            db.0.execute(
                "UPDATE tickets SET priority=?,created_at=? WHERE id=?",
                rusqlite::params![priority, created, id],
            )
            .unwrap();
            candidates.push(Candidate {
                id,
                status: "todo".into(),
                archived: false,
                has_claim: false,
                blocked: false,
                open_pr: false,
                unblocks: vec![],
                priority: priority as u8,
                created_at: created.into(),
                labels: vec![],
                project_in_flight: 0,
            });
        }
        let rules = Rules {
            concurrency_cap: 10,
            ..Rules::default()
        };
        ok(&db, "PUT", "/api/v1/pickup-rules", json!(rules)).await;
        candidates.sort_by(|a, b| compare(a, &rules, b, &rules));
        for expected in candidates {
            let claim = ok(&db, "POST", "/api/v1/next/claim", Value::Null).await;
            assert_eq!(claim["ticket"]["id"], expected.id);
        }
        assert_eq!(
            call(&db, "GET", "/api/v1/next", Value::Null).await.status,
            204
        );
    });
}

#[test]
fn status_pr_blocker_project_and_capacity_boundaries() {
    block_on(async {
        let db = Sqlite::new();
        seed(&db).await;
        for status in ["backlog", "in_progress", "in_review", "done", "canceled"] {
            ok(&db, "PATCH", "/api/v1/tickets/1", json!({"status":status})).await;
            assert_eq!(
                call(&db, "GET", "/api/v1/next", Value::Null).await.status,
                204
            );
        }
        ok(&db, "PATCH", "/api/v1/tickets/1", json!({"status":"todo"})).await;
        ok(&db, "PATCH", "/api/v1/projects/1", json!({"archived":true})).await;
        assert_eq!(
            call(&db, "GET", "/api/v1/next", Value::Null).await.status,
            204
        );
        ok(
            &db,
            "PATCH",
            "/api/v1/projects/1",
            json!({"archived":false}),
        )
        .await;
        for state in ["draft", "open", "closed", "merged"] {
            db.0.execute("INSERT INTO ticket_pull_requests(ticket_id,repo,number,url,title,state,closes,head_sha,updated_at) VALUES(1,'example/repo',1,'https://github.com/example/repo/pull/1','PR',?,0,'abc','2026-01-01') ON CONFLICT(ticket_id,repo,number) DO UPDATE SET state=excluded.state",[state]).unwrap();
            ok(&db, "PATCH", "/api/v1/tickets/1", json!({"status":"todo"})).await;
            assert_eq!(
                call(&db, "GET", "/api/v1/next", Value::Null).await.status,
                if ["open", "draft"].contains(&state) {
                    204
                } else {
                    200
                }
            );
        }
        ticket(&db, "Blocker", 0).await;
        ok(&db, "POST", "/api/v1/tickets/2/dependencies/1", Value::Null).await;
        for status in ["todo", "in_progress", "in_review", "done", "canceled"] {
            ok(&db, "PATCH", "/api/v1/tickets/2", json!({"status":status})).await;
            let result = call(&db, "GET", "/api/v1/next", Value::Null).await;
            if ["done", "canceled"].contains(&status) {
                assert_eq!(result.body["ticket"]["id"], 1);
            } else {
                assert_ne!(result.body["ticket"]["id"], 1);
            }
        }
        ok(
            &db,
            "PATCH",
            "/api/v1/tickets/2",
            json!({"status":"in_progress"}),
        )
        .await;
        ok(
            &db,
            "PUT",
            "/api/v1/pickup-rules",
            json!(Rules {
                skip_blocked: false,
                concurrency_cap: 1,
                ..Rules::default()
            }),
        )
        .await;
        assert_eq!(
            call(&db, "GET", "/api/v1/next", Value::Null).await.status,
            204
        ); // Manual work counts.
        ok(&db, "PATCH", "/api/v1/tickets/2", json!({"status":"todo"})).await;
        let claim = ok(&db, "POST", "/api/v1/next/claim", Value::Null).await;
        let key = claim["ticket"]["id"].as_i64().unwrap();
        ok(
            &db,
            "PATCH",
            &format!("/api/v1/tickets/{key}"),
            json!({"status":"in_review"}),
        )
        .await;
        assert_eq!(
            call(&db, "GET", "/api/v1/next", Value::Null).await.status,
            204
        ); // Active claim in Review counts.
        db.0.execute("UPDATE tickets SET claim_expires_at='2000-01-01T00:00:00.000Z' WHERE claimed_by IS NOT NULL",[]).unwrap();
        sweep(&db).await.unwrap();
        assert_eq!(
            ok(&db, "GET", &format!("/api/v1/tickets/{key}"), Value::Null).await["status"],
            "in_review"
        );
        ok(
            &db,
            "POST",
            "/api/v1/projects",
            json!({"name":"Other","key_prefix":"OTHER","color":"#123456"}),
        )
        .await;
        ok(
            &db,
            "POST",
            "/api/v1/tickets",
            json!({"project_id":2,"title":"Urgent other","status":"todo","priority":1}),
        )
        .await;
        let filtered = handle_as(
            &db,
            &Actor::api_token(1),
            "POST",
            "/api/v1/next/claim",
            &[],
            json!({"project":2}),
        )
        .await;
        assert_eq!(filtered.status, 200);
        assert_eq!(filtered.body["ticket"]["project_id"], 2);
        for query in [
            vec![("unknown".into(), "1".into())],
            vec![("project".into(), "0".into())],
            vec![
                ("project".into(), "1".into()),
                ("project".into(), "1".into()),
            ],
        ] {
            assert_eq!(
                handle_as(
                    &db,
                    &Actor::api_token(1),
                    "GET",
                    "/api/v1/next",
                    &query,
                    Value::Null
                )
                .await
                .status,
                400
            );
        }
    });
}

struct Counting {
    db: Sqlite,
    calls: std::cell::Cell<usize>,
}
impl tickets_worker::repository::Store for Counting {
    async fn query<T: serde::de::DeserializeOwned>(
        &self,
        sql: &str,
        params: Vec<Value>,
    ) -> tickets_worker::repository::Result<Vec<T>> {
        self.calls.set(self.calls.get() + 1);
        self.db.query(sql, params).await
    }
    async fn batch(
        &self,
        statements: Vec<tickets_worker::repository::Statement>,
    ) -> tickets_worker::repository::Result<Vec<Vec<Value>>> {
        self.calls.set(self.calls.get() + 1);
        assert!(statements.len() <= 6);
        assert!(statements.iter().all(|s| s.params.len() <= 100));
        self.db.batch(statements).await
    }
}
#[test]
fn bounded_calls_and_empty_claim_history() {
    block_on(async {
        let store = Counting {
            db: Sqlite::new(),
            calls: std::cell::Cell::new(0),
        };
        seed(&store.db).await;
        for method in ["GET", "POST"] {
            store.calls.set(0);
            let path = if method == "GET" {
                "/api/v1/next"
            } else {
                "/api/v1/next/claim"
            };
            let reply =
                handle_as(&store, &Actor::api_token(1), method, path, &[], Value::Null).await;
            assert_eq!(reply.status, 200);
            assert_eq!(store.calls.get(), 1); // + one auth call.
            if method == "POST" {
                for action in ["heartbeat", "release"] {
                    store.calls.set(0);
                    let reply = handle_as(
                        &store,
                        &Actor::api_token(1),
                        "POST",
                        &format!("/api/v1/tickets/1/claim/{action}"),
                        &[],
                        json!({"claim_id":reply.body["ticket"]["claim_id"]}),
                    )
                    .await;
                    assert_eq!(reply.status, 200);
                    assert_eq!(store.calls.get(), 2);
                }
            }
        }
        ok(
            &store.db,
            "PATCH",
            "/api/v1/tickets/1",
            json!({"status":"done"}),
        )
        .await;
        let before = fields(&store.db).await;
        assert_eq!(
            call(&store.db, "POST", "/api/v1/next/claim", Value::Null)
                .await
                .status,
            204
        );
        assert_eq!(fields(&store.db).await, before);
    });
}

#[test]
fn additive_migration_preserves_existing_label_and_tickets() {
    let db = rusqlite::Connection::open_in_memory().unwrap();
    db.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|v| v.unwrap().path())
        .filter(|v| v.extension().is_some_and(|e| e == "sql"))
        .collect();
    files.sort();
    let index = files
        .iter()
        .position(|file| {
            file.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with("_pickup_rules.sql")
        })
        .unwrap();
    let migration = &files[index];
    for file in &files[..index] {
        db.execute_batch(&std::fs::read_to_string(file).unwrap())
            .unwrap();
    }
    db.execute_batch("INSERT INTO projects(name,key_prefix,color) VALUES('Old','OLD','#123456'); INSERT INTO tickets(project_id,number,title,status) VALUES(1,1,'Existing work','in_progress'); INSERT INTO labels(name,color) VALUES('waiting-on-calum','#123456');").unwrap();
    db.execute_batch(&std::fs::read_to_string(migration).unwrap())
        .unwrap();
    let label: (i64, String) = db
        .query_row(
            "SELECT id,color FROM labels WHERE name='waiting-on-calum'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(label, (1, "#123456".into()));
    let ticket: (String, Option<String>) = db
        .query_row(
            "SELECT status,claimed_by FROM tickets WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(ticket, ("in_progress".into(), None));
    let rules: String = db
        .query_row("SELECT rules FROM pickup_rules WHERE scope_id=0", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        Rules::parse(serde_json::from_str(&rules).unwrap()).unwrap(),
        Rules::default()
    );
}
