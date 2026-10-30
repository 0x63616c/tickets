use crate::components::BusyButton;
use gloo_net::http::RequestBuilder;
use leptos::prelude::*;
use serde_json::{Value, json};
use wasm_bindgen_futures::spawn_local;

/// GitHub branding belongs only on HTTP(S) links to the exact GitHub host.
pub(crate) fn is_github_url(value: &str) -> bool {
    web_sys::Url::new(value).is_ok_and(|url| {
        matches!(url.protocol().as_str(), "https:" | "http:")
            && url.hostname().eq_ignore_ascii_case("github.com")
    })
}

pub(crate) async fn api(method: &str, path: &str, body: Value) -> Result<Value, String> {
    api_typed(method, path, body)
        .await
        .map_err(|error| error.to_string())
}
async fn api_typed(method: &str, path: &str, body: Value) -> Result<Value, crate::login::ApiError> {
    use crate::login::ApiError;
    let builder =
        RequestBuilder::new(path).method(method.parse().map_err(|_| ApiError::InvalidResponse)?);
    let request = if method == "GET" {
        builder.build().map_err(|_| ApiError::InvalidResponse)?
    } else {
        builder.json(&body).map_err(|_| ApiError::InvalidResponse)?
    };
    let response = request.send().await.map_err(|_| ApiError::Network)?;
    let status = response.status();
    if status == 401 {
        let message = if path == "/api/v1/login" {
            "Incorrect username or password."
        } else {
            redirect_to_login();
            "Your session expired. Redirecting to sign in…"
        };
        return Err(ApiError::Http {
            status,
            message: message.into(),
        });
    }
    if !response.ok() {
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum ErrorBody {
            Message { message: String },
            Text(String),
        }
        #[derive(serde::Deserialize)]
        struct Envelope {
            error: ErrorBody,
        }
        let message = match response.json::<Envelope>().await {
            Ok(Envelope {
                error: ErrorBody::Message { message } | ErrorBody::Text(message),
            }) => message,
            Err(_) => format!("Request failed (HTTP {status})."),
        };
        return Err(ApiError::Http { status, message });
    }
    response.json().await.map_err(|_| ApiError::InvalidResponse)
}
pub(crate) fn redirect_to_login() {
    let location = window().location();
    let current = format!(
        "{}{}",
        location.pathname().unwrap_or_default(),
        location.search().unwrap_or_default()
    );
    let encoded = js_sys::encode_uri_component(tickets_core::navigation::safe_next(&current));
    let _ = location.set_href(&format!("/login?next={encoded}"));
}
fn login_next() -> String {
    let query = leptos_router::hooks::use_query_map();
    tickets_core::navigation::safe_next(&query.get_untracked().get("next").unwrap_or_default())
        .to_owned()
}
#[component]
pub fn Login() -> impl IntoView {
    let (username, set_username) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (error, set_error) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let next = login_next();
    view! { <main class="login-page"><section class="login"><div class="login-wordmark"><span class="workspace-switcher"><crate::components::BrandMark/><strong>"Tickets"</strong></span></div><h1>"Sign in"</h1><p>"Use your workspace credentials."</p>
    <form method="post" action="/api/v1/login" aria-busy=move||busy.get().to_string() on:submit=move |ev| { ev.prevent_default(); if busy.get_untracked() { return; } let next = next.clone(); set_busy.set(true); set_error.set(String::new()); spawn_local(async move {
        let result=api_typed("POST","/api/v1/login",json!({"username":username.get_untracked(),"password":password.get_untracked()})).await;
        if crate::login::clear_password(result.as_ref().map(|_| ())) { set_password.set(String::new()); } set_busy.set(false);
        match result { Ok(_)=> { let _=web_sys::window().unwrap().location().set_href(tickets_core::navigation::safe_next(&next)); }, Err(e)=>set_error.set(e.login_message()) }
    }); }>
    <label class="field" for="username"><span>"Username"</span><input id="username" name="username" autofocus autocomplete="username" required prop:value=username on:input=move |e|{set_username.set(event_target_value(&e));set_error.set(String::new());}/></label>
    <label class="field" for="password"><span>"Password"</span><input id="password" name="password" aria-describedby="login-error" aria-invalid=move||error.get().starts_with("Incorrect") type="password" autocomplete="current-password" required prop:value=password on:input=move |e|{set_password.set(event_target_value(&e));set_error.set(String::new());}/></label>
    <p id="login-error" class="login-error" role="alert">{error}</p><BusyButton busy status_label="Signing in">"Sign in"</BusyButton>
    </form></section></main> }
}
mod tokens;
pub use tokens::Tokens;
