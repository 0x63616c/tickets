#![cfg(not(target_arch = "wasm32"))]
mod common;
use common::Sqlite;
use futures::executor::block_on;
use serde_json::{Value, json};
use tickets_worker::api::handle;
async fn call(db: &Sqlite, m: &str, p: &str, b: Value, expected: u16) -> Value {
    let (path, query) = p.split_once('?').unwrap_or((p, ""));
    let q = query
        .split('&')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let (k, v) = s.split_once('=').unwrap();
            (k.into(), v.into())
        })
        .collect::<Vec<_>>();
    let r = handle(db, m, path, &q, b).await;
    assert_eq!(r.status, expected, "{m} {p}: {}", r.body);
    if expected >= 400 {
        assert!(r.body["error"]["code"].is_string());
        assert!(r.body["error"]["message"].is_string());
    }
    r.body
}
#[test]
fn every_route_and_filters() {
    block_on(async {
        let db = Sqlite::new();
        let p=call(&db,"POST","/api/v1/projects",json!({"name":"Example","key_prefix":"EX","color":"#123456","github_owner":"example","github_project_number":3,"github_repo":"repo"}),201).await;
        let pid = p["id"].as_i64().unwrap();
        call(&db, "GET", "/api/v1/projects", Value::Null, 200).await;
        call(
            &db,
            "GET",
            &format!("/api/v1/projects/{pid}"),
            Value::Null,
            200,
        )
        .await;
        assert_eq!(
            call(
                &db,
                "PATCH",
                &format!("/api/v1/projects/{pid}"),
                json!({"archived":true,"name":"Renamed"}),
                200
            )
            .await["archived"],
            true
        );
        let a=call(&db,"POST","/api/v1/tickets",json!({"project_id":pid,"title":"First searchword","body":"**bold** <script>alert(1)</script>","status":"todo","priority":2,"assignee":"agent","sort_order":1.5}),201).await;
        assert_eq!(a["key"], "EX-1");
        assert!(a["body_html"].as_str().unwrap().contains("<strong>"));
        assert!(!a["body_html"].as_str().unwrap().contains("<script>"));
        let b = call(
            &db,
            "POST",
            "/api/v1/tickets",
            json!({"project_id":pid,"title":"Second","body":"needle"}),
            201,
        )
        .await;
        let aid = a["id"].as_i64().unwrap();
        let bid = b["id"].as_i64().unwrap();
        call(&db, "GET", "/api/v1/tickets/EX-1", Value::Null, 200).await;
        call(
            &db,
            "GET",
            &format!("/api/v1/tickets/{aid}"),
            Value::Null,
            200,
        )
        .await;
        call(&db,"PATCH",&format!("/api/v1/tickets/{aid}"),json!({"title":"Edited searchword","body":"body","status":"in_progress","priority":1,"assignee":null,"sort_order":-1}),200).await;
        let label = call(
            &db,
            "POST",
            "/api/v1/labels",
            json!({"name":"bug","color":"#abcdef"}),
            201,
        )
        .await;
        let lid = label["id"].as_i64().unwrap();
        call(&db, "GET", "/api/v1/labels", Value::Null, 200).await;
        call(
            &db,
            "GET",
            &format!("/api/v1/labels/{lid}"),
            Value::Null,
            200,
        )
        .await;
        call(
            &db,
            "PATCH",
            &format!("/api/v1/labels/{lid}"),
            json!({"name":"defect","color":"#654321"}),
            200,
        )
        .await;
        call(
            &db,
            "POST",
            &format!("/api/v1/tickets/{aid}/labels/{lid}"),
            Value::Null,
            200,
        )
        .await;
        let edge = format!("/api/v1/tickets/{aid}/dependencies/{bid}");
        call(&db, "POST", &edge, Value::Null, 200).await;
        for filter in [
            "status=in_progress".into(),
            format!("label={lid}"),
            "blocked=true".into(),
            "blocked=false".into(),
            "q=searchword".into(),
            "q=needle".into(),
        ] {
            assert_eq!(
                call(
                    &db,
                    "GET",
                    &format!("/api/v1/tickets?{filter}"),
                    Value::Null,
                    200
                )
                .await["data"]
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
        }
        assert_eq!(
            call(
                &db,
                "GET",
                &format!("/api/v1/tickets?project={pid}"),
                Value::Null,
                200
            )
            .await["data"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let combined=call(&db,"GET",&format!("/api/v1/tickets?project={pid}&status=in_progress&label={lid}&blocked=false&q=searchword&limit=1"),Value::Null,200).await;
        assert_eq!(combined["data"][0]["id"], aid);
        assert_eq!(combined["data"].as_array().unwrap().len(), 1);
        let r = call(
            &db,
            "GET",
            "/api/v1/tickets?assignee=agent",
            Value::Null,
            200,
        )
        .await;
        assert_eq!(r["data"], json!([]));
        let first = call(&db, "GET", "/api/v1/tickets?limit=1", Value::Null, 200).await;
        let next = call(
            &db,
            "GET",
            &format!(
                "/api/v1/tickets?limit=1&cursor={}",
                first["next_cursor"].as_str().unwrap()
            ),
            Value::Null,
            200,
        )
        .await;
        assert_eq!(next["data"][0]["id"], bid);
        assert!(next["next_cursor"].is_null());
        let full = call(
            &db,
            "GET",
            &format!("/api/v1/tickets/{aid}"),
            Value::Null,
            200,
        )
        .await;
        assert_eq!(full["labels"][0]["id"], lid);
        assert_eq!(full["blocks"], json!([bid]));
        assert_eq!(
            call(
                &db,
                "GET",
                &format!("/api/v1/tickets/{bid}"),
                Value::Null,
                200
            )
            .await["blocked_by"],
            json!([aid])
        );
        call(&db, "POST", &edge, Value::Null, 409).await;
        call(
            &db,
            "POST",
            &format!("/api/v1/tickets/{bid}/dependencies/{aid}"),
            Value::Null,
            409,
        )
        .await;
        call(
            &db,
            "POST",
            &format!("/api/v1/tickets/{aid}/dependencies/{aid}"),
            Value::Null,
            409,
        )
        .await;
        call(&db, "DELETE", &edge, Value::Null, 200).await;
        call(
            &db,
            "DELETE",
            &format!("/api/v1/tickets/{aid}/labels/{lid}"),
            Value::Null,
            200,
        )
        .await;
        let c = call(
            &db,
            "POST",
            &format!("/api/v1/tickets/{aid}/comments"),
            json!({"body":"**comment** <script>bad</script>"}),
            201,
        )
        .await;
        let cid = c["id"].as_i64().unwrap();
        assert!(!c["body_html"].as_str().unwrap().contains("<script>"));
        call(
            &db,
            "GET",
            &format!("/api/v1/tickets/{aid}/comments?limit=1"),
            Value::Null,
            200,
        )
        .await;
        call(
            &db,
            "GET",
            &format!("/api/v1/comments/{cid}"),
            Value::Null,
            200,
        )
        .await;
        call(
            &db,
            "PATCH",
            &format!("/api/v1/comments/{cid}"),
            json!({"body":"Updated"}),
            200,
        )
        .await;
        call(
            &db,
            "DELETE",
            &format!("/api/v1/comments/{cid}"),
            Value::Null,
            200,
        )
        .await;
        let ext = "/api/v1/tickets/by-external?id=github:example/repo:1";
        let imported = call(
            &db,
            "PUT",
            ext,
            json!({"project_id":pid,"title":"Imported","external_url":"https://example.test/1"}),
            200,
        )
        .await;
        let updated = call(
            &db,
            "PUT",
            ext,
            json!({"project_id":pid,"title":"Updated import"}),
            200,
        )
        .await;
        assert_eq!(imported["id"], updated["id"]);
        assert_eq!(imported["number"], updated["number"]);
        let dest = call(
            &db,
            "POST",
            "/api/v1/projects",
            json!({"name":"Destination","key_prefix":"DST","color":"#123456"}),
            201,
        )
        .await;
        let moved = call(
            &db,
            "PATCH",
            &format!("/api/v1/tickets/{aid}"),
            json!({"project_id":dest["id"]}),
            200,
        )
        .await;
        assert_eq!(moved["key"], "DST-1");
        let new = call(
            &db,
            "POST",
            "/api/v1/tickets",
            json!({"project_id":dest["id"],"title":"Next"}),
            201,
        )
        .await;
        assert_eq!(new["key"], "DST-2");
        call(
            &db,
            "DELETE",
            &format!("/api/v1/labels/{lid}"),
            Value::Null,
            200,
        )
        .await;
        call(
            &db,
            "DELETE",
            &format!("/api/v1/tickets/{aid}"),
            Value::Null,
            200,
        )
        .await;
        call(
            &db,
            "GET",
            &format!("/api/v1/tickets/{aid}"),
            Value::Null,
            404,
        )
        .await;
    });
}
#[test]
fn errors_and_validation() {
    block_on(async {
        let db = Sqlite::new();
        for (m, p, b, status) in [
            (
                "POST",
                "/api/v1/projects",
                json!({"name":"","key_prefix":"bad","color":"red"}),
                422,
            ),
            ("POST", "/api/v1/projects", json!({"name":"a"}), 422),
            ("POST", "/api/v1/projects", json!([]), 422),
            (
                "POST",
                "/api/v1/tickets",
                json!({"project_id":999,"title":"Missing"}),
                404,
            ),
            (
                "POST",
                "/api/v1/tickets",
                json!({"project_id":1,"title":"Bad","priority":5}),
                422,
            ),
            (
                "POST",
                "/api/v1/tickets",
                json!({"project_id":1,"title":"Bad","status":"unknown"}),
                422,
            ),
            ("GET", "/api/v1/tickets?limit=101", Value::Null, 400),
            ("GET", "/api/v1/tickets?cursor=bad", Value::Null, 400),
            ("GET", "/api/v1/tickets?blocked=maybe", Value::Null, 400),
            ("GET", "/api/v1/tickets?status=unknown", Value::Null, 400),
            ("GET", "/api/v1/tickets?unknown=x", Value::Null, 400),
            ("GET", "/api/v1/projects/no", Value::Null, 400),
            ("GET", "/api/v1/comments/99", Value::Null, 404),
            ("GET", "/api/v1/missing", Value::Null, 404),
            (
                "PUT",
                "/api/v1/tickets/by-external",
                json!({"title":"a","project_id":1}),
                400,
            ),
            (
                "PUT",
                "/api/v1/tickets/by-external?id=bad",
                json!({"title":"a","project_id":1}),
                422,
            ),
        ] {
            call(&db, m, p, b, status).await;
        }
        call(
            &db,
            "POST",
            "/api/v1/labels",
            json!({"name":"x","color":"#123456"}),
            201,
        )
        .await;
        call(
            &db,
            "POST",
            "/api/v1/labels",
            json!({"name":"x","color":"#123456"}),
            409,
        )
        .await;
        call(&db, "PATCH", "/api/v1/labels/1", json!({}), 400).await;
        call(&db, "PATCH", "/api/v1/labels/1", json!({"id":2}), 422).await;
    });
}

#[test]
fn moves_imports_search_and_cascades() {
    block_on(async {
        let db = Sqlite::new();
        for (name, prefix) in [("One", "ONE"), ("Two", "TWO")] {
            call(
                &db,
                "POST",
                "/api/v1/projects",
                json!({"name":name,"key_prefix":prefix,"color":"#123456"}),
                201,
            )
            .await;
        }
        let ext = "/api/v1/tickets/by-external?id=fixture:42";
        let first = call(
            &db,
            "PUT",
            ext,
            json!({"project_id":1,"title":"oldword","body":"uniquebody"}),
            200,
        )
        .await;
        let moved = call(
            &db,
            "PUT",
            ext,
            json!({"project_id":2,"title":"newword"}),
            200,
        )
        .await;
        assert_eq!(first["id"], moved["id"]);
        assert_eq!(first["created_at"], moved["created_at"]);
        assert_eq!(moved["key"], "TWO-1");
        assert_eq!(moved["body"], "uniquebody");
        call(&db, "GET", "/api/v1/tickets/ONE-1", Value::Null, 404).await;
        let same = call(
            &db,
            "PATCH",
            "/api/v1/tickets/1",
            json!({"project_id":2}),
            200,
        )
        .await;
        assert_eq!(same["key"], "TWO-1");
        let next = call(
            &db,
            "POST",
            "/api/v1/tickets",
            json!({"project_id":2,"title":"Next"}),
            201,
        )
        .await;
        assert_eq!(next["key"], "TWO-2");
        for (q, count) in [
            ("oldword", 0),
            ("newword", 1),
            ("uniquebody", 1),
            ("\"", 0),
            ("' OR 1=1 --", 0),
        ] {
            let r = handle(
                &db,
                "GET",
                "/api/v1/tickets",
                &[("q".into(), q.into())],
                Value::Null,
            )
            .await;
            assert_eq!(r.status, 200, "{}", r.body);
            assert_eq!(r.body["data"].as_array().unwrap().len(), count);
        }
        call(
            &db,
            "POST",
            "/api/v1/tickets/1/comments",
            json!({"body":"comment"}),
            201,
        )
        .await;
        call(
            &db,
            "POST",
            "/api/v1/labels",
            json!({"name":"linked","color":"#123456"}),
            201,
        )
        .await;
        call(&db, "POST", "/api/v1/tickets/1/labels/1", Value::Null, 200).await;
        call(
            &db,
            "POST",
            "/api/v1/tickets/1/dependencies/2",
            Value::Null,
            200,
        )
        .await;
        call(
            &db,
            "PATCH",
            "/api/v1/tickets/1",
            json!({"status":"done"}),
            200,
        )
        .await;
        assert_eq!(
            call(&db, "GET", "/api/v1/tickets?blocked=true", Value::Null, 200).await["data"],
            json!([])
        );
        call(&db, "DELETE", "/api/v1/tickets/1", Value::Null, 200).await;
        call(&db, "GET", "/api/v1/comments/1", Value::Null, 404).await;
        assert_eq!(
            call(&db, "GET", "/api/v1/tickets/2", Value::Null, 200).await["blocked_by"],
            json!([])
        );
        assert_eq!(
            call(&db, "GET", "/api/v1/tickets?q=newword", Value::Null, 200).await["data"],
            json!([])
        );
        call(
            &db,
            "DELETE",
            "/api/v1/tickets/2/dependencies/1",
            Value::Null,
            404,
        )
        .await;
        call(&db, "POST", "/api/v1/tickets/2/labels/99", Value::Null, 404).await;
        call(
            &db,
            "POST",
            "/api/v1/tickets/99/comments",
            json!({"body":"missing"}),
            404,
        )
        .await;
        call(&db, "POST", "/api/v1/tickets/2/comments", json!({}), 422).await;
        call(
            &db,
            "POST",
            "/api/v1/tickets/2/comments",
            json!({"body":"  "}),
            422,
        )
        .await;
        call(
            &db,
            "GET",
            "/api/v1/tickets?limit=1&limit=2",
            Value::Null,
            400,
        )
        .await;
        call(
            &db,
            "GET",
            "/api/v1/tickets/2/comments?unknown=1",
            Value::Null,
            400,
        )
        .await;
        call(
            &db,
            "PATCH",
            "/api/v1/tickets/2",
            json!({"project_id":99}),
            404,
        )
        .await;
        call(&db, "PATCH", "/api/v1/tickets/2", json!({"body":null}), 422).await;
        call(
            &db,
            "PATCH",
            "/api/v1/tickets/2",
            json!({"external_id":":bad"}),
            422,
        )
        .await;
    });
}

/// Counts actual statements, including the same auth queries used by the Worker.
struct CountingStore {
    db: Sqlite,
    queries: std::cell::Cell<usize>,
}
impl tickets_worker::repository::Store for CountingStore {
    async fn query<T: serde::de::DeserializeOwned>(
        &self,
        sql: &str,
        params: Vec<Value>,
    ) -> tickets_worker::repository::Result<Vec<T>> {
        assert!(params.len() <= 100, "D1 parameter limit: {sql}");
        self.queries.set(self.queries.get() + 1);
        self.db.query(sql, params).await
    }
}
impl CountingStore {
    async fn request(
        &self,
        auth: &str,
        method: &str,
        path: &str,
        query: &[(String, String)],
        body: Value,
        expected: (u16, usize),
    ) -> Value {
        use tickets_worker::repository::Store;
        let (status, budget) = expected;
        self.queries.set(0);
        let user: Vec<Value> = self.query(auth, vec!["fixture-hash".into()]).await.unwrap();
        assert_eq!(user[0]["user_id"], 1);
        let reply = handle(self, method, path, query, body).await;
        assert_eq!(reply.status, status, "{}", reply.body);
        assert!(
            self.queries.get() <= budget,
            "{method} {path}: {} queries exceeds {budget}",
            self.queries.get()
        );
        reply.body
    }
}

#[test]
fn constant_query_budget_with_full_enrichment() {
    use tickets_worker::auth_queries::{BEARER, SESSION};
    block_on(async {
        let db = Sqlite::new();
        db.0.execute_batch("INSERT INTO users(id,username) VALUES(1,'owner');
            INSERT INTO api_tokens(user_id,token_hash,name) VALUES(1,'fixture-hash','fixture');
            INSERT INTO sessions(token_hash,user_id,expires_at) VALUES('fixture-hash',1,'2999-01-01T00:00:00.000Z');
            INSERT INTO projects(name,key_prefix,color) VALUES('Budget','BGT','#123456');
            INSERT INTO labels(name,color) VALUES('first','#123456'),('second','#abcdef');
            WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<102)
            INSERT INTO tickets(project_id,number,title,body) SELECT 1,x,'Ticket '||x,'**bold** <script>bad</script>' FROM n;
            INSERT INTO ticket_labels SELECT id,2 FROM tickets;
            INSERT INTO ticket_labels SELECT id,1 FROM tickets;
            INSERT INTO ticket_dependencies SELECT id,id+1 FROM tickets WHERE id<102;
            INSERT INTO ticket_dependencies SELECT id,id+2 FROM tickets WHERE id<101;").unwrap();
        let store = CountingStore {
            db,
            queries: std::cell::Cell::new(0),
        };
        for auth in [BEARER, SESSION] {
            let page = store
                .request(
                    auth,
                    "GET",
                    "/api/v1/tickets",
                    &[("limit".into(), "100".into())],
                    Value::Null,
                    (200, 2),
                )
                .await;
            assert_eq!(page["next_cursor"], "100");
            assert_eq!(page["data"].as_array().unwrap().len(), 100);
            for (index, ticket) in page["data"].as_array().unwrap().iter().enumerate() {
                let id = index + 1;
                assert_eq!(ticket["key"], format!("BGT-{id}"));
                assert_eq!(
                    ticket["labels"],
                    json!([{"id":1,"name":"first","color":"#123456"},{"id":2,"name":"second","color":"#abcdef"}])
                );
                assert_eq!(ticket["blocks"], json!([id + 1, id + 2]));
                let blocked_by: Vec<_> = (id.saturating_sub(2).max(1)..id).collect();
                assert_eq!(ticket["blocked_by"], json!(blocked_by));
                assert_eq!(ticket["body"], "**bold** <script>bad</script>");
                assert!(
                    ticket["body_html"]
                        .as_str()
                        .unwrap()
                        .contains("<strong>bold</strong>")
                );
                assert!(!ticket["body_html"].as_str().unwrap().contains("<script>"));
            }
            for path in ["/api/v1/tickets/50", "/api/v1/tickets/BGT-50"] {
                let ticket = store
                    .request(auth, "GET", path, &[], Value::Null, (200, 2))
                    .await;
                assert_eq!(ticket, page["data"][49]);
            }
            let created = store
                .request(
                    auth,
                    "POST",
                    "/api/v1/tickets",
                    &[],
                    json!({"project_id":1,"title":"Created"}),
                    (201, 4),
                )
                .await;
            assert_eq!(created["labels"], json!([]));
            assert_eq!(created["blocks"], json!([]));
            assert_eq!(created["blocked_by"], json!([]));
            let patched = store
                .request(
                    auth,
                    "PATCH",
                    "/api/v1/tickets/50",
                    &[],
                    json!({"project_id":1,"title":"Patched"}),
                    (200, 5),
                )
                .await;
            for field in ["key", "labels", "blocks", "blocked_by", "body_html"] {
                assert_eq!(patched[field], page["data"][49][field]);
            }
            store
                .request(
                    auth,
                    "PUT",
                    "/api/v1/tickets/by-external",
                    &[("id".into(), "fixture:budget".into())],
                    json!({"project_id":1,"title":"Import"}),
                    (200, 4),
                )
                .await;
            store
                .request(
                    auth,
                    "POST",
                    "/api/v1/tickets/1/dependencies/102",
                    &[],
                    Value::Null,
                    (200, 5),
                )
                .await;
            store
                .request(
                    auth,
                    "DELETE",
                    "/api/v1/tickets/1/dependencies/102",
                    &[],
                    Value::Null,
                    (200, 4),
                )
                .await;
        }
        store
            .db
            .0
            .execute(
                "INSERT INTO projects(name,key_prefix,color) VALUES('Destination','DST','#123456')",
                [],
            )
            .unwrap();
        let moved = store
            .request(
                BEARER,
                "PATCH",
                "/api/v1/tickets/51",
                &[],
                json!({"project_id":2}),
                (200, 5),
            )
            .await;
        assert_eq!(moved["key"], "DST-1");
        assert_eq!(moved["blocks"], json!([52, 53]));
        assert_eq!(moved["blocked_by"], json!([49, 50]));
        assert_eq!(moved["labels"].as_array().unwrap().len(), 2);
    });
}

#[test]
fn unexpected_storage_data_returns_internal_error() {
    struct Unexpected(Value);
    impl tickets_worker::repository::Store for Unexpected {
        async fn query<T: serde::de::DeserializeOwned>(
            &self,
            _: &str,
            _: Vec<Value>,
        ) -> tickets_worker::repository::Result<Vec<T>> {
            Ok(vec![serde_json::from_value(self.0.clone()).unwrap()])
        }
    }
    block_on(async {
        for row in [
            json!({"id":"unexpected"}),
            json!({"id":1,"key":"EX-1","labels":"invalid json"}),
        ] {
            let reply = handle(
                &Unexpected(row),
                "GET",
                "/api/v1/tickets/1",
                &[],
                Value::Null,
            )
            .await;
            assert_eq!(reply.status, 500);
            assert_eq!(
                reply.body,
                json!({"error":{"code":"internal","message":"Storage operation failed"}})
            );
        }
    });
}

#[test]
fn project_link_kind_and_archive_preserve_tickets() {
    block_on(async {
        let db = Sqlite::new();
        let p=call(&db,"POST","/api/v1/projects",json!({"name":"Engine","key_prefix":"ENG","color":"#5e6ad2","github_owner":"example","github_project_number":7,"github_project_kind":"orgs","github_repo":"engine"}),201).await;
        assert_eq!(p["github_project_kind"], "orgs");
        call(
            &db,
            "POST",
            "/api/v1/tickets",
            json!({"project_id":1,"title":"Keep ticket"}),
            201,
        )
        .await;
        call(
            &db,
            "PATCH",
            "/api/v1/projects/1",
            json!({"name":"Renamed","key_prefix":"REN","archived":true}),
            200,
        )
        .await;
        let ticket = call(&db, "GET", "/api/v1/tickets/1", Value::Null, 200).await;
        assert_eq!(ticket["key"], "REN-1");
        let p = call(&db, "GET", "/api/v1/projects/1", Value::Null, 200).await;
        assert_eq!(p["github_project_kind"], "orgs");
        assert_eq!(p["github_repo"], "engine");
        call(
            &db,
            "PATCH",
            "/api/v1/projects/1",
            json!({"github_project_kind":"invalid"}),
            422,
        )
        .await;
        let p=call(&db,"PATCH","/api/v1/projects/1",json!({"archived":false,"github_owner":null,"github_project_number":null,"github_repo":null,"github_project_kind":"users"}),200).await;
        assert_eq!(p["github_owner"], Value::Null);
        assert_eq!(p["github_project_kind"], "users");
    });
}
