//! Cloudflare entry point; the runtime is only available on wasm32.
#[cfg(target_arch = "wasm32")]
use worker::*;

#[cfg(target_arch = "wasm32")]
#[event(fetch)]
pub async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    if req.path() == "/api/health" && req.method() == Method::Get {
        return Response::from_json(&tickets_core::Health::default());
    }
    if req.path() == "/api" || req.path().starts_with("/api/") {
        return Response::error("Not found", 404);
    }
    env.assets("ASSETS")?.fetch_request(req).await
}
