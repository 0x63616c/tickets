#![cfg(not(target_arch = "wasm32"))]
mod common;
use common::Sqlite;
use futures::executor::block_on;
use serde_json::Value;
use tickets_core::domain::*;
use tickets_worker::repository::{Error, Repository, Store};

fn edge(a: i64, b: i64) -> Dependency {
    Dependency {
        blocker_id: a,
        blocked_id: b,
    }
}

#[test]
fn repository_round_trip() {
    block_on(async {
        let r = Repository::new(Sqlite::new());
        assert!(r.create_project("", "Q", "#123456").await.is_err());
        let mut p = r.create_project("Quantum", "QUA", "#123456").await.unwrap();
        assert!(
            r.create_project("duplicate", "QUA", "#123456")
                .await
                .is_err()
        );
        p.github_owner = Some("example-org".into());
        p.github_project_number = Some(7);
        p.github_repo = Some("example".into());
        p.archived = true;
        assert_eq!(r.save_project(&p).await.unwrap(), p);
        assert_eq!(r.projects().await.unwrap(), vec![p.clone()]);
        let mut a = r.create_ticket(p.id, "First", "**body**").await.unwrap();
        for status in [
            Status::Backlog,
            Status::Todo,
            Status::InProgress,
            Status::InReview,
            Status::Done,
            Status::Canceled,
        ] {
            a.status = status;
            assert_eq!(r.save_ticket(&a).await.unwrap().status, status);
        }
        let b = r.create_ticket(p.id, "Second", "").await.unwrap();
        let c = r.create_ticket(p.id, "Third", "").await.unwrap();
        assert_eq!((a.number, b.number, c.number), (1, 2, 3));
        assert_eq!(r.project(p.id).await.unwrap().next_ticket_number, 4);
        assert_eq!(
            tickets_core::domain::ticket_key(&p.key_prefix, a.number).unwrap(),
            "QUA-1"
        );
        a.status = Status::InProgress;
        a.priority = Priority::High;
        a.sort_order = -0.5;
        a.assignee = Some("agent".into());
        a.external_id = Some("example/issue/1".into());
        a.external_url = Some("https://example.com/issues/1".into());
        let a = r.save_ticket(&a).await.unwrap();
        assert_eq!(r.ticket(a.id).await.unwrap(), a);
        assert_eq!(r.tickets(p.id).await.unwrap().len(), 3);
        assert_eq!(r.tickets(p.id).await.unwrap()[0].id, a.id);
        let mut duplicate = b.clone();
        duplicate.external_id = a.external_id.clone();
        assert!(r.save_ticket(&duplicate).await.is_err());
        let label = r.create_label("bug", "#ff0000").await.unwrap();
        assert!(r.labels().await.unwrap().contains(&label));
        assert!(r.create_label("bug", "#ffffff").await.is_err());
        r.add_label(a.id, label.id).await.unwrap();
        assert_eq!(r.ticket_labels(a.id).await.unwrap(), vec![label.clone()]);
        r.remove_label(a.id, label.id).await.unwrap();
        assert!(r.ticket_labels(a.id).await.unwrap().is_empty());
        r.add_label(a.id, label.id).await.unwrap();
        r.add_dependency(edge(a.id, b.id)).await.unwrap();
        r.add_dependency(edge(b.id, c.id)).await.unwrap();
        assert!(matches!(
            r.add_dependency(edge(c.id, a.id)).await,
            Err(Error::Dependency(_))
        ));
        assert!(matches!(
            r.add_dependency(edge(a.id, a.id)).await,
            Err(Error::Dependency(_))
        ));
        assert!(r.add_dependency(edge(a.id, 9999)).await.is_err());
        r.remove_dependency(edge(b.id, c.id)).await.unwrap();
        assert_eq!(r.dependencies().await.unwrap(), vec![edge(a.id, b.id)]);
        let comment = r
            .create_comment(a.id, "agent", "hello **world**")
            .await
            .unwrap();
        assert_eq!(r.comments(a.id).await.unwrap(), vec![comment]);
        r.delete_ticket(a.id).await.unwrap();
        assert!(r.comments(a.id).await.unwrap().is_empty());
        assert!(r.ticket_labels(a.id).await.unwrap().is_empty());
        assert!(r.dependencies().await.unwrap().is_empty());
        assert!(matches!(r.ticket(a.id).await, Err(Error::NotFound)));
        assert_eq!(r.create_ticket(p.id, "Fourth", "").await.unwrap().number, 4);
        assert!(r.create_ticket(9999, "Missing project", "").await.is_err());
        let injection = "'); DROP TABLE tickets; --";
        let literal = r
            .create_ticket(p.id, injection, "<script>raw markdown</script>")
            .await
            .unwrap();
        assert_eq!(literal.title, injection);
        assert_eq!(literal.body, "<script>raw markdown</script>");
        assert!(!tickets_core::markdown::render(&literal.body).contains("<script>"));
    });
}

#[test]
fn database_constraints_and_atomic_cycle_guard() {
    let s = Sqlite::new();
    let db = &s.0;
    db.execute("INSERT INTO users(id,username) VALUES(1,'local-user')", [])
        .unwrap();
    assert!(
        db.execute("INSERT INTO users VALUES(2,'other','now')", [])
            .is_err()
    );
    db.execute_batch("INSERT INTO projects(id,name,key_prefix,color) VALUES(1,'Example','EX','#123456'); INSERT INTO tickets(id,project_id,number,title) VALUES(1,1,1,'a'),(2,1,2,'b'),(3,1,3,'c'); INSERT INTO ticket_dependencies VALUES(1,2),(2,3);").unwrap();
    for sql in [
        "INSERT INTO ticket_dependencies VALUES(3,1)",
        "INSERT INTO ticket_dependencies VALUES(1,1)",
        "INSERT INTO ticket_dependencies VALUES(1,2)",
        "UPDATE ticket_dependencies SET blocker_id=3 WHERE blocker_id=1",
        "INSERT INTO tickets(project_id,number,title,status) VALUES(1,4,'bad','unknown')",
        "INSERT INTO tickets(project_id,number,title,priority) VALUES(1,4,'bad',5)",
        "INSERT INTO tickets(project_id,number,title) VALUES(1,1,'duplicate')",
        "INSERT INTO ticket_labels VALUES(1,999)",
        "DELETE FROM projects WHERE id=1",
    ] {
        assert!(db.execute(sql, []).is_err(), "{sql}");
    }
    assert_eq!(
        db.query_row("PRAGMA foreign_key_check", [], |_| Ok(()))
            .optional()
            .unwrap(),
        None
    );
}
use rusqlite::OptionalExtension;

#[test]
fn auth_expiry_revocation_and_hash_only_storage() {
    use tickets_core::auth::hash_token;
    use tickets_worker::auth_queries::{BEARER, SESSION};
    block_on(async {
        let store = Sqlite::new();
        store
            .0
            .execute("INSERT INTO users(id,username) VALUES(1,'fixture')", [])
            .unwrap();
        let columns: Vec<String> = store
            .0
            .prepare("SELECT name FROM pragma_table_info('users')")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        assert!(
            !columns.iter().any(|c| c.contains("password")),
            "{columns:?}"
        );
        let hash = hash_token("test credential");

        store.query::<Value>("INSERT INTO sessions(token_hash,user_id,expires_at) VALUES(?,1,'2000-01-01T00:00:00.000Z')",vec![hash.clone().into()]).await.unwrap();
        assert!(
            store
                .query::<Value>(SESSION, vec![hash.clone().into()])
                .await
                .unwrap()
                .is_empty()
        );
        store
            .0
            .execute(
                "UPDATE sessions SET expires_at='9999-01-01T00:00:00.000Z'",
                [],
            )
            .unwrap();
        assert_eq!(
            store
                .query::<Value>(SESSION, vec![hash.clone().into()])
                .await
                .unwrap()
                .len(),
            1
        );
        store
            .query::<Value>(
                "INSERT INTO api_tokens(user_id,token_hash,name) VALUES(1,?,'fixture')",
                vec![hash.clone().into()],
            )
            .await
            .unwrap();
        assert_eq!(
            store
                .query::<Value>(BEARER, vec![hash.clone().into()])
                .await
                .unwrap()
                .len(),
            1
        );
        let used: String = store
            .0
            .query_row("SELECT last_used_at FROM api_tokens", [], |r| r.get(0))
            .unwrap();
        assert!(used.ends_with('Z'));
        store
            .0
            .execute(
                "UPDATE api_tokens SET revoked_at='2026-01-01T00:00:00.000Z'",
                [],
            )
            .unwrap();
        assert!(
            store
                .query::<Value>(BEARER, vec![hash.clone().into()])
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            store
                .0
                .query_row::<String, _, _>("SELECT token_hash FROM api_tokens", [], |r| r.get(0))
                .unwrap(),
            hash
        );
    });
}
