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
