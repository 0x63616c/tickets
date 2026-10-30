use crate::{components::*, ui::api};
use leptos::prelude::*;
use leptos_router::{components::A, hooks::use_location};
use serde_json::{Value, json};
use wasm_bindgen_futures::spawn_local;

#[derive(Clone, Default, serde::Deserialize)]
pub struct Account {
    pub username: String,
    pub display_name: Option<String>,
}
impl Account {
    pub fn label(&self) -> String {
        self.display_name
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                if self.username.is_empty() {
                    "Account".into()
                } else {
                    self.username.clone()
                }
            })
    }
}
#[derive(Clone, Copy)]
pub struct AccountState {
    pub account: RwSignal<Account>,
    pub error: RwSignal<String>,
    pub loaded: RwSignal<bool>,
}
impl AccountState {
    pub fn new() -> Self {
        Self {
            account: RwSignal::new(Account::default()),
            error: RwSignal::new(String::new()),
            loaded: RwSignal::new(false),
        }
    }
    pub fn load(self) {
        self.error.set(String::new());
        spawn_local(async move {
            match api("GET", "/api/v1/me", Value::Null).await.and_then(|v| {
                serde_json::from_value(v).map_err(|_| "Could not read account.".into())
            }) {
                Ok(account) => {
                    self.account.set(account);
                    self.loaded.set(true);
                    self.error.set(String::new());
                }
                Err(e) => self.error.set(e),
            }
        });
    }
}
pub fn stored(key: &str, default: &str) -> String {
    window()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|s| s.get_item(key).ok().flatten())
        .unwrap_or_else(|| default.into())
}
pub fn persist(key: &str, value: &str) {
    if let Ok(Some(storage)) = window().local_storage() {
        let _ = storage.set_item(key, value);
    }
}
pub fn apply_appearance() {
    let theme = stored("tickets.theme", "dark");
    let light = theme == "light"
        || (theme == "system"
            && window()
                .match_media("(prefers-color-scheme: light)")
                .ok()
                .flatten()
                .is_some_and(|m| m.matches()));
    if let Some(root) = document().document_element() {
        let _ = root.set_attribute("data-theme", if light { "light" } else { "dark" });
        let _ = root.set_attribute("data-motion", &stored("tickets.motion", "system"));
    }
}

#[component]
pub fn Settings() -> impl IntoView {
    let location = use_location();
    let fixture = location.pathname.get_untracked().starts_with("/design");
    let base = if fixture {
        "/design/settings"
    } else {
        "/settings"
    };
    let section = Signal::derive(move || {
        location
            .pathname
            .get()
            .rsplit('/')
            .next()
            .unwrap_or("account")
            .to_string()
    });
    view! {
        <header class="topbar"><div class="breadcrumbs"><Icon name="settings"/><span>"Settings"</span></div></header>
        <div class="settings-layout">
            <nav class="settings-nav" aria-label="Settings">
                <A href=format!("{base}/account") attr:class="nav-item"><Icon name="user"/>"Account"</A>
                <A href=format!("{base}/api-tokens") attr:class="nav-item"><Icon name="code"/>"API tokens"</A>
                <A href=format!("{base}/projects") attr:class="nav-item"><Icon name="project"/>"Projects & GitHub"</A>
                <A href=format!("{base}/appearance") attr:class="nav-item"><Icon name="appearance"/>"Appearance"</A>
                <div class="settings-nav-footer"><a href="/design" rel="external" class="nav-item quiet"><Icon name="grid"/>"Design system"<Icon name="arrow"/></a></div>
            </nav>
            <div class="settings-content">{move||match section.get().as_str() {
                "api-tokens"=>view!{<div class="auth-page settings-tokens"><crate::ui::Tokens fixture/></div>}.into_any(),
                "projects"=>view!{<ProjectSettings fixture/>}.into_any(),
                "appearance"=>view!{<Appearance/>}.into_any(),
                _=>view!{<AccountSettings fixture/>}.into_any(),
            }}</div>
        </div>
    }
}
#[component]
fn AccountSettings(fixture: bool) -> impl IntoView {
    let state = expect_context::<AccountState>();
    let name = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let message = RwSignal::new(String::new());
    let error = RwSignal::new(String::new());
    Effect::new(move |_| {
        name.set(state.account.get().display_name.unwrap_or_default());
    });
    let reveal_root = NodeRef::<leptos::html::Section>::new();
    use_ascii_reveal(
        reveal_root,
        Signal::derive(move || !state.loaded.get()),
        true,
    );
    view! {<section node_ref=reveal_root class="settings-section"><h1>"Account"</h1><p class="settings-description">"Your profile and sign-in details."</p>
        <form on:submit=move|e|{e.prevent_default();if busy.get_untracked(){return;}busy.set(true);error.set(String::new());message.set(String::new());spawn_local(async move {
            let result=if fixture {Ok(Account {username:"sample-user".into(),display_name:Some(name.get_untracked().trim().into())})} else {api("PATCH","/api/v1/me",json!({"display_name":name.get_untracked()})).await.and_then(|v|serde_json::from_value(v).map_err(|_|"Could not read saved account.".into()))};
            match result {Ok(account)=>{state.account.set(account);if message.is_disposed(){return;}message.set("Changes saved.".into());},Err(e)=>{if error.is_disposed(){return;}error.set(e)}}busy.set(false);
        });}>
            <label class="field" for="display-name"><span>"Display name"</span><input id="display-name" aria-describedby="display-name-hint account-error" aria-invalid=move||(!error.get().is_empty()).to_string() autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other" maxlength="80" placeholder="Add your name" disabled=move||!state.loaded.get() || busy.get() prop:value=name on:input=move|e|{name.set(event_target_value(&e));message.set(String::new());}/></label>
            <p id="display-name-hint" class="field-hint">"Shown in your account menu. Leave blank to use your username."</p>
            <div class="settings-save"><BusyButton busy disabled=Signal::derive(move||!state.loaded.get() || name.get().trim()==state.account.get().display_name.unwrap_or_default())>"Save changes"</BusyButton><span role="status">{message}</span></div>
            <p id="account-error" class="settings-error" role="alert">{error}{move||state.error.get()}</p>
            <Show when=move||!state.loaded.get() && state.error.get().is_empty()><p class="loading-caption" role="status"><LoadingIndicator label="Loading account"/>"Loading account…"</p></Show>
            <Show when=move||!state.error.get().is_empty()><button type="button" class="button button-secondary" on:click=move|_|state.load()>"Retry loading account"</button></Show>
        </form>
        <div class="settings-group"><h2>"Sign-in"</h2><dl class="settings-details"><div><dt>"Username"</dt><dd>{move||state.account.get().username}</dd></div><div><dt>"Authentication"</dt><dd>"Passphrase"</dd></div><div><dt>"Session duration"</dt><dd>"7 days"</dd></div></dl><p class="field-hint">"Your sign-in credentials are managed by the deployment owner. Contact them to change your username or passphrase."</p></div>
    </section>}
}
#[component]
fn ProjectSettings(fixture: bool) -> impl IntoView {
    let board = expect_context::<crate::pages::board::BoardState>();
    let reveal_root = NodeRef::<leptos::html::Section>::new();
    use_ascii_reveal(
        reveal_root,
        Signal::derive(move || board.loading.get()),
        true,
    );
    view! {<section node_ref=reveal_root class="settings-section"><h1>"Projects & GitHub"</h1><p class="settings-description">"Projects and their linked repositories."</p>
        <div class="settings-projects">{move||board.projects.get().into_iter().filter(|p|!p.archived).map(|p|{
            let repo=p.github_owner.zip(p.github_repo).map(|(owner,repo)|format!("{owner}/{repo}"));
            view!{<div class="settings-project"><ProjectIcon name=p.name.clone() icon=p.icon.unwrap_or_default() color=p.color/><div><strong>{p.name}</strong><span class="muted mono">{p.key_prefix}</span></div>{repo.map(|repo|view!{<a class="subtle-link" href=format!("https://github.com/{repo}") target="_blank" rel="noreferrer">{repo.clone()}<Icon name="external"/></a>}.into_any()).unwrap_or_else(||view!{<span class="muted">"No repository linked"</span>}.into_any())}</div>}
        }).collect_view()}</div>
        <Show when=move||board.loading.get()><LoadingSkeleton layout="projects"/></Show><Show when=move||!board.loading.get() && board.projects.get().iter().all(|p|p.archived)><p class="muted">"No active projects."</p></Show>
        <p class="settings-error" role="alert">{move||board.error.get()}</p>
        <a class="button button-secondary" href=if fixture{"/design/projects"}else{"/projects"}>"Manage projects"<Icon name="arrow"/></a>
    </section>}
}
#[component]
fn Appearance() -> impl IntoView {
    let theme = RwSignal::new(stored("tickets.theme", "dark"));
    let motion = RwSignal::new(stored("tickets.motion", "system"));
    view! {<section class="settings-section"><h1>"Appearance"</h1><p class="settings-description">"Make Tickets comfortable to use. Preferences stay in this browser."</p>
        <div class="preference-row"><div><h2>"Theme"</h2><p>"Dark by default."</p></div><SelectField label="Theme" value=theme options=Signal::derive(||vec![SelectOption::new("dark","Dark"),SelectOption::new("light","Light"),SelectOption::new("system","System")]) on_change=Callback::new(move|v:String|{persist("tickets.theme",&v);theme.set(v);apply_appearance();})/></div>
        <div class="preference-row"><div><h2>"Motion"</h2><p>"Reduce movement, keeping subtle fades."</p></div><SelectField label="Motion" value=motion options=Signal::derive(||vec![SelectOption::new("system","Use system setting"),SelectOption::new("reduce","Reduce motion")]) on_change=Callback::new(move|v:String|{persist("tickets.motion",&v);motion.set(v);apply_appearance();})/></div>
    </section>}
}
