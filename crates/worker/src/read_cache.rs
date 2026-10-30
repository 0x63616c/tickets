//! Private, post-auth response cache. Keys never contain credentials. The workspace
//! version is advanced transactionally by additive SQL triggers (including old writers).
use crate::repository::Store;
use serde_json::Value;
use worker::*;

pub struct ReadCache {
    key: String,
    etag: String,
    version: String,
}
impl ReadCache {
    pub async fn lookup(req: &Request, db: &D1Database) -> Result<Option<Self>> {
        if req.method() != Method::Get
            || !matches!(
                req.path().as_str(),
                "/api/v1/tickets"
                    | "/api/v1/projects"
                    | "/api/v1/labels"
                    | "/api/v1/analytics/home"
                    | "/api/v1/search"
            )
        {
            return Ok(None);
        }
        let rows: Vec<Value> = Store::query(
            db,
            "SELECT version FROM workspace_version WHERE id=1",
            vec![],
        )
        .await
        .map_err(|e| Error::RustError(e.to_string()))?;
        // Analytics has a clock-dependent response, even without any writes.
        let clock = if req.path() == "/api/v1/analytics/home" {
            Date::now().as_millis() / 60_000
        } else {
            0
        };
        let digest = tickets_core::auth::hash_token(&format!(
            "read-v1:{}:{}:{clock}",
            req.url()?,
            rows[0]["version"]
        ));
        let etag = format!("W/\"{digest}\"");
        let mut url = req.url()?;
        url.set_path(&format!("/__private_read_cache/{digest}"));
        url.set_query(None);
        Ok(Some(Self {
            key: url.to_string(),
            etag,
            version: rows[0]["version"].to_string(),
        }))
    }
    pub async fn response(&self, req: &Request) -> Result<Option<Response>> {
        match Cache::default().get(self.key.as_str(), false).await {
            Ok(Some(response)) => {
                if req.headers().get("If-None-Match")?.is_some_and(|v| {
                    v.split(',')
                        .any(|tag| tag.trim() == self.etag || tag.trim() == "*")
                }) {
                    return Ok(Some(self.private(Response::empty()?.with_status(304))?));
                }
                Ok(Some(self.private(response)?))
            }
            // Cache availability must never fail an authoritative read.
            _ => Ok(None),
        }
    }
    fn private(&self, response: Response) -> Result<Response> {
        // Cache API responses can have immutable headers.
        let headers = Headers::new();
        for (key, value) in response.headers().entries() {
            headers.set(&key, &value)?;
        }
        headers.set("Cache-Control", "private, no-cache")?;
        headers.set("ETag", &self.etag)?;
        headers.set("X-Workspace-Version", &self.version)?;
        Ok(response.with_headers(headers))
    }
    pub async fn save(&self, mut response: Response) -> Result<Response> {
        if response.status_code() == 200 {
            let headers = Headers::new();
            headers.set("Content-Type", "application/json")?;
            headers.set("Cache-Control", "public, max-age=60")?;
            if let Ok(copy) = response.cloned() {
                let _ = Cache::default()
                    .put(self.key.as_str(), copy.with_headers(headers))
                    .await;
            }
        }
        if response.status_code() == 200 {
            self.private(response)
        } else {
            Ok(response)
        }
    }
}
