//! Canonical public API surface. Every dispatch passes through this table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handler {
    Spec,
    Auth,
    Domain,
    LegacyWebhook,
    AppWebhook,
    Manifest,
}
pub struct Route {
    pub method: &'static str,
    pub path: &'static str,
    pub handler: Handler,
}
pub const ROUTES: &[Route] = &[
    Route {
        method: "GET",
        path: "/api/openapi.json",
        handler: Handler::Spec,
    },
    Route {
        method: "GET",
        path: "/api/health",
        handler: Handler::Auth,
    },
    Route {
        method: "POST",
        path: "/api/v1/login",
        handler: Handler::Auth,
    },
    Route {
        method: "POST",
        path: "/api/v1/logout",
        handler: Handler::Auth,
    },
    Route {
        method: "GET",
        path: "/api/v1/me",
        handler: Handler::Auth,
    },
    Route {
        method: "PATCH",
        path: "/api/v1/me",
        handler: Handler::Auth,
    },
    Route {
        method: "GET",
        path: "/api/v1/tokens",
        handler: Handler::Auth,
    },
    Route {
        method: "POST",
        path: "/api/v1/tokens",
        handler: Handler::Auth,
    },
    Route {
        method: "DELETE",
        path: "/api/v1/tokens/{id}",
        handler: Handler::Auth,
    },
    Route {
        method: "GET",
        path: "/api/v1/users",
        handler: Handler::Auth,
    },
    Route {
        method: "GET",
        path: "/api/v1/version",
        handler: Handler::Auth,
    },
    Route {
        method: "PUT",
        path: "/api/v1/me/avatar",
        handler: Handler::Auth,
    },
    Route {
        method: "POST",
        path: "/api/v1/me/avatar",
        handler: Handler::Auth,
    },
    Route {
        method: "DELETE",
        path: "/api/v1/me/avatar",
        handler: Handler::Auth,
    },
    Route {
        method: "GET",
        path: "/api/v1/users/{id}/avatar",
        handler: Handler::Auth,
    },
    Route {
        method: "GET",
        path: "/api/v1/projects",
        handler: Handler::Domain,
    },
    Route {
        method: "POST",
        path: "/api/v1/projects",
        handler: Handler::Domain,
    },
    Route {
        method: "GET",
        path: "/api/v1/projects/{id}",
        handler: Handler::Domain,
    },
    Route {
        method: "PATCH",
        path: "/api/v1/projects/{id}",
        handler: Handler::Domain,
    },
    Route {
        method: "GET",
        path: "/api/v1/tickets",
        handler: Handler::Domain,
    },
    Route {
        method: "POST",
        path: "/api/v1/tickets",
        handler: Handler::Domain,
    },
    Route {
        method: "GET",
        path: "/api/v1/tickets/{ticket}",
        handler: Handler::Domain,
    },
    Route {
        method: "PATCH",
        path: "/api/v1/tickets/{ticket}",
        handler: Handler::Domain,
    },
    Route {
        method: "DELETE",
        path: "/api/v1/tickets/{ticket}",
        handler: Handler::Domain,
    },
    Route {
        method: "GET",
        path: "/api/v1/labels",
        handler: Handler::Domain,
    },
    Route {
        method: "POST",
        path: "/api/v1/labels",
        handler: Handler::Domain,
    },
    Route {
        method: "GET",
        path: "/api/v1/labels/{id}",
        handler: Handler::Domain,
    },
    Route {
        method: "PATCH",
        path: "/api/v1/labels/{id}",
        handler: Handler::Domain,
    },
    Route {
        method: "DELETE",
        path: "/api/v1/labels/{id}",
        handler: Handler::Domain,
    },
    Route {
        method: "PUT",
        path: "/api/v1/tickets/by-external",
        handler: Handler::Domain,
    },
    Route {
        method: "GET",
        path: "/api/v1/tickets/{ticket}/history",
        handler: Handler::Domain,
    },
    Route {
        method: "GET",
        path: "/api/v1/tickets/{ticket}/comments",
        handler: Handler::Domain,
    },
    Route {
        method: "POST",
        path: "/api/v1/tickets/{ticket}/comments",
        handler: Handler::Domain,
    },
    Route {
        method: "GET",
        path: "/api/v1/comments/{id}",
        handler: Handler::Domain,
    },
    Route {
        method: "PATCH",
        path: "/api/v1/comments/{id}",
        handler: Handler::Domain,
    },
    Route {
        method: "DELETE",
        path: "/api/v1/comments/{id}",
        handler: Handler::Domain,
    },
    Route {
        method: "POST",
        path: "/api/v1/tickets/{ticket}/labels/{label_id}",
        handler: Handler::Domain,
    },
    Route {
        method: "DELETE",
        path: "/api/v1/tickets/{ticket}/labels/{label_id}",
        handler: Handler::Domain,
    },
    Route {
        method: "POST",
        path: "/api/v1/tickets/{ticket}/dependencies/{blocked_ticket_id}",
        handler: Handler::Domain,
    },
    Route {
        method: "DELETE",
        path: "/api/v1/tickets/{ticket}/dependencies/{blocked_ticket_id}",
        handler: Handler::Domain,
    },
    Route {
        method: "GET",
        path: "/api/v1/pickup-rules",
        handler: Handler::Domain,
    },
    Route {
        method: "PUT",
        path: "/api/v1/pickup-rules",
        handler: Handler::Domain,
    },
    Route {
        method: "DELETE",
        path: "/api/v1/pickup-rules",
        handler: Handler::Domain,
    },
    Route {
        method: "GET",
        path: "/api/v1/next",
        handler: Handler::Domain,
    },
    Route {
        method: "POST",
        path: "/api/v1/next/claim",
        handler: Handler::Domain,
    },
    Route {
        method: "POST",
        path: "/api/v1/tickets/{ticket}/claim/heartbeat",
        handler: Handler::Domain,
    },
    Route {
        method: "POST",
        path: "/api/v1/tickets/{ticket}/claim/release",
        handler: Handler::Domain,
    },
    Route {
        method: "GET",
        path: "/api/v1/search",
        handler: Handler::Domain,
    },
    Route {
        method: "GET",
        path: "/api/v1/analytics/home",
        handler: Handler::Domain,
    },
    Route {
        method: "GET",
        path: "/api/v1/mission",
        handler: Handler::Domain,
    },
    Route {
        method: "PUT",
        path: "/api/v1/mission/reports/{source}",
        handler: Handler::Domain,
    },
    Route {
        method: "GET",
        path: "/api/v1/live/{project_id}",
        handler: Handler::Auth,
    },
    Route {
        method: "GET",
        path: "/api/v1/github/app",
        handler: Handler::Domain,
    },
    Route {
        method: "GET",
        path: "/api/v1/github/app/credentials",
        handler: Handler::Domain,
    },
    Route {
        method: "POST",
        path: "/api/v1/github/webhook",
        handler: Handler::LegacyWebhook,
    },
    Route {
        method: "POST",
        path: "/api/v1/github/app/webhook",
        handler: Handler::AppWebhook,
    },
    Route {
        method: "GET",
        path: "/api/v1/github/app/manifest-callback",
        handler: Handler::Manifest,
    },
];
/// Exact segments, with nonempty path-template parameters. Static paths win.
pub fn resolve(method: &str, path: &str) -> Option<&'static Route> {
    let path = path.trim_end_matches('/');
    ROUTES
        .iter()
        .filter(|r| r.method == method)
        .find(|r| r.path == path)
        .or_else(|| {
            ROUTES.iter().filter(|r| r.method == method).find(|r| {
                let mut actual = path.split('/');
                r.path.split('/').all(|expected| {
                    actual.next().is_some_and(|segment| {
                        if expected.starts_with('{') {
                            !segment.is_empty()
                        } else {
                            expected == segment
                        }
                    })
                }) && actual.next().is_none()
            })
        })
}
/// Embedded bytes only: no Store, Env, D1 or authentication dependency.
pub const OPENAPI: &str = include_str!("../../../docs/openapi.json");
#[cfg(target_arch = "wasm32")]
pub fn spec_response() -> worker::Result<worker::Response> {
    let mut response = worker::Response::ok(OPENAPI)?;
    response
        .headers_mut()
        .set("Content-Type", "application/json; charset=utf-8")?;
    response
        .headers_mut()
        .set("Cache-Control", "public, max-age=3600")?;
    Ok(response)
}

/// Path lookup is used only to preserve public handlers' method-not-allowed errors.
pub fn resolve_path(path: &str) -> Option<&'static Route> {
    ROUTES.iter().find(|r| r.path == path)
}
