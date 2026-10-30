use super::api;
use crate::components::*;
use leptos::{ev, prelude::*};
use serde_json::{Value, json};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{JsFuture, spawn_local};

fn focus_create() {
    request_animation_frame(move || {
        if let Ok(Some(button)) = document().query_selector(".token-create")
            && let Some(button) = button.dyn_ref::<web_sys::HtmlElement>()
        {
            let _ = button.focus();
        }
    });
}

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
    metadata: Token,
}
fn date(value: &str) -> String {
    value.split('T').next().unwrap_or(value).to_string()
}

#[component]
pub fn Tokens(#[prop(default = false)] fixture: bool) -> impl IntoView {
    let items = RwSignal::new(Vec::<Token>::new());
    let loaded = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let message = RwSignal::new(String::new());
    let creating = RwSignal::new(false);
    let selected = RwSignal::new(None::<Token>);
    let history = RwSignal::new(false);
    let refresh = Callback::new(move |()| {
        error.set(String::new());
        spawn_local(async move {
            let result = if fixture {
                Ok(
                    if window()
                        .location()
                        .search()
                        .unwrap_or_default()
                        .contains("empty=true")
                    {
                        vec![]
                    } else {
                        vec![
                            Token {
                                id: 4,
                                name: "Fixture · Build agent".into(),
                                created_at: "2026-10-28T10:00:00.000Z".into(),
                                last_used_at: Some("2026-10-29T12:00:00.000Z".into()),
                                revoked_at: None,
                            },
                            Token {
                                id: 3,
                                name: "Fixture · Release automation".into(),
                                created_at: "2026-10-21T10:00:00.000Z".into(),
                                last_used_at: Some("2026-10-29T10:00:00.000Z".into()),
                                revoked_at: None,
                            },
                            Token {
                                id: 2,
                                name: "Fixture · Local CLI".into(),
                                created_at: "2026-10-20T10:00:00.000Z".into(),
                                last_used_at: None,
                                revoked_at: None,
                            },
                            Token {
                                id: 1,
                                name: "Fixture · Retired import".into(),
                                created_at: "2026-09-04T10:00:00.000Z".into(),
                                last_used_at: None,
                                revoked_at: Some("2026-10-01T10:00:00.000Z".into()),
                            },
                        ]
                    },
                )
            } else {
                api("GET", "/api/v1/tokens", Value::Null)
                    .await
                    .and_then(|v| {
                        serde_json::from_value(v)
                            .map_err(|_| "Could not read tokens. Retry loading the list.".into())
                    })
            };
            if items.is_disposed() {
                return;
            }
            match result {
                Ok(value) => {
                    items.set(value);
                    loaded.set(true);
                }
                Err(e) => error.set(e),
            }
        });
    });
    refresh.run(());
    let open_create = move |_| {
        message.set(String::new());
        creating.set(true);
    };
    let reveal_root = NodeRef::<leptos::html::Section>::new();
    use_ascii_reveal(reveal_root, Signal::derive(move || !loaded.get()), true);
    view! {
        <section node_ref=reveal_root class="token-inventory">
            <div class="token-heading"><h1>"API tokens"</h1><Show when=move||items.get().iter().any(|t|history.get() || t.revoked_at.is_none())><button class="button button-primary token-create" disabled=move||!loaded.get() on:click=open_create><Icon name="plus"/>"Create token"</button></Show></div>
            <p class="settings-description">"Connect agents and scripts to your workspace. Give each one its own token."</p>
            <div class="token-list-heading"><h2>{move||format!("{} ({})",if history.get(){"All tokens"}else{"Active tokens"},items.get().iter().filter(|t|history.get() || t.revoked_at.is_none()).count())}</h2><label class="token-history"><input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other" type="checkbox" prop:checked=move||history.get() on:change=move|e|history.set(event_target_checked(&e))/>"Show revoked"</label></div>
            <p class="settings-error" role="alert">{error}</p>
            <Show when=move||!error.get().is_empty()><button class="button" on:click=move|_|refresh.run(())>"Retry loading tokens"</button></Show>
            <Show when=move||!loaded.get() && error.get().is_empty()><p class="loading-caption" role="status"><LoadingIndicator label="Loading tokens"/>"Loading tokens…"</p></Show>
            <ul class="tokens token-list">{move||items.get().into_iter().filter(|t|history.get() || t.revoked_at.is_none()).map(|item|{
                let revoked=item.revoked_at.is_some();
                let action=item.clone();
                let used=item.last_used_at.as_deref().map(|v|format!("Last used {}",date(v))).unwrap_or_else(||"Never used".into());
                let used_label=item.last_used_at.as_deref().map(|v|format!("Last authenticated {v}")).unwrap_or_else(||"Never used".into());
                view!{<li><div class="token-identity"><strong>{item.name.clone()}</strong><div class="token-meta"><span class="mono">{format!("Token #{}",item.id)}</span><span>"Created "<time datetime=item.created_at.clone() aria-label=item.created_at.clone() title=item.created_at.clone()>{date(&item.created_at)}</time></span><span aria-label=used_label.clone() title=used_label>{used}</span></div>{item.revoked_at.map(|v|view!{<p class="token-meta">"Revoked "<time datetime=v.clone()>{date(&v)}</time></p>})}</div>{if revoked {view!{<span class="token-revoked">"Revoked"</span>}.into_any()} else {view!{<button class="button button-ghost" aria-label=format!("Revoke {} (Token #{})",item.name,item.id) on:click=move|_|{message.set(String::new());selected.set(Some(action.clone()));}>"Revoke"</button>}.into_any()}}</li>}
            }).collect_view()}</ul>
            <Show when=move||loaded.get() && !items.get().iter().any(|t|history.get() || t.revoked_at.is_none())>
                <div class="token-empty"><Icon name="code"/><h2>"No active API tokens"</h2><p>"Create a token for an agent, a CLI or an automation. You’ll copy it once, then store it somewhere safe."</p><button class="button button-primary token-create" on:click=open_create>{move||if items.get().is_empty(){"Create your first token"}else{"Create token"}}</button></div>
            </Show>
            <p class="token-result" role="status">{message}</p>
            <p class="field-hint token-help">"Replacing a token? Create a new one, update your agent, then revoke the old token."</p>
        </section>
        <Show when=move||creating.get()><CreateToken fixture items on_done=Callback::new(move|()|{creating.set(false);focus_create();})/></Show>
        {move||selected.get().map(|token|view!{<RevokeToken fixture token items on_done=Callback::new(move|result:Option<String>|{selected.set(None);if let Some(result)=result {message.set(result);focus_create();}})/>})}
    }
}

#[component]
fn CreateToken(fixture: bool, items: RwSignal<Vec<Token>>, on_done: Callback<()>) -> impl IntoView {
    let name = RwSignal::new(String::new());
    let secret = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let copy_status = RwSignal::new(String::new());
    let copy_busy = RwSignal::new(false);
    let saved = RwSignal::new(false);
    let discarding = RwSignal::new(false);
    let identity = RwSignal::new(String::new());
    let copy = NodeRef::<leptos::html::Button>::new();
    Effect::new(move |_| {
        if !secret.get().is_empty() {
            request_animation_frame(move || {
                if let Some(button) = copy.get() {
                    let _ = button.focus();
                }
            });
        }
    });
    let unload = window_event_listener(ev::beforeunload, move |event| {
        if !secret.get_untracked().is_empty() || busy.get_untracked() {
            event.prevent_default();
            let _ = js_sys::Reflect::set(event.as_ref(), &"returnValue".into(), &"".into());
        }
    });
    on_cleanup(move || unload.remove());
    view! {
            <Modal title="Create API token" locked=Signal::derive(move||busy.get() || !secret.get().is_empty()) on_close=on_done>
                <Show when=move||secret.get().is_empty() fallback=move||view!{
                    <div class="modal-body secret token-secret"><h3>"Save your token"</h3><p class="token-secret-identity">{identity}</p><p id="token-once">"This token is shown only once. Store it securely before continuing."</p><textarea class="mono" aria-label="New API token" aria-describedby="token-once" readonly rows="3" autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other" prop:value=secret/><button node_ref=copy class="button" aria-describedby="token-once token-copy-status" disabled=move||copy_busy.get() on:click=move|_|{if copy_busy.get_untracked(){return;}copy_busy.set(true);spawn_local(async move {
                        let result=JsFuture::from(window().navigator().clipboard().write_text(&secret.get_untracked())).await;
                        if copy_status.is_disposed(){return;}copy_busy.set(false);copy_status.set(if result.is_ok(){"Token copied."}else{"Copy failed. Select the token above and copy it manually."}.into());
                    });}><Icon name="copy"/>"Copy token"</button><p id="token-copy-status" class="field-hint" role="status">{copy_status}</p><Show when=move||!discarding.get()><label class="token-saved"><input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other" type="checkbox" prop:checked=move||saved.get() on:change=move|e|saved.set(event_target_checked(&e))/>"I’ve saved this token somewhere safe"</label></Show>
                <Show when=move||discarding.get()><div class="token-discard"><h3>"Close without saving?"</h3><p>"You won’t be able to view this token again. It will stay active until you revoke it from the list."</p></div></Show></div>
                    <div class="modal-footer"><Show when=move||!discarding.get() fallback=move||view!{<button class="button button-ghost token-keep" on:click=move|_|discarding.set(false)>"Keep token visible"</button><button class="button button-secondary token-discard-confirm" on:click=move|_|{secret.set(String::new());on_done.run(());}>"Close anyway"</button>}><button class="button button-ghost token-discard-trigger" on:click=move|_|discarding.set(true)>"Close without saving…"</button><button class="button button-primary token-acknowledge" disabled=move||!saved.get() on:click=move|_|{secret.set(String::new());on_done.run(());}>"I’ve saved it"</button></Show></div>
                }>
                    <form autocomplete="off" data-1p-ignore="true" data-form-type="other" on:submit=move|ev|{ev.prevent_default();if busy.get_untracked(){return;}
    if name.get_untracked().len()>100 {error.set("Use 100 bytes or fewer (some characters use more than one byte).".into());return;}
    if name.get_untracked().trim().is_empty(){return;}busy.set(true);error.set(String::new());spawn_local(async move {
                        let result=if fixture {
                            Ok(Created {token:"tk_FIXTURE_ONLY_NOT_A_VALID_TOKEN_0000000000000000000000000000000000".into(),metadata:Token{id:items.get_untracked().iter().map(|t|t.id).max().unwrap_or(0)+1,name:name.get_untracked().trim().into(),created_at:"2026-10-29T12:00:00.000Z".into(),last_used_at:None,revoked_at:None}})
                        } else {api("POST","/api/v1/tokens",json!({"name":name.get_untracked().trim()})).await.and_then(|v|serde_json::from_value::<Created>(v).map_err(|_|"The token response could not be read.".into()))};
                        if secret.is_disposed(){return;}busy.set(false);match result {Ok(value)=>{identity.set(format!("{} · Token #{}",value.metadata.name,value.metadata.id));items.update(|items|items.insert(0,value.metadata));secret.set(value.token);},Err(e)=>error.set(format!("{e} If the request reached the server, a token may have been created. Close this dialog and reload the list before trying again."))}
                    });}>
                        <div class="modal-body"><label class="field" for="token-label"><span>"Token name"</span><input id="token-label" autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other" placeholder="Build agent" required maxlength="100" disabled=move||busy.get() aria-describedby="token-name-hint token-create-error" aria-invalid=move||(!error.get().is_empty()).to_string() prop:value=name on:input=move|e|{name.set(event_target_value(&e));error.set(String::new());}/></label><p id="token-name-hint" class="field-hint">"Use a name that tells you which agent or script uses this token."</p><div class="token-grant"><h3>"Full workspace access"</h3><p>"Can read and change all workspace data, including API tokens. Active until you revoke it."</p></div><p id="token-create-error" class="settings-error" role="alert">{error}</p></div>
                        <div class="modal-footer settings-footer"><button type="button" class="button button-ghost" disabled=move||busy.get() on:click=move|_|on_done.run(())>"Cancel"</button><BusyButton busy status_label="Creating token" disabled=Signal::derive(move||name.get().trim().is_empty())>"Create token"</BusyButton></div>
                    </form>
                </Show>
            </Modal>
        }
}

#[component]
fn RevokeToken(
    fixture: bool,
    token: Token,
    items: RwSignal<Vec<Token>>,
    on_done: Callback<Option<String>>,
) -> impl IntoView {
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let id = token.id;
    let name = token.name.clone();
    view! {
        <Modal title="Revoke API token?" locked=busy.into() on_close=Callback::new(move|()|on_done.run(None))>
            <div class="modal-body token-revoke"><strong>{token.name}</strong><p class="mono muted">{format!("Token #{id}")}</p><p>"New API requests using this token will fail. Update any agents or scripts that use it before revoking. This can’t be undone."</p><p class="settings-error" role="alert">{error}</p></div>
            <div class="modal-footer"><button class="button button-ghost" autofocus disabled=move||busy.get() on:click=move|_|on_done.run(None)>"Cancel"</button><BusyButton kind="button" variant="danger" busy status_label="Revoking token" on_click=Callback::new(move|()|{if busy.get_untracked(){return;}busy.set(true);error.set(String::new());let name=name.clone();spawn_local(async move {
                let result=if fixture {Ok(Value::Null)}else{api("DELETE",&format!("/api/v1/tokens/{id}"),Value::Null).await};
                if busy.is_disposed(){return;}busy.set(false);match result {Ok(_)=>{items.update(|items|{if let Some(token)=items.iter_mut().find(|t|t.id==id){token.revoked_at=Some(js_sys::Date::new_0().to_iso_string().into());}});on_done.run(Some(format!("Revoked {name} (Token #{id}).")));},Err(e)=>error.set(e)}
            });})>"Revoke token"</BusyButton></div>
        </Modal>
    }
}
