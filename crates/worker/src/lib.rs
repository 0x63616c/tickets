pub mod avatars;
pub mod d1_errors;
#[cfg(target_arch = "wasm32")]
pub mod github_app;
pub mod github_app_events;
pub mod github_app_registration;
#[cfg(target_arch = "wasm32")]
mod github_app_webhook;
#[cfg(target_arch = "wasm32")]
mod github_mirror;
#[cfg(target_arch = "wasm32")]
mod github_prs;
#[cfg(target_arch = "wasm32")]
mod live;
#[cfg(target_arch = "wasm32")]
mod read_cache;
#[cfg(target_arch = "wasm32")]
pub use live::BoardRoom;
mod search;
// Cloudflare entry point; all private API routes pass through the auth gate.
#[cfg(target_arch = "wasm32")]
mod auth;
#[cfg(target_arch = "wasm32")]
use worker::*;
#[cfg(target_arch = "wasm32")]
#[event(fetch)]
pub async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    if req.path() == "/api/v1/github/app/manifest-callback" {
        return github_app::manifest_callback(req, &env).await;
    }
    if req.path() == "/api/v1/github/app/webhook" {
        return github_app_webhook::webhook(req, &env).await;
    }
    if req.path() == "/api/v1/github/webhook" {
        return github_mirror::webhook(req, &env).await;
    }
    if req.path() == "/api" || req.path().starts_with("/api/") {
        return match auth::handle(req, env).await {
            Ok(response) => Ok(response),
            Err(error) => request_error(error),
        };
    }
    let path = req.path();
    let public = is_public_asset(&path);
    if !public {
        let url = req.url()?;
        let signed_in = match auth::has_session(&req, &env).await {
            Ok(value) => value,
            Err(error) => return request_error(error),
        };
        let destination = if path == "/login" {
            if signed_in {
                let next = url
                    .query_pairs()
                    .find(|(k, _)| k == "next")
                    .map(|(_, v)| v.into_owned())
                    .unwrap_or_default();
                Some(tickets_core::navigation::safe_next(&next).to_owned())
            } else {
                None
            }
        } else if !signed_in {
            let current = format!(
                "{}{}",
                url.path(),
                url.query().map(|q| format!("?{q}")).unwrap_or_default()
            );
            let encoded: String = tickets_core::navigation::safe_next(&current)
                .bytes()
                .map(|b| {
                    if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                        (b as char).to_string()
                    } else {
                        format!("%{b:02X}")
                    }
                })
                .collect();
            Some(format!("/login?next={encoded}"))
        } else {
            None
        };
        if let Some(destination) = destination {
            let mut response = Response::empty()?.with_status(302);
            response.headers_mut().set("Location", &destination)?;
            response.headers_mut().set("Cache-Control", "no-store")?;
            return Ok(response);
        }
    }
    if path == "/github/app/register" {
        return github_app::register_page(req, &env).await;
    }
    let asset_request = if public {
        req
    } else {
        let mut url = req.url()?;
        url.set_path("/index.html");
        url.set_query(None);
        Request::new(url.as_str(), req.method())?
    };
    let mut response = env.assets("ASSETS")?.fetch_request(asset_request).await?;
    if !public {
        // Binding responses have immutable headers; copy them before setting cache policy.
        let headers = Headers::new();
        for (name, value) in response.headers().entries() {
            headers.set(&name, &value)?;
        }
        headers.set("Cache-Control", "no-store")?;
        response = response.with_headers(headers);
    }
    Ok(response)
}
#[cfg(target_arch = "wasm32")]
fn request_error(error: worker::Error) -> Result<Response> {
    if matches!(error, worker::Error::RustError(ref message) if message == d1_errors::MARKER) {
        auth::reply(serde_json::json!({"error":d1_errors::MESSAGE}), 503)
    } else {
        auth::reply(
            serde_json::json!({"error":"Request could not be completed"}),
            500,
        )
    }
}
pub mod repository;

pub mod api;
mod api_repository;
pub mod auth_queries;

// Only build-produced assets are public; unknown paths (even with dots) are gated.
#[cfg(any(target_arch = "wasm32", test))]
fn is_public_asset(path: &str) -> bool {
    // wasm-bindgen emits the local DOM bridge under a content-addressed snippet.
    // Match only build-owned modules, not arbitrary nested JavaScript paths.
    path.strip_prefix("/snippets/tickets-web-")
        .and_then(|rest| {
            rest.strip_suffix("/src/components/dom.js")
                .or_else(|| rest.strip_suffix("/node_modules/ascii-reveal/dist/core.js"))
                .or_else(|| {
                    rest.strip_suffix("/node_modules/sortablejs/modular/sortable.complete.esm.js")
                })
        })
        .is_some_and(|hash| hash.len() == 16 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
        || path.starts_with("/fonts/")
        || matches!(
            path,
            "/favicon.ico"
                | "/favicon.svg"
                | "/favicon-32.png"
                | "/apple-touch-icon.png"
                | "/icon-192.png"
                | "/icon-512.png"
                | "/icon-maskable-512.png"
                | "/site.webmanifest"
        )
        || (path.matches('/').count() == 1
            && path.rsplit_once('-').is_some_and(|(_, tail)| {
                tail.split_once('.').is_some_and(|(hash, ext)| {
                    hash.strip_suffix("_bg").unwrap_or(hash).len() >= 8
                        && hash
                            .strip_suffix("_bg")
                            .unwrap_or(hash)
                            .bytes()
                            .all(|b| b.is_ascii_hexdigit())
                        && matches!(ext, "js" | "wasm" | "css")
                })
            }))
}

#[cfg(test)]
mod tests {
    use super::is_public_asset;

    #[test]
    fn only_known_hashed_modules_are_public() {
        assert!(is_public_asset(
            "/snippets/tickets-web-8e589db3b4472a38/src/components/dom.js"
        ));
        assert!(is_public_asset(
            "/snippets/tickets-web-8e589db3b4472a38/node_modules/ascii-reveal/dist/core.js"
        ));
        assert!(is_public_asset(
            "/snippets/tickets-web-8e589db3b4472a38/node_modules/sortablejs/modular/sortable.complete.esm.js"
        ));
        for path in [
            "/snippets/tickets-web-8e589db3b4472a38/node_modules/ascii-reveal/dist/core.js.bak",
            "/snippets/tickets-web-8e589db3b4472a38/node_modules/ascii-reveal/dist/index.js",
            "/snippets/tickets-web-notahash/node_modules/ascii-reveal/dist/core.js",
            "/snippets/tickets-web-8e589db3b4472a38/node_modules/sortablejs/modular/sortable.complete.esm.js.bak",
            "/snippets/tickets-web-notahash/node_modules/sortablejs/modular/sortable.complete.esm.js",
            "/snippets/private.js",
            "/snippets/tickets-web-notahash/src/components/dom.js",
            "/snippets/tickets-web-8e589db3b4472a38/src/components/secret.js",
            "/snippets/tickets-web-8e589db3b4472a38/src/components/dom.js.bak",
        ] {
            assert!(!is_public_asset(path), "{path} must stay gated");
        }
    }

    #[test]
    fn icon_paths_are_public_exactly() {
        for path in [
            "/favicon.ico",
            "/favicon.svg",
            "/favicon-32.png",
            "/apple-touch-icon.png",
            "/icon-192.png",
            "/icon-512.png",
            "/icon-maskable-512.png",
            "/site.webmanifest",
        ] {
            assert!(is_public_asset(path), "{path} must be public");
            for suffix in [".bak", "/x"] {
                let nearby = format!("{path}{suffix}");
                assert!(!is_public_asset(&nearby), "{nearby} must stay gated");
            }
        }
        for path in [
            "/secret.png",
            "/icons/favicon.svg",
            "/login",
            "/",
            "/t/QUA-87",
            "/github/app/register",
        ] {
            assert!(!is_public_asset(path), "{path} must stay gated");
        }
    }
}

pub mod analytics;

pub mod mutation;

pub mod pickup;

#[cfg(target_arch = "wasm32")]
#[event(scheduled)]
pub async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    let result = async {
        let db = env.d1("DB")?;
        let rows = pickup::sweep(&db)
            .await
            .map_err(|e| worker::Error::RustError(e.to_string()))?;
        if !rows.is_empty() {
            let mut projects = rows
                .iter()
                .filter_map(|v| v["project_id"].as_i64())
                .collect::<Vec<_>>();
            projects.sort_unstable();
            projects.dedup();
            let change = api::CommittedChange {
                entity: "tickets".into(),
                id: 0,
                action: "update".into(),
                projects,
            };
            if live::notify(&env, &db, change).await.is_err() {
                console_error!("Claim expiry notification failed");
            }
        }
        Ok::<(), worker::Error>(())
    }
    .await;
    if result.is_err() {
        console_error!("Claim expiry sweep failed");
    }
}

pub mod mission;
