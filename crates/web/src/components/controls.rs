use super::Icon;
use leptos::prelude::*;

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
    #[prop(default=Signal::derive(||false))] disabled: Signal<bool>,
) -> impl IntoView {
    view! { <super::Tooltip text=label><button type="button" class="icon-button" aria-label=label disabled=disabled on:click=move |_| on_click.run(())><Icon name=icon/></button></super::Tooltip> }
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
    #[prop(default = false)] hide_label: bool,
) -> impl IntoView {
    view! { <label class="field" for=id>{(!hide_label).then(|| view! { <span>{label}</span> })}<input aria-label=hide_label.then_some(label) autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  id=id placeholder=placeholder prop:value=move || value.get() on:input=move |ev| value.set(event_target_value(&ev))/></label> }
}

/// Legacy import metadata remains stored but has dedicated UI affordances.
pub(crate) fn visible_label(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    !name.starts_with("priority:") && name != "status:blocked" && name != "later"
}

#[component]
pub fn LabelChip(#[prop(into)] name: String, #[prop(optional)] color: String) -> impl IntoView {
    let visible = visible_label(&name);
    let class = match name.as_str() {
        "Feature" => "feature",
        "Bug" => "bug",
        "Infrastructure" => "infra",
        "Design" => "design",
        _ => "neutral",
    };
    let tint = if tickets_core::domain::validate_color(&color).is_ok() {
        format!("--c:{color}")
    } else {
        String::new()
    };
    view! { {visible.then(|| view! { <span title=name.clone() class=format!("label-chip label-{class}") style=tint><svg viewBox="0 0 8 8" width="8" height="8" aria-hidden="true"><circle cx="4" cy="4" r="3" fill="currentColor"/></svg><span>{name.clone()}</span></span> })} }
}

#[component]
pub fn Avatar(
    #[prop(default=Signal::derive(||None))] url: Signal<Option<String>>,
    #[prop(default = true)] lookup: bool,
    #[prop(default = "Unassigned".into(), into)] label: String,
    #[prop(default = "".into(), into)] initials: String,
) -> impl IntoView {
    let account = expect_context::<crate::pages::settings::AccountState>();
    let name = label.clone();
    let source = Signal::derive(move || {
        url.get().or_else(|| {
            if !lookup {
                return None;
            }
            let current = account.account.get();
            if name == current.username || name == current.label() {
                return current.avatar_url;
            }
            account
                .users
                .get()
                .into_iter()
                .find(|u| u.username == name || u.label() == name)
                .and_then(|u| u.avatar_url)
        })
    });
    let failed = RwSignal::new(None::<String>);
    view! { <span class="avatar" role="img" aria-label=label.clone() title=label>{move||{
        let src=source.get();
        match src.filter(|src|failed.get().as_ref()!=Some(src)) {
            Some(src)=>{let failed_src=src.clone();view!{<img src=src alt="" on:error=move|_|failed.set(Some(failed_src.clone()))/>}.into_any()},
            None=>initials.clone().into_any(),
        }
    }}</span> }
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
pub fn Markdown(
    #[prop(into)] source: String,
    #[prop(optional_no_strip)] project: Option<String>,
) -> impl IntoView {
    view! { <div class="markdown" inner_html=tickets_core::markdown::render_in_project(&source, project.as_deref())/> }
}
