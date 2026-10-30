#![cfg(not(target_arch = "wasm32"))]
mod common;
use common::Sqlite;
use futures::executor::block_on;
use serde_json::{Value, json};
use tickets_worker::{api::handle, mission, repository::Store};

#[test]
fn validation_raw_storage_and_replacement() {
    block_on(async {
        let db = Sqlite::new();
        for source in ["", "Box", "a_b", "../box", &"x".repeat(41)] {
            assert_eq!(mission::put(&db, source, "{}").await.status, 422);
        }
        for raw in [
            "null",
            "[]",
            "1",
            "\"hello\"",
            "{",
            "{\"jobs\":{}}",
            "{\"builds\":[1]}",
            "{\"needs_calum\":null}",
        ] {
            assert_eq!(mission::put(&db, "box", raw).await.status, 422, "{raw}");
        }
        for field in tickets_core::mission::LIST_FIELDS {
            assert_eq!(
                mission::put(&db, "box", &json!({*field:[null]}).to_string())
                    .await
                    .status,
                422
            );
        }
        let raw = format!("{{\"extra\":\"{}\"}}", "x".repeat(512 * 1024));
        assert_eq!(mission::put(&db, "box", &raw).await.status, 413);
        let raw = "{ \"jobs\": [{}], \"future\": true }";
        assert_eq!(mission::put(&db, "box", raw).await.status, 200);
        let stored: Vec<Value> = db
            .query("SELECT body FROM mission_reports", vec![])
            .await
            .unwrap();
        assert_eq!(stored[0]["body"], raw);
        let receipt = mission::put(&db, "box", "{\"builds\":[]}").await;
        assert_eq!(receipt.status, 200);
        assert_eq!(receipt.body["source"], "box");
        assert!(receipt.body["received_at"].is_string());
        let overview = mission::get(&db).await.unwrap();
        assert_eq!(overview["reports"].as_array().unwrap().len(), 1);
        assert_eq!(overview["reports"][0]["report"], json!({"builds":[]}));
        assert_eq!(overview["reports"][0]["stale"], false);
        db.0.execute("UPDATE mission_reports SET received_at=strftime('%Y-%m-%dT%H:%M:%fZ','now','-11 minutes')",[]).unwrap();
        assert_eq!(
            mission::get(&db).await.unwrap()["reports"][0]["stale"],
            true
        );
    });
}
#[test]
fn cross_project_queues_current_head_ci_done_window_and_archive() {
    block_on(async {
        let db = Sqlite::new();
        for prefix in ["EX", "OTHER", "ARCH"] {
            let r = handle(
                &db,
                "POST",
                "/api/v1/projects",
                &[],
                json!({"name":prefix,"key_prefix":prefix,"color":"#123456"}),
            )
            .await;
            assert_eq!(r.status, 201);
        }
        for (project, status) in [
            (1, "todo"),
            (2, "in_review"),
            (3, "in_review"),
            (1, "done"),
            (1, "done"),
        ] {
            let r = handle(
                &db,
                "POST",
                "/api/v1/tickets",
                &[],
                json!({"project_id":project,"title":"Example","status":"todo"}),
            )
            .await;
            let id = r.body["id"].as_i64().unwrap();
            assert_eq!(
                handle(
                    &db,
                    "PATCH",
                    &format!("/api/v1/tickets/{id}"),
                    &[],
                    json!({"status":status})
                )
                .await
                .status,
                200
            );
        }
        db.0.execute_batch("INSERT INTO ticket_labels SELECT 1,id FROM labels WHERE name='waiting-on-calum';
            UPDATE projects SET archived=1 WHERE id=3;
            UPDATE ticket_events SET recorded_at=strftime('%Y-%m-%dT%H:%M:%fZ','now','-49 hours') WHERE subject_id=(SELECT id FROM ticket_subjects WHERE ticket_id=5) AND new_value='done';
            INSERT INTO ticket_pull_requests VALUES(2,'example/repo',10,'https://github.com/example/repo/pull/10','Review','open',1,'current','2026-10-30T10:00:00.000Z');
            INSERT INTO github_pull_request_checks(repo,number,head_sha,ci_status,ci_updated_at) VALUES('example/repo',10,'old','failure','2026-10-30T12:00:00.000Z'),('example/repo',10,'current','success','2026-10-30T11:00:00.000Z');").unwrap();
        let response = handle(&db, "GET", "/api/v1/mission", &[], Value::Null).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let m: tickets_core::mission::Mission = serde_json::from_value(response.body).unwrap();
        assert!(m.generated_at.ends_with('Z'));
        assert_eq!(m.waiting_on_calum.len(), 1);
        assert_eq!(m.waiting_on_calum[0].key, "EX-1");
        assert_eq!(m.waiting_on_calum[0].labels[0].name, "waiting-on-calum");
        assert_eq!(m.in_review.len(), 1);
        assert_eq!(m.in_review[0].key, "OTHER-1");
        assert_eq!(m.in_review[0].pull_requests[0]["ci_status"], "success");
        assert_eq!(m.recently_done.len(), 1);
        assert_eq!(m.recently_done[0].id, 4);
    });
}

#[test]
fn done_queue_is_bounded_sorted_and_not_extended_by_waiting_label() {
    block_on(async {
        let db = Sqlite::new();
        handle(
            &db,
            "POST",
            "/api/v1/projects",
            &[],
            json!({"name":"Example","key_prefix":"EX","color":"#123456"}),
        )
        .await;
        for n in 0..35 {
            let t = handle(
                &db,
                "POST",
                "/api/v1/tickets",
                &[],
                json!({"project_id":1,"title":format!("Done {n}"),"status":"done"}),
            )
            .await;
            assert_eq!(t.status, 201);
        }
        db.0.execute_batch("INSERT INTO ticket_labels SELECT 1,id FROM labels WHERE name='waiting-on-calum'; UPDATE ticket_events SET recorded_at=strftime('%Y-%m-%dT%H:%M:%fZ','now','-49 hours') WHERE subject_id=(SELECT id FROM ticket_subjects WHERE ticket_id=1);").unwrap();
        let value = mission::get(&db).await.unwrap();
        assert_eq!(value["waiting_on_calum"].as_array().unwrap().len(), 1);
        let done = value["recently_done"].as_array().unwrap();
        assert_eq!(done.len(), 30);
        assert!(!done.iter().any(|t| t["id"] == 1));
        assert_eq!(done[0]["id"], 35);
    });
}
