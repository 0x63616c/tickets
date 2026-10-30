#![cfg(not(target_arch = "wasm32"))]
mod common;
use common::Sqlite;
use futures::executor::block_on;
use serde_json::{Value, json};
use tickets_core::history::Actor;
use tickets_worker::api::handle_as;

#[test]
fn identity_permissions_soft_delete_and_history() {
    block_on(async {
        let db = Sqlite::new();
        let mut user = Actor::user(1);
        user.label = "Sample user".into();
        let mut bot = Actor::api_token(1);
        bot.label = "Build agent".into();
        let mut other = Actor::api_token(2);
        other.label = bot.label.clone();
        for (path, body) in [
            (
                "projects",
                json!({"name":"Example","key_prefix":"EX","color":"#789abc"}),
            ),
            ("tickets", json!({"project_id":1,"title":"Comments"})),
        ] {
            let r = handle_as(&db, &user, "POST", &format!("/api/v1/{path}"), &[], body).await;
            assert_eq!(r.status, 201, "{}", r.body);
        }
        for actor in [&user, &bot] {
            let r = handle_as(
                &db,
                actor,
                "POST",
                "/api/v1/tickets/EX-1/comments",
                &[],
                json!({"body":"**Hello** <script>bad()</script>"}),
            )
            .await;
            assert_eq!(r.status, 201, "{}", r.body);
            assert_eq!(r.body["author"], actor.label);
            assert_eq!(
                r.body["author_kind"],
                if actor.id == user.id && actor.label == user.label {
                    "user"
                } else {
                    "bot"
                }
            );
            assert_eq!(r.body["can_edit"], true);
            assert_eq!(r.body["deleted"], false);
            assert_eq!(r.body["edited_at"], Value::Null);
            assert!(r.body.get("author_id").is_none());
            assert!(!r.body["body_html"].as_str().unwrap().contains("<script>"));
        }
        for actor in [&user, &other] {
            for method in ["PATCH", "DELETE"] {
                let r = handle_as(
                    &db,
                    actor,
                    method,
                    "/api/v1/comments/2",
                    &[],
                    json!({"body":"no"}),
                )
                .await;
                assert_eq!(r.status, 403, "{}", r.body);
            }
        }
        let r = handle_as(&db, &bot, "GET", "/api/v1/comments/1", &[], Value::Null).await;
        assert_eq!(r.body["can_edit"], false);
        // Renaming changes attribution on future posts, never ownership.
        bot.label = "Renamed agent".into();
        let r = handle_as(
            &db,
            &bot,
            "PATCH",
            "/api/v1/comments/2",
            &[],
            json!({"body":"Updated"}),
        )
        .await;
        assert_eq!(r.status, 200, "{}", r.body);
        assert!(r.body["edited_at"].is_string());
        assert_eq!(r.body["author"], "Build agent");
        let r = handle_as(&db, &bot, "DELETE", "/api/v1/comments/2", &[], Value::Null).await;
        assert_eq!(r.status, 200, "{}", r.body);
        let r = handle_as(
            &db,
            &bot,
            "GET",
            "/api/v1/tickets/1/comments",
            &[],
            Value::Null,
        )
        .await;
        assert_eq!(r.body["data"][1]["deleted"], true);
        assert_eq!(r.body["data"][1]["body"], "");
        assert_eq!(r.body["data"][1]["body_html"], "");
        assert_eq!(r.body["data"][1]["can_edit"], false);
        let r = handle_as(&db, &bot, "GET", "/api/v1/tickets/1", &[], Value::Null).await;
        assert_eq!(r.body["comment_count"], 1);
        assert_eq!(
            db.0.query_row("SELECT count(*) FROM comments", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            2
        );
        for method in ["PATCH", "DELETE"] {
            assert_eq!(
                handle_as(
                    &db,
                    &bot,
                    method,
                    "/api/v1/comments/2",
                    &[],
                    json!({"body":"Restore"})
                )
                .await
                .status,
                404
            );
        }
        let r = handle_as(
            &db,
            &bot,
            "GET",
            "/api/v1/tickets/1/history",
            &[],
            Value::Null,
        )
        .await;
        assert_eq!(r.status, 200, "{}", r.body);
        let events = r.body["data"].as_array().unwrap();
        assert_eq!(
            events
                .iter()
                .map(|e| e["field"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "comment_deleted",
                "comment_edited",
                "comment_created",
                "comment_created",
                "created"
            ]
        );
        assert_eq!(events[0]["actor_label"], "Renamed agent");
        assert_eq!(events[0]["old_value"], json!({"id":2}));
        let cursor = events[1]["id"].to_string();
        let page = handle_as(
            &db,
            &bot,
            "GET",
            "/api/v1/tickets/1/history",
            &[("cursor".into(), cursor), ("limit".into(), "1".into())],
            Value::Null,
        )
        .await;
        assert_eq!(page.body["data"][0]["field"], "comment_created");
        let list = handle_as(&db, &user, "GET", "/api/v1/tickets", &[], Value::Null).await;
        assert_eq!(list.body["data"][0]["comment_count"], 1);
        db.0.execute(
            "INSERT INTO comments(ticket_id,author,body) VALUES(1,'owner','Legacy body')",
            [],
        )
        .unwrap();
        let legacy = handle_as(&db, &user, "GET", "/api/v1/comments/3", &[], Value::Null).await;
        assert_eq!(legacy.body["can_edit"], false);
        assert_eq!(
            handle_as(&db, &user, "DELETE", "/api/v1/comments/3", &[], Value::Null)
                .await
                .status,
            403
        );
        // Ticket deletion may cascade; retained events stay unreachable.
        assert_eq!(
            handle_as(&db, &user, "DELETE", "/api/v1/tickets/1", &[], Value::Null)
                .await
                .status,
            200
        );
        assert_eq!(
            handle_as(
                &db,
                &user,
                "GET",
                "/api/v1/tickets/1/history",
                &[],
                Value::Null
            )
            .await
            .status,
            404
        );
    });
}

#[test]
fn authentication_resolves_labels_without_extra_queries() {
    use tickets_worker::{auth_queries, repository::Store};
    block_on(async {
        let db = Sqlite::new();
        db.0.execute_batch("INSERT INTO users(id,username,display_name) VALUES(1,'fixture','Sample User'); INSERT INTO sessions(token_hash,user_id,expires_at) VALUES('fixture-hash',1,'2999-01-01T00:00:00.000Z'); INSERT INTO api_tokens(user_id,token_hash,name) VALUES(1,'fixture-hash','Build agent');").unwrap();
        for (sql, label) in [
            (auth_queries::SESSION, "Sample User"),
            (auth_queries::BEARER, "Build agent"),
        ] {
            let rows = db
                .query::<Value>(sql, vec![json!("fixture-hash")])
                .await
                .unwrap();
            assert_eq!(rows[0]["actor_label"], label);
        }
        db.0.execute("UPDATE users SET display_name=NULL", [])
            .unwrap();
        let rows = db
            .query::<Value>(auth_queries::SESSION, vec![json!("fixture-hash")])
            .await
            .unwrap();
        assert_eq!(rows[0]["actor_label"], "fixture");
    });
}

#[test]
fn legacy_machine_authors_and_same_named_bot_remain_distinct() {
    block_on(async {
        let db = Sqlite::new();
        let mut bot = Actor::api_token(1);
        bot.label = "GitHub".into();
        for (path, body) in [
            (
                "projects",
                json!({"name":"Example","key_prefix":"EX","color":"#789abc"}),
            ),
            ("tickets", json!({"project_id":1,"title":"Machine authors"})),
        ] {
            assert_eq!(
                handle_as(&db, &bot, "POST", &format!("/api/v1/{path}"), &[], body)
                    .await
                    .status,
                201
            );
        }
        // Match the INSERT shape retained by migration 0008's PR/status triggers.
        db.0.execute("INSERT INTO comments(ticket_id,author,body) VALUES(1,'GitHub','Linked PR'),(1,'system','Status changed')",[]).unwrap();
        let created = handle_as(
            &db,
            &bot,
            "POST",
            "/api/v1/tickets/1/comments",
            &[],
            json!({"body":"Bot progress"}),
        )
        .await;
        assert_eq!(created.status, 201);
        assert_eq!(created.body["author_kind"], "bot");
        assert_eq!(created.body["can_edit"], true);
        let list = handle_as(
            &db,
            &bot,
            "GET",
            "/api/v1/tickets/1/comments",
            &[],
            Value::Null,
        )
        .await;
        assert_eq!(list.body["data"][0]["author_kind"], "github");
        assert_eq!(list.body["data"][0]["can_edit"], false);
        assert_eq!(list.body["data"][1]["author_kind"], "system");
        assert_eq!(list.body["data"][1]["can_edit"], false);
        assert_eq!(
            handle_as(&db, &bot, "DELETE", "/api/v1/comments/1", &[], Value::Null)
                .await
                .status,
            403
        );
    });
}
