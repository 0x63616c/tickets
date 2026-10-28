use crate::repository::Store;
use js_sys::{Array, Object, Reflect, Uint8Array};
use serde::Deserialize;
use serde_json::{Value, json};
use tickets_core::auth::*;
use wasm_bindgen_futures::JsFuture;
use worker::{
    wasm_bindgen::{JsCast, JsValue},
    *,
};

fn crypto() -> Result<web_sys::Crypto> {
    Ok(Reflect::get(&js_sys::global(), &"crypto".into())?.unchecked_into())
}
fn random() -> Result<String> {
    let mut bytes = [0; 32];
    crypto()?.get_random_values_with_u8_array(&mut bytes)?;
    Ok(hex(&bytes))
}
async fn derive(password: &str, salt: &[u8], iterations: u32) -> Result<Vec<u8>> {
    let subtle = crypto()?.subtle();
    let usages = Array::of1(&"deriveBits".into());
    let key = JsFuture::from(subtle.import_key_with_str(
        "raw",
        &Uint8Array::from(password.as_bytes()),
        "PBKDF2",
        false,
        &usages,
    )?)
    .await?;
    let algorithm = Object::new();
    for (k, v) in [
        ("name", JsValue::from_str("PBKDF2")),
        ("hash", "SHA-256".into()),
        ("iterations", iterations.into()),
        ("salt", Uint8Array::from(salt).into()),
    ] {
        Reflect::set(&algorithm, &k.into(), &v)?;
    }
    let bits =
        JsFuture::from(subtle.derive_bits_with_object(&algorithm, &key.unchecked_into(), 256)?)
            .await?;
    Ok(Uint8Array::new(&bits).to_vec())
}
async fn query(db: &D1Database, sql: &str, args: Vec<Value>) -> Result<Vec<Value>> {
    Store::query(db, sql, args)
        .await
        .map_err(|_| Error::RustError("Authentication storage failed".into()))
}
fn reply(value: Value, status: u16) -> Result<Response> {
    let value = if status >= 400 && value["error"].is_string() {
        let code = match status {
            400 => "bad_request",
            401 => "unauthorized",
            403 => "forbidden",
            404 => "not_found",
            429 => "rate_limited",
            _ => "internal",
        };
        json!({"error":{"code":code,"message":value["error"]}})
    } else {
        value
    };
    let mut r = Response::from_json(&value)?.with_status(status);
    r.headers_mut().set("Cache-Control", "no-store")?;
    Ok(r)
}
fn denied() -> Result<Response> {
    reply(json!({"error":"Unauthorized"}), 401)
}
fn same_origin(req: &Request) -> bool {
    let Ok(Some(origin)) = req.headers().get("Origin") else {
        return false;
    };
    req.url()
        .map(|u| u.origin().ascii_serialization() == origin)
        .unwrap_or(false)
}
fn cookie(req: &Request) -> Option<String> {
    req.headers()
        .get("Cookie")
        .ok()
        .flatten()?
        .split(';')
        .find_map(|p| p.trim().strip_prefix("tickets_session=").map(str::to_owned))
}
/// Common gate for every private API route. An invalid bearer never falls back to cookies.
pub async fn handle(mut req: Request, env: Env) -> Result<Response> {
    let path = req.path();
    let method = req.method();
    if path == "/api/health" && method == Method::Get {
        return Response::from_json(&tickets_core::Health::default());
    }
    let db = env.d1("DB")?;
    let changing = !matches!(method, Method::Get | Method::Head | Method::Options);
    let authorization = req.headers().get("Authorization")?;
    if path == "/api/v1/login" && method == Method::Post {
        if !same_origin(&req) {
            return reply(json!({"error":"Origin required"}), 403);
        }
        let ip = req
            .headers()
            .get("CF-Connecting-IP")?
            .unwrap_or_else(|| "local".into());
        query(
            &db,
            "DELETE FROM login_attempts WHERE window_start < unixepoch()-900",
            vec![],
        )
        .await?;
        let attempts=query(&db,"INSERT INTO login_attempts(ip_hash,window_start,attempts) VALUES(?,unixepoch(),1) ON CONFLICT(ip_hash) DO UPDATE SET attempts=attempts+1 RETURNING attempts",vec![hash_token(&ip).into()]).await?;
        if attempts[0]["attempts"].as_i64().unwrap_or(99) > 5 {
            return reply(
                json!({"error":"Too many attempts. Try again in 15 minutes."}),
                429,
            );
        }
        #[derive(Deserialize)]
        struct Login {
            username: String,
            password: String,
        }
        let input = match req.json::<Login>().await {
            Ok(v) if v.username.len() <= 128 && v.password.len() <= 1024 => v,
            _ => return denied(),
        };
        let username = env.secret("BOOTSTRAP_USERNAME")?.to_string();
        let secret = env.secret("BOOTSTRAP_PASSWORD_HASH")?.to_string();
        let (iterations, salt, expected) = password_parts(&secret)
            .ok_or_else(|| Error::RustError("Invalid bootstrap hash configuration".into()))?;
        let actual = derive(&input.password, &salt, iterations).await?;
        if !(constant_eq(&actual, &expected)
            & constant_eq(input.username.as_bytes(), username.as_bytes()))
        {
            return denied();
        }
        query(&db,"INSERT INTO users(id,username) VALUES(1,?) ON CONFLICT(id) DO UPDATE SET username=excluded.username",vec![username.into()]).await?;
        let token = random()?;
        query(
            &db,
            "DELETE FROM sessions WHERE expires_at<=strftime('%Y-%m-%dT%H:%M:%fZ','now')",
            vec![],
        )
        .await?;
        query(&db,"INSERT INTO sessions(token_hash,user_id,expires_at) VALUES(?,1,strftime('%Y-%m-%dT%H:%M:%fZ','now','+7 days'))",vec![hash_token(&token).into()]).await?;
        let mut response = reply(json!({"ok":true}), 200)?;
        response.headers_mut().set(
            "Set-Cookie",
            &format!(
                "tickets_session={token}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=604800"
            ),
        )?;
        return Ok(response);
    }
    let bearer = authorization.is_some();
    let token = if let Some(value) = authorization {
        value
            .strip_prefix("Bearer ")
            .filter(|t| valid_token(t, true))
            .map(str::to_owned)
    } else {
        cookie(&req).filter(|t| valid_token(t, false))
    };
    let Some(token) = token else {
        return denied();
    };
    let hash = hash_token(&token);
    let sql = if bearer {
        crate::auth_queries::BEARER
    } else {
        crate::auth_queries::SESSION
    };
    if query(&db, sql, vec![hash.clone().into()]).await?.is_empty() {
        return denied();
    }
    if changing && !bearer && !same_origin(&req) {
        return reply(json!({"error":"Origin required"}), 403);
    }
    if method == Method::Get
        && let Some(project) = path.strip_prefix("/api/v1/live/")
    {
        let Ok(project) = tickets_core::api::positive_id(project) else {
            return reply(json!({"error":"Invalid project id"}), 400);
        };
        if !bearer && !same_origin(&req) {
            return reply(json!({"error":"Origin required"}), 403);
        }
        return crate::live::upgrade(req, &env, &db, project).await;
    }
    match (method,path.as_str()) {
        (Method::Get,"/api/v1/me")=>reply(json!({"authenticated":true}),200),
        (Method::Post,"/api/v1/logout")=> {
            if !bearer { query(&db,"DELETE FROM sessions WHERE token_hash=?",vec![hash.into()]).await?; }
            let mut r=reply(json!({"ok":true}),200)?;
            r.headers_mut().set("Set-Cookie","tickets_session=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0")?; Ok(r)
        },
        (Method::Get,"/api/v1/tokens")=>reply(json!(query(&db,"SELECT id,name,created_at,last_used_at,revoked_at FROM api_tokens WHERE user_id=1 ORDER BY id DESC",vec![]).await?),200),
        (Method::Post,"/api/v1/tokens")=> {
            #[derive(Deserialize)] struct Create { name:String }
            let input=match req.json::<Create>().await { Ok(v) if !v.name.trim().is_empty() && v.name.len()<=100 =>v, _=>return reply(json!({"error":"Name must contain 1–100 bytes"}),400) };
            let token=format!("tk_{}",random()?);
            let rows=query(&db,"INSERT INTO api_tokens(user_id,token_hash,name) VALUES(1,?,?) RETURNING id,name,created_at,last_used_at,revoked_at",vec![hash_token(&token).into(),input.name.trim().into()]).await?;
            reply(json!({"token":token,"metadata":rows[0]}),201)
        },
        (Method::Delete,_) if path.starts_with("/api/v1/tokens/")=> {
            let Ok(id)=path.trim_start_matches("/api/v1/tokens/").parse::<i64>() else { return reply(json!({"error":"Invalid token id"}),400); };
            let rows=query(&db,"UPDATE api_tokens SET revoked_at=coalesce(revoked_at,strftime('%Y-%m-%dT%H:%M:%fZ','now')) WHERE id=? AND user_id=1 RETURNING id",vec![id.into()]).await?;
            if rows.is_empty() { reply(json!({"error":"Token not found"}),404) } else { reply(json!({"ok":true}),200) }
        },
        _=> {
            let url=req.url()?;
            let query=url.query_pairs().map(|(k,v)|(k.into_owned(),v.into_owned())).collect::<Vec<_>>();
            let body=if changing {
                match req.text().await {
                    Ok(text) if text.is_empty()=>Value::Null,
                    Ok(text)=>match serde_json::from_str(&text) {Ok(v)=>v,Err(_)=>return reply(json!({"error":"Malformed JSON"}),400)},
                    Err(_)=>return reply(json!({"error":"Unable to read body"}),400),
                }
            } else {Value::Null};
            let response=crate::api::handle(&db,req.method().as_ref(),&path,&query,body).await;
            if let Some(change) = response.change {
                // Best effort after D1 commit: notification cannot roll back or fail a write.
                if crate::live::notify(&env, &db, change).await.is_err() { console_error!("Live notification failed"); }
            }
            reply(response.body,response.status)
        }
    }
}
