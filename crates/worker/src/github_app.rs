//! Phase 1 manifest callback. No session is required; setup state authorizes registration.
use crate::github_app_registration as storage;
use futures_util::StreamExt;
use js_sys::{Array, Object, Reflect, Uint8Array};
use tickets_core::github_app::{self as app, AppCredentials, CallbackError};
use wasm_bindgen_futures::JsFuture;
use worker::{wasm_bindgen::JsCast, *};

fn crypto() -> Result<web_sys::Crypto> {
    Ok(Reflect::get(&js_sys::global(), &"crypto".into())?.unchecked_into())
}

async fn import_key(key: &[u8; 32], usage: &str) -> Result<web_sys::CryptoKey> {
    let key = JsFuture::from(crypto()?.subtle().import_key_with_str(
        "raw",
        &Uint8Array::from(key.as_slice()),
        "AES-GCM",
        false,
        &Array::of1(&usage.into()),
    )?)
    .await?;
    Ok(key.unchecked_into())
}

fn algorithm(iv: &[u8; app::IV_BYTES]) -> Result<Object> {
    let algorithm = Object::new();
    Reflect::set(&algorithm, &"name".into(), &"AES-GCM".into())?;
    Reflect::set(&algorithm, &"iv".into(), &Uint8Array::from(iv.as_slice()))?;
    Reflect::set(&algorithm, &"tagLength".into(), &128.into())?;
    Ok(algorithm)
}

fn credential_error() -> Error {
    Error::RustError("GitHub App credentials unavailable".into())
}

async fn encrypt(key: &[u8; 32], secrets: &app::AppSecrets) -> Result<String> {
    let mut iv = [0; app::IV_BYTES];
    crypto()?.get_random_values_with_u8_array(&mut iv)?;
    let key = import_key(key, "encrypt").await?;
    let plaintext = serde_json::to_vec(secrets)?;
    let encrypted = JsFuture::from(crypto()?.subtle().encrypt_with_object_and_u8_array(
        &algorithm(&iv)?,
        &key,
        &plaintext,
    )?)
    .await?;
    app::pack_ciphertext(&iv, &Uint8Array::new(&encrypted).to_vec()).ok_or_else(credential_error)
}

async fn decrypt(key: &[u8; 32], ciphertext: &str) -> Result<app::AppSecrets> {
    let (iv, ciphertext) = app::unpack_ciphertext(ciphertext).ok_or_else(credential_error)?;
    let key = import_key(key, "decrypt").await?;
    let plaintext = JsFuture::from(crypto()?.subtle().decrypt_with_object_and_u8_array(
        &algorithm(&iv)?,
        &key,
        &ciphertext,
    )?)
    .await?;
    app::parse_secrets(&Uint8Array::new(&plaintext).to_vec()).ok_or_else(credential_error)
}

/// Phase 2 can load credentials without the setup state. No registration yields None;
/// missing/wrong keys, corrupt rows and failed authentication tags return generic errors.
pub async fn load_credentials(env: &Env) -> Result<Option<AppCredentials>> {
    let load = async {
        let Some(stored) = storage::load(&env.d1("DB")?)
            .await
            .map_err(|_| credential_error())?
        else {
            return Ok(None);
        };
        let key = app::store_key(&env.secret("GITHUB_APP_STORE_KEY")?.to_string())
            .ok_or_else(credential_error)?;
        Ok(Some(AppCredentials {
            app: stored.app,
            secrets: decrypt(&key, &stored.secrets_ciphertext).await?,
        }))
    };
    load.await.map_err(|_: Error| credential_error())
}

async fn exchange(code: &str) -> std::result::Result<AppCredentials, CallbackError> {
    let exchange = async {
        let mut init = RequestInit::new();
        init.with_method(Method::Post)
            .with_redirect(RequestRedirect::Manual);
        init.headers.set("Accept", "application/vnd.github+json")?;
        init.headers.set("X-GitHub-Api-Version", "2022-11-28")?;
        init.headers.set("User-Agent", "tickets-worldwidewebb")?;
        let request = Request::new_with_init(
            &format!("https://api.github.com/app-manifests/{code}/conversions"),
            &init,
        )?;
        let mut response = Fetch::Request(request).send().await?;
        if response.status_code() != 201 {
            // Do not read GitHub's error body or follow redirects containing the code.
            return Ok(Err(CallbackError::ExchangeFailed(Some(
                response.status_code(),
            ))));
        }
        let mut bytes = Vec::new();
        let mut stream = response.stream()?;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if chunk.len() > (64 * 1024_usize).saturating_sub(bytes.len()) {
                return Ok(Err(CallbackError::ExchangeFailed(None)));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(app::parse_conversion(&bytes).ok_or(CallbackError::ExchangeFailed(None)))
    };
    exchange
        .await
        .map_err(|_: Error| CallbackError::ExchangeFailed(None))?
}

async fn register(req: &Request, env: &Env) -> std::result::Result<String, CallbackError> {
    if req.method() != Method::Get {
        return Err(CallbackError::MethodNotAllowed);
    }
    let setup_state = env
        .secret("GITHUB_APP_SETUP_STATE")
        .ok()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .ok_or(CallbackError::Unavailable)?;
    let store_key = env
        .secret("GITHUB_APP_STORE_KEY")
        .ok()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .ok_or(CallbackError::Unavailable)?;
    let url = req.url().map_err(|_| CallbackError::BadCode)?;
    // Decode once; reject ambiguous duplicate parameters instead of choosing a value.
    let parameter = |name| {
        let mut values = url.query_pairs().filter(|(key, _)| key == name);
        let value = values.next().map(|(_, value)| value.into_owned());
        if values.next().is_some() { None } else { value }
    };
    let state = parameter("state").unwrap_or_default();
    let code = parameter("code").unwrap_or_default();
    let db = env.d1("DB").map_err(|_| CallbackError::Internal)?;
    let key = storage::preflight(&db, Some(&setup_state), Some(&store_key), &state, &code).await?;
    let credentials = exchange(&code).await?;
    let ciphertext = encrypt(&key, &credentials.secrets)
        .await
        .map_err(|_| CallbackError::Internal)?;
    storage::register(&db, &credentials.app, &ciphertext).await?;
    Ok(app::registration_html(&credentials.app))
}

pub async fn manifest_callback(req: Request, env: &Env) -> Result<Response> {
    // Keep runtime errors, including JS exceptions and fetch URLs, out of responses/logs.
    let (html, status) = match register(&req, env).await {
        Ok(html) => (html, 200),
        Err(error) => (app::error_html(&error), error.status()),
    };
    let mut response = Response::from_html(html)?.with_status(status);
    response.headers_mut().set("Cache-Control", "no-store")?;
    response
        .headers_mut()
        .set("Referrer-Policy", "no-referrer")?;
    response
        .headers_mut()
        .set("Content-Security-Policy", &app::content_security_policy())?;
    response
        .headers_mut()
        .set("X-Content-Type-Options", "nosniff")?;
    if status == 405 {
        response.headers_mut().set("Allow", "GET")?;
    }
    Ok(response)
}
