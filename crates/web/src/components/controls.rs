use super::Icon;
use leptos::{ev, html, prelude::*};
use web_sys::wasm_bindgen::JsCast;

/// A disclosure of native buttons: Tab/Enter, Escape, and outside-click dismissal.
#[component]
pub fn Menu(label: &'static str, children: Children) -> impl IntoView {
    let menu = NodeRef::<html::Details>::new();
    let handle = window_event_listener(ev::click, move |event| {
        if let Some(menu) = menu.get_untracked()
            && let Some(target) = event
                .target()
                .and_then(|target| target.dyn_into::<web_sys::Node>().ok())
            && !menu.contains(Some(&target))
        {
            menu.set_open(false);
        }
    });
    on_cleanup(move || handle.remove());
    view! {
        <details node_ref=menu class="label-menu" on:keydown=move |event: web_sys::KeyboardEvent| {
            if event.key() == "Escape" {
                event.prevent_default();
                if let Some(menu) = menu.get_untracked() {
                    menu.set_open(false);
                    if let Ok(Some(summary)) = menu.query_selector("summary")
                        && let Some(summary) = summary.dyn_ref::<web_sys::HtmlElement>() { let _ = summary.focus(); }
                }
            }
        }><summary><super::Icon name="plus"/>{label}</summary><div class="menu">{children()}</div></details>
    }
}

#[component]
pub fn Button(
    children: Children,
    #[prop(default = "secondary")] variant: &'static str,
    #[prop(optional)] on_click: Option<Callback<()>>,
    #[prop(default = false)] disabled: bool,
) -> impl IntoView {
    view! { <button type="button" class=format!("button button-{variant}") disabled=disabled on:click=move |_| { if let Some(callback) = on_click { callback.run(()); } }>{children()}</button> }
}

#[component]
pub fn IconButton(
    label: &'static str,
    icon: &'static str,
    on_click: Callback<()>,
) -> impl IntoView {
    view! { <button type="button" class="icon-button" aria-label=label title=label on:click=move |_| on_click.run(())><Icon name=icon/></button> }
}

#[component]
pub fn Shortcut(children: Children) -> impl IntoView {
    view! { <kbd>{children()}</kbd> }
}

#[component]
pub fn TextInput(
    id: &'static str,
    label: &'static str,
    #[prop(default = "")] placeholder: &'static str,
    value: RwSignal<String>,
) -> impl IntoView {
    view! { <label class="field" for=id><span>{label}</span><input id=id placeholder=placeholder prop:value=move || value.get() on:input=move |ev| value.set(event_target_value(&ev))/></label> }
}

/// Native select keeps keyboard, touch and assistive-technology behavior consistent.
#[component]
pub fn Select(
    label: &'static str,
    options: Vec<&'static str>,
    value: RwSignal<String>,
    #[prop(optional)] on_change: Option<Callback<String>>,
) -> impl IntoView {
    view! { <select aria-label=label prop:value=move || value.get() on:change=move |ev| { let next = event_target_value(&ev); value.set(next.clone()); if let Some(callback) = on_change { callback.run(next); } }>{options.into_iter().map(|option| view! { <option value=option selected=move || value.get() == option>{option}</option> }).collect_view()}</select> }
}

#[component]
pub fn LabelChip(#[prop(into)] name: String) -> impl IntoView {
    let class = match name.as_str() {
        "Feature" => "feature",
        "Bug" => "bug",
        "Infrastructure" => "infra",
        "Design" => "design",
        _ => "neutral",
    };
    view! { <span class=format!("label-chip label-{class}")><svg viewBox="0 0 8 8" width="8" height="8" aria-hidden="true"><circle cx="4" cy="4" r="3" fill="currentColor"/></svg><span>{name}</span></span> }
}

#[component]
pub fn Avatar(
    #[prop(default = "You")] label: &'static str,
    #[prop(default = "Y")] initials: &'static str,
) -> impl IntoView {
    view! { <span class="avatar" role="img" aria-label=label title=label>{initials}</span> }
}

#[derive(Clone, Copy)]
pub enum ConnectionState {
    Connected,
    Reconnecting,
    Offline,
}

#[component]
pub fn ConnectionIndicator(state: ConnectionState) -> impl IntoView {
    let (class, label, icon, description) = match state {
        ConnectionState::Connected => (
            "connected",
            "Connected",
            "wifi",
            "Live updates are connected",
        ),
        ConnectionState::Reconnecting => (
            "reconnecting",
            "Reconnecting…",
            "refresh",
            "Reconnecting to live updates; your last received data remains visible",
        ),
        ConnectionState::Offline => (
            "offline",
            "Offline",
            "offline",
            "Live updates are offline; showing the last received data",
        ),
    };
    view! { <span class=format!("connection {class}") role="status" aria-live="polite" title=description><Icon name=icon/>{label}</span> }
}

#[component]
pub fn EmptyState(
    title: &'static str,
    description: &'static str,
    action: &'static str,
    icon: &'static str,
    on_action: Callback<()>,
) -> impl IntoView {
    view! { <div class="empty-state"><Icon name=icon/><h2>{title}</h2><p>{description}</p><Button variant="primary" on_click=on_action><Icon name="plus"/>{action}</Button></div> }
}

#[component]
pub fn Markdown(#[prop(into)] source: String) -> impl IntoView {
    view! { <div class="markdown" inner_html=tickets_core::markdown::render(&source)/> }
}
