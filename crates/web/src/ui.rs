use gloo_net::http::RequestBuilder;
use leptos::prelude::*;
use serde_json::{Value, json};
use wasm_bindgen_futures::spawn_local;

async fn api(method: &str, path: &str, body: Value) -> Result<Value, String> {
    let builder = RequestBuilder::new(path).method(method.parse().unwrap());
    let request = if method == "GET" {
        builder.build().map_err(|_| "Request failed")?
    } else {
        builder.json(&body).map_err(|_| "Request failed")?
    };
    let response = request.send().await.map_err(|_| "Connection failed")?;
    if response.status() == 401 {
        return Err("Please sign in with your workspace credentials.".into());
    }
    let value: Value = response.json().await.map_err(|_| "Invalid response")?;
    if !response.ok() {
        return Err(value["error"].as_str().unwrap_or("Request failed").into());
    }
    Ok(value)
}
#[component]
pub fn Login() -> impl IntoView {
    let (username, set_username) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (error, set_error) = signal(String::new());
    let (busy, set_busy) = signal(false);
    view! { <section class="login"><h1>"Sign in to Tickets"</h1><p>"Use your workspace credentials."</p>
    <form on:submit=move |ev| { ev.prevent_default(); set_busy.set(true); set_error.set(String::new()); spawn_local(async move {
        let result=api("POST","/api/v1/login",json!({"username":username.get_untracked(),"password":password.get_untracked()})).await;
        set_password.set(String::new()); set_busy.set(false);
        match result { Ok(_)=> { let _=web_sys::window().unwrap().location().set_href("/settings/tokens"); }, Err(e)=>set_error.set(e) }
    }); }>
    <label for="username">"Username"</label><input id="username" autocomplete="username" required prop:value=username on:input=move |e|set_username.set(event_target_value(&e))/>
    <label for="password">"Password"</label><input id="password" type="password" autocomplete="current-password" required prop:value=password on:input=move |e|set_password.set(event_target_value(&e))/>
    <p role="alert">{error}</p><button type="submit" disabled=busy>{move ||if busy.get(){"Signing in…"}else{"Sign in"}}</button>
    </form></section> }
}
#[component]
pub fn Tokens() -> impl IntoView {
    let (items, set_items) = signal(Vec::<Value>::new());
    let (name, set_name) = signal(String::new());
    let (created, set_created) = signal(String::new());
    let (message, set_message) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let refresh = move || {
        spawn_local(async move {
            match api("GET", "/api/v1/tokens", Value::Null).await {
                Ok(v) => set_items.set(v.as_array().cloned().unwrap_or_default()),
                Err(e) => set_message.set(e),
            }
        })
    };
    refresh();
    view! { <header><h1>"API tokens"</h1><button on:click=move |_|spawn_local(async move { if api("POST","/api/v1/logout",Value::Null).await.is_ok(){let _=web_sys::window().unwrap().location().set_href("/login");} })>"Sign out"</button></header>
    <section><h2>"Personal API tokens"</h2><p>"Give each agent its own token. Revoke it when access is no longer needed."</p>
    <form on:submit=move |ev| { ev.prevent_default(); set_busy.set(true); set_created.set(String::new()); set_message.set(String::new()); spawn_local(async move {
        match api("POST","/api/v1/tokens",json!({"name":name.get_untracked()})).await { Ok(v)=>{set_created.set(v["token"].as_str().unwrap_or_default().into());set_name.set(String::new());refresh();},Err(e)=>set_message.set(e) } set_busy.set(false);
    }); }><label for="token-name">"Token name"</label><div class="inline"><input id="token-name" placeholder="e.g. Build agent" required maxlength="100" prop:value=name on:input=move |e|set_name.set(event_target_value(&e))/><button disabled=busy type="submit">"Create token"</button></div></form>
    <Show when=move ||!created.get().is_empty()><div class="secret"><h2>"Copy your token"</h2><p>"This token is shown only once. Store it somewhere safe before leaving this page."</p><code>{created}</code><button on:click=move |_|spawn_local(async move { let promise=web_sys::window().unwrap().navigator().clipboard().write_text(&created.get_untracked()); set_message.set(if wasm_bindgen_futures::JsFuture::from(promise).await.is_ok(){"Token copied."}else{"Copy failed. Select the token and copy it manually."}.into()); })>"Copy token"</button><button on:click=move |_|set_created.set(String::new())>"Dismiss"</button></div></Show>
    <p role="status">{message}</p><ul class="tokens">{move || items.get().into_iter().map(|item| {let id=item["id"].as_i64().unwrap();let revoked=!item["revoked_at"].is_null(); view!{<li><div><strong>{item["name"].as_str().unwrap_or_default().to_owned()}</strong><p>{format!("Created {} · Last used {}",item["created_at"].as_str().unwrap_or("—"),item["last_used_at"].as_str().unwrap_or("Never"))}</p></div>{if revoked {view!{<span>"Revoked"</span>}.into_any()}else{view!{<button on:click=move |_|spawn_local(async move { match api("DELETE",&format!("/api/v1/tokens/{id}"),Value::Null).await {Ok(_)=>refresh(),Err(e)=>set_message.set(e)} })>"Revoke"</button>}.into_any()}}</li>} }).collect_view()}</ul>
    <Show when=move ||items.get().is_empty()><p>"No API tokens yet."</p></Show></section> }
}
