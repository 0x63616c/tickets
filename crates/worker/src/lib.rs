//! Cloudflare entry point; all private API routes pass through the auth gate.
#[cfg(target_arch = "wasm32")]
mod auth;
#[cfg(target_arch = "wasm32")]
use worker::*;
#[cfg(target_arch = "wasm32")]
#[event(fetch)]
pub async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    if req.path() == "/api" || req.path().starts_with("/api/") {
        return match auth::handle(req, env).await {
            Ok(response) => Ok(response),
            Err(_) => Ok(Response::from_json(&serde_json::json!({"error":{"code":"internal","message":"Request could not be completed"}}))?.with_status(500)),
        };
    }
    env.assets("ASSETS")?.fetch_request(req).await
}
pub mod repository;

pub mod api;
mod api_repository;
pub mod auth_queries;
