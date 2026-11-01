#![cfg(not(target_arch = "wasm32"))]
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use tickets_worker::{
    repository::{Error, Result, Store},
    routes::{Handler, OPENAPI, ROUTES, resolve},
};

fn check_refs(node: &Value, root: &Value) {
    match node {
        Value::Object(map) => {
            if let Some(reference) = map.get("$ref") {
                let reference = reference.as_str().expect("string $ref");
                let pointer = reference.strip_prefix('#').expect("local refs only");
                assert!(root.pointer(pointer).is_some(), "unresolved {reference}");
            }
            for value in map.values() {
                check_refs(value, root);
            }
        }
        Value::Array(values) => {
            for value in values {
                check_refs(value, root);
            }
        }
        _ => {}
    }
}

#[test]
fn openapi_and_dispatch_table_are_exactly_in_sync() {
    let spec: Value = serde_json::from_str(OPENAPI).expect("valid spec JSON");
    assert_eq!(spec["openapi"], "3.1.0");
    assert!(spec["info"]["title"].is_string());
    assert!(spec["info"]["version"].is_string());
    assert!(spec["components"]["schemas"].is_object());
    check_refs(&spec, &spec);
    let mut actual = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for (path, item) in spec["paths"].as_object().unwrap() {
        assert!(path.starts_with("/api/"));
        for (method, operation) in item.as_object().unwrap() {
            assert!(
                ["get", "post", "put", "patch", "delete", "head", "options"]
                    .contains(&method.as_str())
            );
            assert!(ids.insert(operation["operationId"].as_str().expect("operationId")));
            assert!(operation["summary"].as_str().is_some_and(|s| !s.is_empty()));
            assert!(!operation["tags"].as_array().unwrap().is_empty());
            assert!(
                operation["responses"]
                    .as_object()
                    .is_some_and(|o| !o.is_empty())
            );
            for response in operation["responses"].as_object().unwrap().values() {
                let response = response.get("$ref").map_or(response, |r| {
                    spec.pointer(r.as_str().unwrap().trim_start_matches('#'))
                        .unwrap()
                });
                assert!(response["description"].is_string());
                if let Some(content) = response.get("content") {
                    for media in content.as_object().unwrap().values() {
                        assert!(media["schema"].is_object());
                    }
                }
            }
            for requirement in operation["security"].as_array().expect("explicit security") {
                for scheme in requirement.as_object().unwrap().keys() {
                    assert!(spec["components"]["securitySchemes"].get(scheme).is_some());
                }
            }
            if let Some(body) = operation.get("requestBody") {
                assert!(body["required"].is_boolean());
                let content = body["content"].as_object().expect("request media types");
                assert!(!content.is_empty());
                for media in content.values() {
                    assert!(media["schema"].is_object());
                }
            }
            let params = operation["parameters"].as_array().unwrap();
            for segment in path.split('/') {
                if let Some(name) = segment.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                    assert!(
                        params.iter().any(|p| p["name"] == name
                            && p["in"] == "path"
                            && p["required"] == true),
                        "missing parameter {path} {name}"
                    );
                }
            }
            let mut parameter_keys = BTreeSet::new();
            for p in params {
                assert!(
                    parameter_keys.insert((p["in"].as_str().unwrap(), p["name"].as_str().unwrap())),
                    "duplicate parameter {path}"
                );
                assert!(["path", "query", "header", "cookie"].contains(&p["in"].as_str().unwrap()));
                assert!(p["schema"].is_object());
                assert!(p["description"].is_string());
            }
            actual.insert((method.to_uppercase(), path.clone()));
        }
    }
    let expected: BTreeSet<_> = ROUTES
        .iter()
        .map(|r| (r.method.to_owned(), r.path.to_owned()))
        .collect();
    assert_eq!(expected.len(), ROUTES.len(), "duplicate route");
    assert_eq!(
        actual, expected,
        "update docs/openapi.json and routes.rs together"
    );
    // The entrypoint and domain/auth adapters must retain their table gate.
    assert!(
        include_str!("../src/lib.rs").contains("routes::resolve(req.method().as_ref(), &path)")
    );
    assert!(
        include_str!("../src/auth.rs")
            .contains("crate::routes::resolve(method.as_ref(), &path).is_none()")
    );
    assert!(
        include_str!("../src/api.rs").contains("route.handler == crate::routes::Handler::Domain")
    );
}

/// Any recognized domain handler reaches validation or storage. Unknown routes
/// return 404 without ever touching this failing Store, so stale table entries fail.
struct Unavailable;
impl Store for Unavailable {
    async fn query<T: DeserializeOwned>(&self, _: &str, _: Vec<Value>) -> Result<Vec<T>> {
        Err(Error::Unavailable)
    }
    async fn batch(
        &self,
        _: Vec<tickets_worker::repository::Statement>,
    ) -> Result<Vec<Vec<Value>>> {
        Err(Error::Unavailable)
    }
}
#[test]
fn every_domain_table_entry_has_a_real_dispatch_handler() {
    futures::executor::block_on(async {
        for route in ROUTES.iter().filter(|r| r.handler == Handler::Domain) {
            let mut path = route.path.to_owned();
            for segment in route.path.split('/').filter(|s| s.starts_with('{')) {
                path = path.replace(segment, "1");
            }
            let query = if path.ends_with("by-external") {
                vec![("id".into(), "test:1".into())]
            } else {
                vec![]
            };
            let body = if path.ends_with("pickup-rules") {
                json!({"skip_blocked":true,"skip_open_pr":true,"blockers_first":true,"priority_first":true,"excluded_labels":[],"concurrency_cap":3,"lease_seconds":1800})
            } else if path.ends_with("/heartbeat") || path.ends_with("/release") {
                json!({"claim_id":"0123456789abcdef0123456789abcdef"})
            } else if path.ends_with("/claim") {
                json!({})
            } else {
                Value::Null
            };
            let reply = tickets_worker::api::handle_as(
                &Unavailable,
                &tickets_core::history::Actor::api_token(1),
                route.method,
                &path,
                &query,
                body,
            )
            .await;
            assert_ne!(reply.status, 404, "stale route {} {}", route.method, path);
        }
    });
}
#[test]
fn route_matching_is_segment_exact_and_method_specific() {
    assert!(resolve("GET", "/api/v1/projects/1").is_some());
    assert!(resolve("PUT", "/api/v1/tickets/by-external").is_some());
    for path in [
        "/api/v1/projects/1/extra",
        "/api/v1/projects//extra",
        "/api/v1/missing",
        "/api/v1/tickets/1/claim/unknown",
    ] {
        assert!(resolve("GET", path).is_none(), "{path}");
    }
    assert!(resolve("DELETE", "/api/v1/projects/1").is_none());
}
