use crate::components::{AsciiSpinner, BusyButton};
use gloo_net::http::RequestBuilder;
use leptos::prelude::*;
use serde_json::{Value, json};
use wasm_bindgen_futures::spawn_local;

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
    view! { <main class="login-page"><section class="login"><div class="login-wordmark"><span class="workspace-switcher"><span class="workspace-mark">"T"</span><strong>"Tickets"</strong></span></div><h1>"Sign in"</h1><p>"Use your workspace credentials."</p>
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
#[component]
pub fn Tokens(#[prop(default = false)] fixture: bool) -> impl IntoView {
    #[derive(Clone, serde::Deserialize)]
    struct Token {
        id: i64,
        name: String,
        created_at: String,
        last_used_at: Option<String>,
        revoked_at: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct Created {
        token: String,
    }

    let (items, set_items) = signal(Vec::<Token>::new());
    let (name, set_name) = signal(String::new());
    let (created, set_created) = signal(String::new());
    let (message, set_message) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let revoking = RwSignal::new(None::<i64>);
    let (authorized, set_authorized) = signal(false);
    let refresh = move || {
        if fixture {
            set_authorized.set(true);
            return;
        }
        spawn_local(async move {
            match api("GET", "/api/v1/tokens", Value::Null).await {
                Ok(v) => match serde_json::from_value(v) {
                    Ok(items) => {
                        set_items.set(items);
                        set_authorized.set(true);
                    }
                    Err(e) => set_message.set(format!("Invalid token list: {e}")),
                },
                Err(e) => set_message.set(e),
            }
        })
    };
    refresh();
    view! { <h1>"API tokens"</h1>
    <section><div class="settings-panel"><div class="settings-intro"><h2>"Personal API tokens"</h2><p>"Give each agent its own token. Revoke it when access is no longer needed."</p></div>
    <form autocomplete="off" data-1p-ignore="true" data-form-type="other" on:submit=move |ev| { ev.prevent_default(); if busy.get_untracked() || fixture {return;} set_busy.set(true); set_created.set(String::new()); set_message.set(String::new()); spawn_local(async move {
        match api("POST","/api/v1/tokens",json!({"name":name.get_untracked()})).await { Ok(v)=>match serde_json::from_value::<Created>(v){Ok(v)=>{set_created.set(v.token);set_name.set(String::new());refresh();},Err(e)=>set_message.set(format!("Invalid token response: {e}"))},Err(e)=>set_message.set(e) } set_busy.set(false);
    }); }><div class="settings-fields"><label class="field" for="token-label"><span>"Token name"</span><input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  id="token-label" placeholder="Build agent" required maxlength="100" disabled=move||!authorized.get() prop:value=name on:input=move |e|set_name.set(event_target_value(&e))/></label></div><div class="settings-footer"><BusyButton busy disabled=Signal::derive(move||!authorized.get() || name.get().trim().is_empty())>"Create token"</BusyButton></div></form></div>
    <Show when=move ||!created.get().is_empty()><div class="secret"><h2>"Copy your token"</h2><p>"This token is shown only once. Copy it before leaving this page."</p><code>{created}</code><button class="button" on:click=move |_|spawn_local(async move { let promise=web_sys::window().unwrap().navigator().clipboard().write_text(&created.get_untracked()); set_message.set(if wasm_bindgen_futures::JsFuture::from(promise).await.is_ok(){"Token copied."}else{"Copy failed. Select the token and copy it manually."}.into()); })>"Copy token"</button><button class="button button-ghost" on:click=move |_|set_created.set(String::new())>"Dismiss"</button></div></Show>
    <p role="status">{message}</p><ul class="tokens">{move || items.get().into_iter().map(|item| {let id=item.id;let revoked=item.revoked_at.is_some(); view!{<li><div><strong>{item.name}</strong><p><span>"Created "<time datetime=item.created_at.clone() title=item.created_at.clone()>{token_date(&item.created_at)}</time></span><span>"Last used "{item.last_used_at.map(|date|view!{<time datetime=date.clone() title=date.clone()>{token_date(&date)}</time>}.into_any()).unwrap_or_else(||view!{<span>"Never"</span>}.into_any())}</span></p></div>{if revoked {view!{<span class="muted">"Revoked"</span>}.into_any()}else{view!{<BusyButton kind="button" variant="secondary" busy=Signal::derive(move||revoking.get()==Some(id)) disabled=Signal::derive(move||revoking.get().is_some()) status_label="Revoking token" on_click=Callback::new(move |_|{revoking.set(Some(id));spawn_local(async move { let result=api("DELETE",&format!("/api/v1/tokens/{id}"),Value::Null).await; revoking.set(None);match result {Ok(_)=>refresh(),Err(e)=>set_message.set(e)} });})>"Revoke"</BusyButton>}.into_any()}}</li>} }).collect_view()}</ul>

    <Show when=move||!authorized.get() && message.get().is_empty()><p class="loading-caption"><AsciiSpinner label="Loading tokens"/>"Loading tokens…"</p></Show>
    <Show when=move ||authorized.get() && items.get().is_empty() && message.get().is_empty()><p>"No API tokens yet."</p></Show></section> }
}

fn token_date(value: &str) -> String {
    value.split('T').next().unwrap_or(value).to_string()
}
