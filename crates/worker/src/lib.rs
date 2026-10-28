//! Cloudflare entry point; all private API routes pass through the auth gate.
#[cfg(target_arch = "wasm32")]
mod auth;
#[cfg(target_arch = "wasm32")]
use worker::*;
#[cfg(target_arch = "wasm32")]
#[event(fetch)]
pub async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    if req.path() == "/api" || req.path().starts_with("/api/") {
        return auth::handle(req, env).await;
    }
    env.assets("ASSETS")?.fetch_request(req).await
}
pub mod repository;

pub mod auth_queries;
