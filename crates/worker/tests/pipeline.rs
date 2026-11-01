#![cfg(not(target_arch = "wasm32"))]
#[path = "../../web/src/colors.rs"]
mod colors;
mod common;
use common::Sqlite;
use futures::executor::block_on;
use serde_json::{Value, json};
use tickets_core::history::Actor;
use tickets_worker::api::{Reply, handle_as};

async fn call(db: &Sqlite, method: &str, path: &str, body: Value) -> Reply {
    handle_as(db, &Actor::api_token(1), method, path, &[], body).await
}
async fn ok(db: &Sqlite, method: &str, path: &str, body: Value) -> Value {
    let r = call(db, method, path, body).await;
    assert!(r.status < 300, "{method} {path}: {} {}", r.status, r.body);
    r.body
}
async fn seed(db: &Sqlite) {
    // Use palette values rather than introducing styling literals in fixtures.
    ok(
        db,
        "POST",
        "/api/v1/projects",
        json!({"name":"Runs","key_prefix":"RUN","color":colors::value("--accent")}),
    )
    .await;
    for title in ["First", "Second"] {
        ok(
            db,
            "POST",
            "/api/v1/tickets",
            json!({"project_id":1,"title":title,"status":"todo"}),
        )
        .await;
    }
}
async fn events(db: &Sqlite) -> Value {
    ok(db, "GET", "/api/v1/runs/1/events", Value::Null).await["data"].clone()
}
#[test]
fn enqueue_versions_events_and_terminal_runs() {
    block_on(async {
        let db = Sqlite::new();
        seed(&db).await;
        assert_eq!(
            call(&db, "PUT", "/api/v1/runs/RUN-99", json!({}))
                .await
                .status,
            404
        );
        let created = call(
            &db,
            "PUT",
            "/api/v1/runs/RUN-1",
            json!({"repo":"tickets","owner":"box"}),
        )
        .await;
        assert_eq!(created.status, 201);
        assert_eq!(created.body["version"], 1);
        assert_eq!(created.body["intent"], "merge");
        let again = call(
            &db,
            "PUT",
            "/api/v1/runs/RUN-1",
            json!({"intent":"draft","owner":"other"}),
        )
        .await;
        assert_eq!(again.status, 200);
        assert_eq!(again.body, created.body);
        let initial = events(&db).await;
        assert_eq!(initial.as_array().unwrap().len(), 1);
        assert_eq!(initial[0]["detail"], json!({"action":"created"}));
        assert_eq!(initial[0]["actor"], "api_token:1");
        assert!(initial[0]["from_state"].is_null());
        db.0.execute(
            "UPDATE pipeline_runs SET entered_at='2000-01-01T00:00:00.000Z' WHERE id=1",
            [],
        )
        .unwrap();
        let updated=ok(&db,"PATCH","/api/v1/runs/1",json!({"expected_version":1,"state":"implementing","attempts":2,"claim_id":"abc","pr":42,"head_sha":"sha","last_error":"retry"})).await;
        assert_eq!(updated["version"], 2);
        assert_ne!(updated["entered_at"], "2000-01-01T00:00:00.000Z");
        let log = events(&db).await;
        assert_eq!(log[1]["from_state"], "queued");
        assert_eq!(log[1]["to_state"], "implementing");
        assert_eq!(
            log[1]["detail"]["state"],
            json!({"old":"queued","new":"implementing"})
        );
        assert_eq!(log[1]["detail"]["pr"], json!({"old":null,"new":42}));
        let conflict = call(
            &db,
            "PATCH",
            "/api/v1/runs/1",
            json!({"expected_version":1,"state":"done"}),
        )
        .await;
        assert_eq!(conflict.status, 409);
        assert_eq!(conflict.body["current"], updated);
        assert_eq!(events(&db).await, log);
        let cleared = ok(
            &db,
            "PATCH",
            "/api/v1/runs/1",
            json!({"expected_version":2,"pr":null,"last_error":null,"state":"implementing"}),
        )
        .await;
        assert!(cleared["pr"].is_null());
        assert_eq!(cleared["entered_at"], updated["entered_at"]);
        assert!(events(&db).await[2]["detail"].get("state").is_none());
        ok(
            &db,
            "PATCH",
            "/api/v1/runs/1",
            json!({"expected_version":3}),
        )
        .await;
        assert_eq!(events(&db).await[3]["detail"], json!({}));
        ok(
            &db,
            "PATCH",
            "/api/v1/runs/1",
            json!({"expected_version":4,"state":"done"}),
        )
        .await;
        let before = events(&db).await;
        for state in ["queued", "claimed"] {
            assert_eq!(
                call(
                    &db,
                    "PATCH",
                    "/api/v1/runs/1",
                    json!({"expected_version":5,"state":state})
                )
                .await
                .status,
                409
            );
        }
        assert_eq!(events(&db).await, before);
        let fresh = call(
            &db,
            "PUT",
            "/api/v1/runs/RUN-1",
            json!({"intent":"no_merge"}),
        )
        .await;
        assert_eq!(fresh.status, 201);
        assert_ne!(fresh.body["id"], created.body["id"]);
        assert_eq!(fresh.body["version"], 1);
        assert_eq!(fresh.body["state"], "queued");
        assert!(
            db.0.execute(
                "INSERT INTO pipeline_runs(ticket_id,key) VALUES(1,'RUN-1')",
                []
            )
            .is_err()
        );
        assert!(
            db.0.execute("UPDATE pipeline_run_events SET detail='{}'", [])
                .is_err()
        );
        assert!(db.0.execute("DELETE FROM pipeline_run_events", []).is_err());
        assert_eq!(
            call(&db, "GET", "/api/v1/runs/999", Value::Null)
                .await
                .status,
            404
        );
        assert_eq!(
            call(&db, "GET", "/api/v1/runs/999/events", Value::Null)
                .await
                .status,
            404
        );
    });
}
#[test]
fn input_validation_and_filters() {
    block_on(async {
        let db = Sqlite::new();
        seed(&db).await;
        for body in [
            json!({"intent":"invalid"}),
            json!({"intent":null}),
            json!({"unknown":1}),
            json!({"owner":1}),
            Value::Null,
        ] {
            assert_eq!(
                call(&db, "PUT", "/api/v1/runs/RUN-1", body).await.status,
                400
            );
        }
        ok(&db, "PUT", "/api/v1/runs/RUN-1", json!({"repo":"tickets"})).await;
        ok(
            &db,
            "PUT",
            "/api/v1/runs/RUN-2",
            json!({"repo":"other","intent":"draft"}),
        )
        .await;
        for body in [
            json!({"state":"checks"}),
            json!({"expected_version":0}),
            json!({"expected_version":1,"state":"bad"}),
            json!({"expected_version":1,"intent":"bad"}),
            json!({"expected_version":1,"attempts":-1}),
            json!({"expected_version":1,"pr":0}),
            json!({"expected_version":1,"next_check_at":"tomorrow"}),
            json!({"expected_version":1,"escalated_at":"2026-02-30T00:00:00.000Z"}),
            json!({"expected_version":1,"state":null}),
            json!({"expected_version":1,"owner":false}),
            json!({"expected_version":1,"id":3}),
        ] {
            assert_eq!(
                call(&db, "PATCH", "/api/v1/runs/1", body.clone())
                    .await
                    .status,
                400,
                "{body}"
            );
        }
        let filter = |q: Vec<(&str, &str)>| {
            q.into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect::<Vec<_>>()
        };
        for q in [
            vec![("limit", "101")],
            vec![("due", "false")],
            vec![("state", "bogus")],
            vec![("active", "yes")],
            vec![("repo", "a"), ("repo", "b")],
            vec![("x", "y")],
        ] {
            assert_eq!(
                handle_as(
                    &db,
                    &Actor::api_token(1),
                    "GET",
                    "/api/v1/runs",
                    &filter(q),
                    Value::Null
                )
                .await
                .status,
                400
            );
        }
        ok(&db,"PATCH","/api/v1/runs/1",json!({"expected_version":1,"state":"waiting_on_calum","next_check_at":"2999-01-01T00:00:00.000Z"})).await;
        for (q, expected) in [
            (vec![("due", "true")], 1),
            (vec![("active", "true")], 2),
            (vec![("repo", "tickets"), ("key", "RUN-1")], 1),
            (
                vec![("state", "waiting_on_calum,checks"), ("state", "queued")],
                2,
            ),
            (vec![("limit", "1")], 1),
            (vec![("active", "false")], 0),
        ] {
            let r = handle_as(
                &db,
                &Actor::api_token(1),
                "GET",
                "/api/v1/runs",
                &filter(q),
                Value::Null,
            )
            .await;
            assert_eq!(r.status, 200, "{}", r.body);
            assert_eq!(r.body["data"].as_array().unwrap().len(), expected);
        }
        ok(&db,"PATCH","/api/v1/runs/1",json!({"expected_version":2,"state":"stuck","next_check_at":"2000-01-01T00:00:00.000Z"})).await;
        let due = handle_as(
            &db,
            &Actor::api_token(1),
            "GET",
            "/api/v1/runs",
            &filter(vec![("due", "true")]),
            Value::Null,
        )
        .await;
        assert_eq!(due.body["data"].as_array().unwrap().len(), 2);
        ok(
            &db,
            "PATCH",
            "/api/v1/runs/1",
            json!({"expected_version":3,"state":"awaiting_review"}),
        )
        .await;
        let due = handle_as(
            &db,
            &Actor::api_token(1),
            "GET",
            "/api/v1/runs",
            &filter(vec![("due", "true")]),
            Value::Null,
        )
        .await;
        assert_eq!(due.body["data"].as_array().unwrap().len(), 1);
    });
}
#[test]
fn direct_claim_shares_capacity_leases_and_history() {
    block_on(async {
        let db = Sqlite::new();
        seed(&db).await;
        assert_eq!(
            call(&db, "POST", "/api/v1/tickets/RUN-99/claim", json!({}))
                .await
                .status,
            404
        );
        let claim = ok(&db, "POST", "/api/v1/tickets/RUN-2/claim", json!({})).await;
        assert_eq!(claim["ticket"]["id"], 2);
        assert_eq!(claim["ticket"]["claimed_by"], "api_token:1");
        let other = handle_as(
            &db,
            &Actor::api_token(2),
            "POST",
            "/api/v1/tickets/RUN-2/claim",
            &[],
            json!({}),
        )
        .await;
        assert_eq!(other.status, 409);
        assert_eq!(other.body["error"]["code"], "claim_conflict");
        let lease = json!({"claim_id":claim["ticket"]["claim_id"]});
        ok(
            &db,
            "POST",
            "/api/v1/tickets/RUN-2/claim/heartbeat",
            lease.clone(),
        )
        .await;
        ok(
            &db,
            "PUT",
            "/api/v1/pickup-rules",
            json!(tickets_core::pickup::Rules {
                concurrency_cap: 1,
                ..Default::default()
            }),
        )
        .await;
        assert_eq!(
            call(&db, "POST", "/api/v1/tickets/RUN-1/claim", json!({}))
                .await
                .status,
            409
        );
        assert_eq!(
            call(&db, "POST", "/api/v1/next/claim", json!({}))
                .await
                .status,
            204
        );
        ok(&db, "POST", "/api/v1/tickets/RUN-2/claim/release", lease).await;
        ok(&db, "POST", "/api/v1/tickets/RUN-1/claim", Value::Null).await;
        let history = ok(&db, "GET", "/api/v1/tickets/RUN-2/history", Value::Null).await;
        let fields: Vec<_> = history["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["field"].as_str().unwrap())
            .collect();
        for field in ["claim", "heartbeat", "release"] {
            assert!(fields.contains(&field));
        }
    });
}

#[test]
fn concurrent_enqueue_and_patch_are_fenced_and_events_rollback() {
    let path = std::env::temp_dir().join(format!("tik151-{}.sqlite", std::process::id()));
    let db = Sqlite::new();
    block_on(seed(&db));
    db.0.execute("VACUUM INTO ?", [path.to_str().unwrap()])
        .unwrap();
    for method in ["PUT", "PATCH"] {
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let threads: Vec<_> = (0..2).map(|n| {
            let path=path.clone();let barrier=barrier.clone();
            std::thread::spawn(move || {
                let connection=rusqlite::Connection::open(path).unwrap();
                connection.busy_timeout(std::time::Duration::from_secs(5)).unwrap();
                connection.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
                let db=Sqlite(connection);
                barrier.wait();
                block_on(call(&db,method,if method=="PUT" {"/api/v1/runs/RUN-1"} else {"/api/v1/runs/1"},if method=="PUT" {json!({"owner":format!("box-{n}")})} else {json!({"expected_version":1,"owner":format!("reconciler-{n}"),"state":"claimed"})}))
            })
        }).collect();
        let mut replies: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
        replies.sort_by_key(|r| r.status);
        assert_eq!(
            replies.iter().map(|r| r.status).collect::<Vec<_>>(),
            if method == "PUT" {
                vec![200, 201]
            } else {
                vec![200, 409]
            }
        );
        if method == "PUT" {
            assert_eq!(replies[0].body, replies[1].body);
        } else {
            assert_eq!(replies[0].body, replies[1].body["current"]);
        }
    }
    let db = Sqlite(rusqlite::Connection::open(&path).unwrap());
    block_on(async {
        assert_eq!(events(&db).await.as_array().unwrap().len(), 2);
        let before = ok(&db, "GET", "/api/v1/runs/1", Value::Null).await;
        db.0.execute_batch("CREATE TRIGGER fail_run_update BEFORE UPDATE ON pipeline_runs BEGIN SELECT RAISE(ABORT,'injected update failure'); END;").unwrap();
        assert_eq!(
            call(
                &db,
                "PATCH",
                "/api/v1/runs/1",
                json!({"expected_version":2,"state":"checks"})
            )
            .await
            .status,
            500
        );
        assert_eq!(events(&db).await.as_array().unwrap().len(), 2);
        assert_eq!(ok(&db, "GET", "/api/v1/runs/1", Value::Null).await, before);
    });
    drop(db);
    std::fs::remove_file(path).unwrap();
}
