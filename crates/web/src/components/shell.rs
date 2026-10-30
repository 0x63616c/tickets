use super::*;
use leptos::{ev, prelude::*};
use leptos_router::components::A;
use leptos_router::hooks::use_location;
use web_sys::wasm_bindgen::JsCast;

#[component]
pub fn AppShell(children: Children) -> impl IntoView {
    let board = expect_context::<crate::pages::board::BoardState>();
    let location = use_location();
    crate::live::subscribe(board, location.pathname);
    let account = expect_context::<crate::pages::settings::AccountState>();
    let fixture = location.pathname.get_untracked().starts_with("/design");
    if fixture {
        fixture_account(account);
    } else {
        account.load();
    }
    if !fixture && location.pathname.get_untracked().starts_with("/settings") {
        board.refresh();
    }
    Effect::new(move |_| {
        let cleanup = install_motion();
        on_cleanup(move || {
            let _ = cleanup.call0(&wasm_bindgen::JsValue::NULL);
        });
    });
    let mobile_open = RwSignal::new(false);
    let toggle = NodeRef::<leptos::html::Button>::new();
    let close_sidebar = Callback::new(move |_| {
        mobile_open.set(false);
        if let Some(toggle) = toggle.get_untracked() {
            let _ = toggle.focus();
        }
    });
    view! {
        <a class="skip-link" href="#main">"Skip to content"</a>
        <div class="app-shell" class:sidebar-open=move || mobile_open.get() on:keydown=move |event: web_sys::KeyboardEvent| { if mobile_open.get_untracked() { if event.key()=="Escape" { close_sidebar.run(()); } else { sidebar_key(event); } } }>
            <Show when=move||mobile_open.get()><button class="sidebar-dismiss" aria-label="Close sidebar" tabindex="-1" on:click=move|_|close_sidebar.run(())/></Show>
            <Sidebar on_navigate=Callback::new(move |_| mobile_open.set(false))/>
            <div class="app-main">
                <div class="mobile-bar"><button node_ref=toggle class="icon-button" aria-label="Toggle sidebar" aria-controls="workspace-sidebar" aria-expanded=move||mobile_open.get().to_string() on:click=move|_|{ mobile_open.update(|v| *v = !*v); if mobile_open.get_untracked() { request_animation_frame(move||{if let Ok(Some(first))=document().query_selector(".sidebar a[href], .sidebar button") && let Some(first)=first.dyn_ref::<web_sys::HtmlElement>() {let _=first.focus();}}); } }><Icon name="list"/></button><strong class="mobile-brand"><Icon name="logo"/>"Tickets"</strong></div>
                <main id="main" tabindex="-1">{children()}</main>
            </div>
        </div>
        <Overlays/>
        <Toast/>
    }
}

pub(super) fn fixture_account(account: crate::pages::settings::AccountState) {
    account.account.set(crate::pages::settings::Account {
        username: "sample-user".into(),
        display_name: Some("Sample User".into()),
    });
    account.loaded.set(true);
    let board = expect_context::<crate::pages::board::BoardState>();
    board.projects.set(serde_json::from_value(serde_json::json!([
        {"id":1,"name":"Tickets","key_prefix":"TKT","color":"#5e6ad2","icon":"ticket","archived":false,"github_owner":"example","github_repo":"tickets","github_project_kind":"users","github_project_number":null,"next_ticket_number":1,"open_ticket_count":12},
        {"id":2,"name":"Quantum","key_prefix":"QUA","color":"#4ea7fc","icon":"box","archived":false,"github_owner":"example","github_repo":"quantum","github_project_kind":"users","github_project_number":null,"next_ticket_number":1,"open_ticket_count":8},
        {"id":3,"name":"Apps","key_prefix":"APP","color":"#27a644","icon":"grid","archived":false,"github_owner":null,"github_repo":null,"github_project_kind":"users","github_project_number":null,"next_ticket_number":1,"open_ticket_count":3}
    ])).expect("valid project fixtures"));
    board.loading.set(false);
}

#[component]
pub fn Sidebar(#[prop(optional)] on_navigate: Option<Callback<()>>) -> impl IntoView {
    let state = expect_context::<DesignState>();
    let board = expect_context::<crate::pages::board::BoardState>();
    let account = expect_context::<crate::pages::settings::AccountState>();
    let location = use_location();
    let fixture = location.pathname.get_untracked().starts_with("/design");
    if fixture {
        fixture_account(account);
    }
    let shortcuts = RwSignal::new(false);
    let count_revision = RwSignal::new(0_u64);
    Effect::new(move |_| {
        board.tickets.track();
        if fixture || board.loading.get_untracked() {
            return;
        }
        count_revision.update(|r| *r += 1);
        let revision = count_revision.get_untracked();
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(projects) =
                crate::pages::board::all::<tickets_core::domain::Project>("projects").await
                && !count_revision.is_disposed()
                && count_revision.get_untracked() == revision
            {
                board.projects.set(projects);
            }
        });
    });
    let signing_out = RwSignal::new(false);
    let navigate = leptos_router::hooks::use_navigate();
    let go = Callback::new(move |path: String| {
        if let Some(cb) = on_navigate {
            cb.run(());
        }
        navigate(&path, Default::default());
    });
    let settings = if fixture {
        "/design/settings/account"
    } else {
        "/settings/account"
    };
    let home = if fixture { "/design/home" } else { "/" };
    let board_path = if fixture { "/design/board" } else { "/board" };
    let list = if fixture { "/design/list" } else { "/list" };
    let projects = if fixture {
        "/design/projects"
    } else {
        "/projects"
    };
    let stamp = StoredValue::new(0.0);
    let handle = window_event_listener(ev::keydown, move |e| {
        let editing = e
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .is_some_and(|el| {
                el.closest("input,textarea,select,[contenteditable=true]")
                    .ok()
                    .flatten()
                    .is_some()
            });
        if editing
            || e.meta_key()
            || e.ctrl_key()
            || e.alt_key()
            || e.repeat()
            || document()
                .query_selector("dialog[open],.popup[data-state=open]:not([hidden])")
                .ok()
                .flatten()
                .is_some()
        {
            stamp.set_value(0.);
            return;
        }
        let now = js_sys::Date::now();
        if e.key() == "?" {
            e.prevent_default();
            shortcuts.set(true);
        } else if e.key().eq_ignore_ascii_case("g") {
            stamp.set_value(now);
        } else {
            if now - stamp.get_value() < 1000. {
                let path = match e.key().to_lowercase().as_str() {
                    "b" => Some(board_path),
                    "l" => Some(list),
                    "h" => Some(home),
                    _ => None,
                };
                if let Some(path) = path {
                    e.prevent_default();
                    go.run(path.into());
                }
            }
            stamp.set_value(0.);
        }
    });
    on_cleanup(move || handle.remove());
    view! {<aside class="sidebar" id="workspace-sidebar" aria-label="Workspace">
        <div class="workspace workspace-brand"><Icon name="logo"/><strong>"Tickets"</strong></div>
        <nav class="sidebar-nav" aria-label="Main navigation" on:click=move|e: web_sys::MouseEvent|{if e.target().and_then(|t|t.dyn_into::<web_sys::Element>().ok()).is_some_and(|t|t.closest("a").ok().flatten().is_some()) && let Some(cb)=on_navigate {cb.run(());}}>
            <Tooltip text="Home · G then H"><A href=home attr:class=move||if location.pathname.get()=="/home"{"nav-item active"}else{"nav-item"} exact=true on:click=move|_|{if let Some(cb)=on_navigate{cb.run(());}}><Icon name="home"/><span>"Home"</span></A></Tooltip>
            <button class="nav-item search-trigger" on:click=move|_|{state.dialog.set(DialogKind::Command);if let Some(cb)=on_navigate{cb.run(());}}><Icon name="search"/><span>"Search tickets"</span><Shortcut>"⌘ K"</Shortcut></button>
            <SidebarSection title="Projects" storage="tickets.sidebar.projects" id="sidebar-projects">
                <A href=projects attr:class="nav-item" on:click=move|_|{if let Some(cb)=on_navigate{cb.run(());}}><Icon name="project"/><span>"All projects"</span></A>
                {move||board.projects.get().into_iter().filter(|p|!p.archived).map(|p|{let id=p.id;let name=p.name.clone();let active=Signal::derive(move||if fixture {state.current_project.get()==name && ["/design/board","/design/empty"].contains(&location.pathname.get().as_str())}else{board.project.get()==id && location.pathname.get()=="/board"});view!{
                    <a href=if fixture && id!=1 {"/design/empty"}else{board_path} class="nav-item project-nav" class:active=move||active.get() aria-current=move||active.get().then_some("page") aria-label=format!("{}, {} open tickets",p.name,p.open_ticket_count)
                        on:click=move|ev|{if fixture{state.current_project.set(p.name.clone());}else if !board.select_project(id){ev.prevent_default();return;}
                        if let Some(cb)=on_navigate{cb.run(());}}>
                        <ProjectIcon name=p.name.clone() icon=p.icon.clone().unwrap_or_default() color=p.color.clone()/><span>{p.name.clone()}</span><span class="nav-count" title="Open tickets">{p.open_ticket_count}</span>
                    </a>
                }}).collect_view()}
                <a class="nav-item quiet" href=if fixture {"/design/projects"}else{"/projects?new=true"} on:click=move|e|{if fixture{e.prevent_default();state.dialog.set(DialogKind::Project);}}><Icon name="plus"/><span>"New project"</span></a>
            </SidebarSection>
            <SidebarSection title="Views" storage="tickets.sidebar.views" id="sidebar-views">
                <Tooltip text="Board · G then B"><A href=board_path exact=true attr:class="nav-item"><Icon name="board"/><span>"Board"</span></A></Tooltip>
                <Tooltip text="List · G then L"><A href=list attr:class="nav-item"><Icon name="list"/><span>"List"</span></A></Tooltip>
            </SidebarSection>
        </nav>
        <div class="sidebar-bottom">{fixture.then(||view!{<div class="preview-note"><span>"Design preview"</span><span>"Sample data"</span></div>})}
        <div class="user-menu"><DropdownMenu class="account-dropdown" trigger_class="button button-ghost account-trigger" label="Account menu" trigger_content=Box::new(move||view!{<span class="avatar" aria-hidden="true">{move||account.account.get().label().chars().next().unwrap_or('U').to_uppercase().to_string()}</span><span class="account-name">{move||account.account.get().label()}</span>}.into_any())>
            <MenuItem on_select=Callback::new(move|_|go.run(settings.into()))><Icon name="settings"/>"Settings"</MenuItem>
            <MenuItem on_select=Callback::new(move|_|shortcuts.set(true)) shortcut="?"><Icon name="help"/>"Keyboard shortcuts"</MenuItem>
            <MenuItem on_select=Callback::new(move|_|{if fixture || signing_out.get_untracked(){return;}signing_out.set(true);wasm_bindgen_futures::spawn_local(async move{match crate::ui::api("POST","/api/v1/logout",serde_json::Value::Null).await{Ok(_)=>{let _=window().location().set_href("/login");},Err(e)=>state.toast.set(e)}signing_out.set(false);});})><Icon name="arrow"/>{move||if signing_out.get(){"Signing out…"}else{"Sign out"}}</MenuItem>
        </DropdownMenu></div>
        </div>
    </aside>
    <Presence show=shortcuts><Modal title="Keyboard shortcuts" on_close=Callback::new(move|_|shortcuts.set(false))><div class="modal-body shortcut-list">{[("Search tickets","⌘ / Ctrl K"),("New ticket","C"),("Home","G then H"),("Board","G then B"),("List","G then L"),("Next / previous list ticket","J / K"),("Open focused ticket","Enter"),("Close menu or dialog","Esc")].into_iter().map(|(name,key)|view!{<div><span>{name}</span><Shortcut>{key}</Shortcut></div>}).collect_view()}</div></Modal></Presence>}
}

#[component]
fn SidebarSection(
    title: &'static str,
    storage: &'static str,
    id: &'static str,
    children: Children,
) -> impl IntoView {
    let open = RwSignal::new(crate::pages::settings::stored(storage, "open") != "closed");
    view! {<section class="sidebar-section"><button class="nav-section-toggle" aria-expanded=move||open.get().to_string() aria-controls=id on:click=move|_|{open.update(|v|*v= !*v);crate::pages::settings::persist(storage,if open.get_untracked(){"open"}else{"closed"});}><span>{title}</span><Icon name="chevron"/></button><div id=id class="sidebar-section-items" hidden=move||!open.get()>{children()}</div></section>}
}

fn sidebar_key(event: web_sys::KeyboardEvent) {
    if event.key() != "Tab" {
        return;
    }
    let Ok(items) =
        document().query_selector_all(".sidebar a[href], .sidebar button:not([disabled])")
    else {
        return;
    };
    let visible = (0..items.length())
        .filter_map(|i| items.item(i))
        .filter_map(|n| n.dyn_into::<web_sys::HtmlElement>().ok())
        .filter(|e| e.get_bounding_client_rect().height() > 0.)
        .collect::<Vec<_>>();
    let (Some(first), Some(last), Some(active)) =
        (visible.first(), visible.last(), document().active_element())
    else {
        return;
    };
    let target = if event.shift_key() && first.is_same_node(Some(&active)) {
        Some(last.clone())
    } else if !event.shift_key() && last.is_same_node(Some(&active)) {
        Some(first.clone())
    } else {
        None
    };
    if let Some(target) = target {
        event.prevent_default();
        let _ = target.focus();
    }
}

#[component]
pub fn TopBar(
    #[prop(default = true)] new_ticket: bool,
    #[prop(default = "Tickets")] title: &'static str,
    #[prop(default = false)] detail: bool,
) -> impl IntoView {
    let state = expect_context::<DesignState>();
    view! {
        <header class="topbar">
            <div class="breadcrumbs"><A href="/design/projects"><Icon name="project"/>"Projects"</A>{(title != "Projects").then(|| view! { <span class="breadcrumb-slash">"/"</span><span class="breadcrumb-current">{move || if title == "Project" || title == "Tickets" || detail { state.current_project.get() } else { title.into() }}</span> })}{detail.then(|| view! { <span class="breadcrumb-slash">"/"</span><span class="mono">{move || format!("TKT-{}", state.selected_ticket.get())}</span> })}</div>
            <div class="topbar-actions"><ConnectionIndicator state=ConnectionState::Connected/>{(new_ticket || title == "Projects").then(|| view! { <span class="toolbar-divider"/> })}{if title == "Projects" { view! { <Button variant="primary" on_click=Callback::new(move |_| state.dialog.set(DialogKind::Project))><Icon name="plus"/>"New project"</Button> }.into_any() } else if new_ticket { view! { <Button variant="primary" on_click=Callback::new(move |_| state.new_ticket(Status::Todo))><Icon name="plus"/>"New ticket"<Shortcut>"C"</Shortcut></Button> }.into_any() } else { ().into_any() }}</div>
        </header>
    }
}

#[component]
pub fn ProjectHeader(#[prop(default = "board")] view: &'static str) -> impl IntoView {
    view! {
        <div class="view-tabs"><nav aria-label="Ticket view"><A href="/design/board" attr:class=if view == "board" { "view-tab selected" } else { "view-tab" }><Icon name="board"/>"Board"</A><A href="/design/list" attr:class=if view == "list" { "view-tab selected" } else { "view-tab" }><Icon name="list"/>"List"</A></nav><a class="subtle-link" href="https://github.com/0x63616c/tickets" target="_blank" rel="noreferrer"><Icon name="link"/>"0x63616c/tickets"<Icon name="external"/></a></div>
    }
}
