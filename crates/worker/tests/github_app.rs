#![cfg(not(target_arch = "wasm32"))]

mod common;
use common::Sqlite;
use futures::executor::block_on;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::cell::Cell;
use tickets_core::github_app::{AppMetadata, CallbackError, pack_ciphertext};
use tickets_worker::{github_app_registration as registration, repository::Store};

fn metadata() -> AppMetadata {
    AppMetadata {
        app_id: 42,
        slug: "fixture-app".into(),
        name: "Fixture's App".into(),
        owner_login: "fixture-owner".into(),
        client_id: "fixture-client-id".into(),
        html_url: "https://github.com/apps/fixture-app".into(),
    }
}

struct CountingStore {
    db: Sqlite,
    reads: Cell<usize>,
}
impl Store for CountingStore {
    async fn query<T: DeserializeOwned>(
        &self,
        sql: &str,
        params: Vec<Value>,
    ) -> tickets_worker::repository::Result<Vec<T>> {
        self.reads.set(self.reads.get() + 1);
        self.db.query(sql, params).await
    }
}

#[test]
fn callback_preflight_statuses_do_not_need_network_or_authentication() {
    block_on(async {
        let store = CountingStore {
            db: Sqlite::new(),
            reads: Cell::new(0),
        };
        // Standard base64 of a deliberately invalid production key (32 zero bytes).
        let key = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
        for (setup, key, state, code, status) in [
            (None, Some(key), "fixture-state", "code", 503),
            (Some(""), Some(key), "fixture-state", "code", 503),
            (Some("fixture-state"), None, "fixture-state", "code", 503),
            (
                Some("fixture-state"),
                Some(""),
                "fixture-state",
                "code",
                503,
            ),
            (
                Some("fixture-state"),
                Some("invalid"),
                "fixture-state",
                "code",
                503,
            ),
            (Some("fixture-state"), Some(key), "bad-state", "code", 403),
            (
                Some("fixture-state"),
                Some(key),
                "fixture-state",
                "bad/code",
                400,
            ),
        ] {
            let result = registration::preflight(&store, setup, key, state, code).await;
            assert_eq!(result.unwrap_err().status(), status);
            assert_eq!(store.reads.get(), 0, "invalid requests must not reach D1");
        }
        assert!(
            registration::preflight(
                &store,
                Some("fixture-state"),
                Some(key),
                "fixture-state",
                "code"
            )
            .await
            .is_ok()
        );
        assert_eq!(store.reads.get(), 1);
        let ciphertext = pack_ciphertext(&[0; 12], &[0; 32]).unwrap();
        registration::register(&store.db, &metadata(), &ciphertext)
            .await
            .unwrap();
        let result = registration::preflight(
            &store,
            Some("fixture-state"),
            Some(key),
            "fixture-state",
            "code",
        )
        .await;
        assert_eq!(result.unwrap_err(), CallbackError::AlreadyRegistered);
        assert_eq!(store.reads.get(), 2);
    });
}

#[test]
fn singleton_registration_is_parameterized_and_never_overwrites_a_winner() {
    block_on(async {
        let db = Sqlite::new();
        assert!(registration::load(&db).await.unwrap().is_none());
        let ciphertext = pack_ciphertext(&[1; 12], &[2; 32]).unwrap();
        registration::register(&db, &metadata(), &ciphertext)
            .await
            .unwrap();
        let mut loser = metadata();
        loser.app_id = 99;
        assert_eq!(
            registration::register(&db, &loser, "other").await,
            Err(CallbackError::AlreadyRegistered)
        );
        let stored = registration::load(&db).await.unwrap().unwrap();
        assert_eq!(stored.app.app_id, 42);
        assert_eq!(stored.app.name, "Fixture's App");
        assert_eq!(stored.secrets_ciphertext, ciphertext);
        let rows: Vec<Value> = db
            .query("SELECT created_at FROM github_app", vec![])
            .await
            .unwrap();
        let created = rows[0]["created_at"].as_str().unwrap();
        assert_eq!(created.len(), 24);
        assert!(created.ends_with('Z') && created.contains('T'));
        assert!(db.0.execute("UPDATE github_app SET id=2", []).is_err());
        assert!(
            db.0.execute("UPDATE github_app SET app_id=NULL", [])
                .is_err()
        );
        assert!(
            db.0.execute("UPDATE github_app SET secrets_ciphertext=NULL", [])
                .is_err()
        );
    });
}

#[test]
fn installation_schema_enforces_identity_cascades_and_timestamps() {
    let db = Sqlite::new();
    db.0.execute("INSERT INTO github_app_installations(installation_id,account_login) VALUES(7,'fixture-owner')", []).unwrap();
    db.0.execute(
        "INSERT INTO github_app_installation_repos VALUES(7,'fixture-owner/repo')",
        [],
    )
    .unwrap();
    assert!(
        db.0.execute(
            "INSERT INTO github_app_installation_repos VALUES(7,'fixture-owner/repo')",
            []
        )
        .is_err()
    );
    assert!(
        db.0.execute(
            "INSERT INTO github_app_installation_repos VALUES(8,'fixture-owner/repo')",
            []
        )
        .is_err()
    );
    let rows: Vec<Value> =
        block_on(db.query("SELECT * FROM github_app_installations", vec![])).unwrap();
    assert_eq!(rows[0]["repository_selection"], Value::Null);
    assert_eq!(rows[0]["suspended_at"], Value::Null);
    assert_eq!(rows[0]["created_at"], rows[0]["updated_at"]);
    assert_eq!(rows[0]["created_at"].as_str().unwrap().len(), 24);
    db.0.execute(
        "DELETE FROM github_app_installations WHERE installation_id=7",
        [],
    )
    .unwrap();
    let rows: Vec<Value> = block_on(db.query(
        "SELECT count(*) AS count FROM github_app_installation_repos",
        vec![],
    ))
    .unwrap();
    assert_eq!(rows[0]["count"], json!(0));
}

#[test]
fn credentials_export_requires_token_and_returns_stored_ciphertext() {
    block_on(async {
        use tickets_core::history::Actor;
        let store = CountingStore {
            db: Sqlite::new(),
            reads: Cell::new(0),
        };
        let path = "/api/v1/github/app/credentials";
        let token = Actor::api_token(1);
        assert!(registration::load(&store.db).await.unwrap().is_none());
        let missing =
            tickets_worker::api::handle_as(&store, &token, "GET", path, &[], Value::Null).await;
        assert_eq!(missing.status, 404);
        assert_eq!(missing.body["error"]["code"], "not_found");
        let ciphertext = pack_ciphertext(&[1; 12], &[2; 32]).unwrap();
        registration::register(&store.db, &metadata(), &ciphertext)
            .await
            .unwrap();
        for actor in [Actor::user(1), Actor::system(), Actor::webhook()] {
            let reads = store.reads.get();
            let denied =
                tickets_worker::api::handle_as(&store, &actor, "GET", path, &[], Value::Null).await;
            assert_eq!(denied.status, 403);
            assert_eq!(denied.body["error"]["code"], "forbidden");
            assert_eq!(store.reads.get(), reads);
        }
        let reply =
            tickets_worker::api::handle_as(&store, &token, "GET", path, &[], Value::Null).await;
        assert_eq!(reply.status, 200);
        assert!(reply.change.is_none());
        assert_eq!(
            reply.body,
            json!({
                "app_id":42, "slug":"fixture-app", "name":"Fixture's App",
                "owner_login":"fixture-owner", "client_id":"fixture-client-id",
                "html_url":"https://github.com/apps/fixture-app", "secrets_ciphertext":ciphertext
            })
        );
    });
}
