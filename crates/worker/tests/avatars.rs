#![cfg(not(target_arch = "wasm32"))]
mod common;
use common::Sqlite;
use futures::executor::block_on;
use tickets_worker::avatars;

#[test]
fn upload_replace_remove_and_binary_roundtrip() {
    block_on(async {
        let db = Sqlite::new();
        db.0.execute("INSERT INTO users(id,username) VALUES(1,'sample')", [])
            .unwrap();
        let original = b"RIFF\x04\0\0\0WEBPtest";
        let first = avatars::save(&db, 1, "image/webp", original).await.unwrap();
        assert!(
            first["avatar_url"]
                .as_str()
                .unwrap()
                .contains("/users/1/avatar?v=")
        );
        let photo = avatars::get(&db, 1).await.unwrap().unwrap();
        assert_eq!(photo.bytes, original);
        assert_eq!(photo.content_type, "image/webp");
        assert!(avatars::get(&db, 2).await.unwrap().is_none());
        let second = avatars::save(&db, 1, "image/jpeg", b"\xff\xd8\xffnew")
            .await
            .unwrap();
        assert_ne!(first["avatar_url"], second["avatar_url"]);
        assert_eq!(
            avatars::get(&db, 1).await.unwrap().unwrap().bytes,
            b"\xff\xd8\xffnew"
        );
        assert!(avatars::remove(&db, 1).await.unwrap()["avatar_url"].is_null());
        assert!(avatars::get(&db, 1).await.unwrap().is_none());
        assert!(avatars::remove(&db, 1).await.unwrap()["avatar_url"].is_null());
    });
}
#[test]
fn invalid_type_mismatch_empty_and_oversize_preserve_existing_photo() {
    block_on(async {
        let db = Sqlite::new();
        db.0.execute("INSERT INTO users(id,username) VALUES(1,'sample')", [])
            .unwrap();
        avatars::save(&db, 1, "image/png", b"\x89PNG\r\n\x1a\noriginal")
            .await
            .unwrap();
        for (mime, bytes) in [
            ("image/svg+xml", b"<svg/>".as_slice()),
            ("image/png", b"\xff\xd8\xff"),
            ("image/webp", b"RIFF"),
            ("image/jpeg", b""),
        ] {
            assert_eq!(avatars::save(&db, 1, mime, bytes).await.unwrap_err(), 415);
        }
        assert_eq!(
            avatars::save(&db, 1, "image/png", &vec![0; avatars::MAX_BYTES + 1])
                .await
                .unwrap_err(),
            413
        );
        assert_eq!(
            avatars::get(&db, 1).await.unwrap().unwrap().bytes,
            b"\x89PNG\r\n\x1a\noriginal"
        );
        let kind: String =
            db.0.query_row("SELECT typeof(bytes) FROM user_avatars", [], |r| r.get(0))
                .unwrap();
        assert_eq!(kind, "blob");
    });
}

#[test]
fn comments_use_durable_identity_and_versioned_photos() {
    use serde_json::{Value, json};
    use tickets_core::history::Actor;
    use tickets_worker::api::handle_as;
    block_on(async {
        let db = Sqlite::new();
        db.0.execute("INSERT INTO users(id,username) VALUES(1,'sample')", [])
            .unwrap();
        let mut user = Actor::user(1);
        user.label = "Same name".into();
        let mut bot = Actor::api_token(1);
        bot.label = user.label.clone();
        for (entity, body) in [
            (
                "projects",
                json!({"name":"Photos","key_prefix":"AV","color":"#535fc4"}),
            ),
            ("tickets", json!({"project_id":1,"title":"Identity"})),
        ] {
            assert_eq!(
                handle_as(&db, &user, "POST", &format!("/api/v1/{entity}"), &[], body)
                    .await
                    .status,
                201
            );
        }
        let photo = avatars::save(&db, 1, "image/webp", b"RIFF\x04\0\0\0WEBPtest")
            .await
            .unwrap();
        for (actor, url) in [(&user, photo["avatar_url"].clone()), (&bot, Value::Null)] {
            let response = handle_as(
                &db,
                actor,
                "POST",
                "/api/v1/tickets/1/comments",
                &[],
                json!({"body":"Hello"}),
            )
            .await;
            assert_eq!(response.status, 201, "{}", response.body);
            assert_eq!(response.body["avatar_url"], url);
        }
        let list = handle_as(
            &db,
            &user,
            "GET",
            "/api/v1/tickets/1/comments",
            &[],
            Value::Null,
        )
        .await;
        assert_eq!(list.body["data"][0]["avatar_url"], photo["avatar_url"]);
        assert!(list.body["data"][1]["avatar_url"].is_null());
        avatars::remove(&db, 1).await.unwrap();
        let list = handle_as(
            &db,
            &user,
            "GET",
            "/api/v1/tickets/1/comments",
            &[],
            Value::Null,
        )
        .await;
        assert!(list.body["data"][0]["avatar_url"].is_null());
    });
}
