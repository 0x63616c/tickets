use super::*;
use leptos::{ev, prelude::*};
use leptos_router::components::A;
use leptos_router::hooks::use_location;
use web_sys::wasm_bindgen::JsCast;

fn is_mac() -> bool {
    window()
        .navigator()
        .platform()
        .unwrap_or_default()
        .contains("Mac")
}

/// Navigation hints share the same numbering across the sidebar, palette and help.
pub(super) fn navigation_shortcut(number: usize) -> &'static str {
    if is_mac() {
        ["⌘1", "⌘2", "⌘3", "⌘4", "⌘5", "⌘6", "⌘7", "⌘8", "⌘9"][number - 1]
    } else {
        [
            "Ctrl+1", "Ctrl+2", "Ctrl+3", "Ctrl+4", "Ctrl+5", "Ctrl+6", "Ctrl+7", "Ctrl+8",
            "Ctrl+9",
        ][number - 1]
    }
}

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
                <main id="main" class="page-container" class:page-container--board=move || matches!(location.pathname.get().as_str(), "/board" | "/design/board") tabindex="-1">{children()}</main>
            </div>
        </div>
        <Overlays/>
        <Toast/>
    }
}

pub(super) fn fixture_account(account: crate::pages::settings::AccountState) {
    account.account.set(crate::pages::settings::Account {
        username: "sample-user".into(),
        avatar_url: None,
        display_name: Some("Sample User".into()),
    });
    account.loaded.set(true);
    let board = expect_context::<crate::pages::board::BoardState>();
    board.projects.set(serde_json::from_value(serde_json::json!([
        {"id":1,"name":"Tickets","key_prefix":"TKT","color":crate::colors::value("--project-default"),"icon":"ticket","archived":false,"github_owner":"example","github_repo":"tickets","github_project_kind":"users","github_project_number":null,"next_ticket_number":1,"open_ticket_count":12},
        {"id":2,"name":"Quantum","key_prefix":"QUA","color":crate::colors::value("--label-infra"),"icon":"box","archived":false,"github_owner":"example","github_repo":"quantum","github_project_kind":"users","github_project_number":null,"next_ticket_number":1,"open_ticket_count":8},
        {"id":3,"name":"Apps","key_prefix":"APP","color":crate::colors::value("--success"),"icon":"grid","archived":false,"github_owner":null,"github_repo":null,"github_project_kind":"users","github_project_number":null,"next_ticket_number":1,"open_ticket_count":3}
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
    let editor = RwSignal::new(None::<tickets_core::domain::Project>);
    let archive = RwSignal::new(None::<tickets_core::domain::Project>);
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
    let analytics = if fixture {
        "/design/analytics"
    } else {
        "/analytics"
    };
    let board_path = if fixture { "/design/board" } else { "/board" };
    let list = if fixture { "/design/list" } else { "/list" };
    let projects = if fixture {
        "/design/projects"
    } else {
        "/projects"
    };
    let mission = if fixture {
        "/design/mission"
    } else {
        "/mission"
    };
    let destinations = [
        home, analytics, board_path, list, mission, projects, settings,
    ];
    let handle = window_event_listener(ev::keydown, move |e| {
        let editing = e
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .is_some_and(|el| {
                el.closest("input,textarea,select").ok().flatten().is_some()
                    || el
                        .dyn_ref::<web_sys::HtmlElement>()
                        .is_some_and(|el| el.is_content_editable())
            });
        if editing
            || e.alt_key()
            || e.repeat()
            || e.is_composing()
            || e.default_prevented()
            || document()
                .query_selector(
                    "dialog[open],.popup[data-state=open]:not([hidden]):not([role=tooltip])",
                )
                .ok()
                .flatten()
                .is_some()
        {
            return;
        }
        let primary = if is_mac() {
            e.meta_key() && !e.ctrl_key()
        } else {
            e.ctrl_key() && !e.meta_key()
        };
        if primary && !e.shift_key() {
            if let Some(path) = e
                .key()
                .parse::<usize>()
                .ok()
                .and_then(|number| number.checked_sub(1))
                .and_then(|index| destinations.get(index))
            {
                e.prevent_default();
                go.run((*path).into());
            }
        } else if !e.meta_key() && !e.ctrl_key() && e.key() == "?" {
            e.prevent_default();
            shortcuts.set(true);
        }
    });
    on_cleanup(move || handle.remove());
    view! {<aside class="sidebar" id="workspace-sidebar" aria-label="Workspace">
        <div class="workspace workspace-brand"><Icon name="logo"/><strong>"Tickets"</strong></div>
        <nav class="sidebar-nav" aria-label="Main navigation" on:click=move|e: web_sys::MouseEvent|{if e.target().and_then(|t|t.dyn_into::<web_sys::Element>().ok()).is_some_and(|t|t.closest("a").ok().flatten().is_some()) && let Some(cb)=on_navigate {cb.run(());}}>
            <Tooltip text="Home" shortcut=navigation_shortcut(1)><A href=home attr:class=move||if location.pathname.get()=="/home"{"nav-item active"}else{"nav-item"} exact=true on:click=move|_|{if let Some(cb)=on_navigate{cb.run(());}}><Icon name="home"/><span>"Home"</span></A></Tooltip>
            <Tooltip text="Analytics" shortcut=navigation_shortcut(2)><A href=analytics attr:class="nav-item" exact=true><Icon name="analytics"/><span>"Analytics"</span></A></Tooltip>
            <Tooltip text="Mission Control" shortcut=navigation_shortcut(5)><A href=mission attr:class="nav-item"><Icon name="grid"/><span>"Mission Control"</span></A></Tooltip>
            <button class="nav-item search-trigger" on:click=move|_|{state.dialog.set(DialogKind::Command);if let Some(cb)=on_navigate{cb.run(());}}><Icon name="search"/><span>"Search tickets"</span><Shortcut>"⌘ K"</Shortcut></button>
            <SidebarSection title="Projects" storage="tickets.sidebar.projects" id="sidebar-projects">
                <Tooltip text="All projects" shortcut=navigation_shortcut(6)><A href=projects attr:class="nav-item" on:click=move|_|{if let Some(cb)=on_navigate{cb.run(());}}><Icon name="project"/><span>"All projects"</span></A></Tooltip>
                {move||board.projects.get().into_iter().filter(|p|!p.archived).map(|p|{let id=p.id;let edit=p.clone();let arch=p.clone();let repo=crate::pages::projects::repository(&p);let name=p.name.clone();let active=Signal::derive(move||if fixture {state.current_project.get()==name && ["/design/board","/design/empty"].contains(&location.pathname.get().as_str())}else{board.project.get()==id && location.pathname.get()=="/board"});view!{
                    <div class="sidebar-project-row" data-project-id=id><a href=if fixture {if id!=1 {"/design/empty".to_string()}else{board_path.to_string()}}else{format!("/board?project={id}")} class="nav-item project-nav" class:active=move||active.get() aria-current=move||active.get().then_some("page") aria-label=format!("{}, {} open tickets",p.name,p.open_ticket_count)
                        on:click=move|ev|{if fixture{state.current_project.set(p.name.clone());}else if !board.select_project(id){ev.prevent_default();return;}
                        if let Some(cb)=on_navigate{cb.run(());}}>
                        <ProjectIcon name=p.name.clone() icon=p.icon.clone().unwrap_or_default() color=p.color.clone()/><span>{p.name.clone()}</span><span class="nav-count" title="Open tickets">{p.open_ticket_count}</span>
                    </a>
                    <DropdownMenu class="project-dropdown" trigger_class="icon-button project-more-trigger" label=format!("More actions for {}",edit.name) trigger_content=Box::new(||view!{<Icon name="more"/>}.into_any())>
                        <MenuItem on_select=Callback::new(move|_|{let edit=edit.clone();request_animation_frame(move||editor.set(Some(edit)));})><Icon name="edit"/>"Edit project…"</MenuItem>
                        <MenuItem on_select=Callback::new(move|_|{wasm_bindgen_futures::spawn_local(async move{let url=format!("{}/board?project={id}",window().location().origin().unwrap_or_default());let result=wasm_bindgen_futures::JsFuture::from(window().navigator().clipboard().write_text(&url)).await;state.toast.set(if result.is_ok(){"Project link copied."}else{"Could not copy. Allow clipboard access and try again."}.into());});})><Icon name="copy"/>"Copy link"</MenuItem>
                        {repo.map(|repo|view!{<MenuItem on_select=Callback::new(move|_|{let _=window().open_with_url_and_target_and_features(&format!("https://github.com/{repo}"),"_blank","noopener,noreferrer");})><Icon name="github"/>"Open on GitHub"</MenuItem>})}
                        <MenuSeparator/>
                        <MenuItem on_select=Callback::new(move|_|{let arch=arch.clone();request_animation_frame(move||archive.set(Some(arch)));})><Icon name="archive"/>"Archive project"</MenuItem>
                    </DropdownMenu></div>
                }}).collect_view()}
                <a class="nav-item quiet" href=if fixture {"/design/projects"}else{"/projects?new=true"} on:click=move|e|{if fixture{e.prevent_default();state.dialog.set(DialogKind::Project);}}><Icon name="plus"/><span>"New project"</span></a>
            </SidebarSection>
            <SidebarSection title="Views" storage="tickets.sidebar.views" id="sidebar-views">
                <Tooltip text="Board" shortcut=navigation_shortcut(3)><A href=board_path exact=true attr:class="nav-item"><Icon name="board"/><span>"Board"</span></A></Tooltip>
                <Tooltip text="List" shortcut=navigation_shortcut(4)><A href=list attr:class="nav-item"><Icon name="list"/><span>"List"</span></A></Tooltip>
            </SidebarSection>
        </nav>
        <div class="sidebar-bottom">{fixture.then(||view!{<div class="preview-note"><span>"Design preview"</span><span>"Sample data"</span></div>})}
        <div class="user-menu"><DropdownMenu class="account-dropdown" trigger_class="button button-ghost account-trigger" label="Account menu" trigger_content=Box::new(move||view!{{move||view!{<Avatar label=account.account.get().label() initials={account.account.get().label().chars().take(1).collect::<String>().to_uppercase()} url=Signal::derive(move||account.account.get().avatar_url) lookup=false/>}}<span class="account-name">{move||account.account.get().label()}</span>}.into_any())>
            <MenuItem shortcut=navigation_shortcut(7) on_select=Callback::new(move|_|go.run(settings.into()))><Icon name="settings"/>"Settings"</MenuItem>
            <MenuItem on_select=Callback::new(move|_|shortcuts.set(true)) shortcut="?"><Icon name="help"/>"Keyboard shortcuts"</MenuItem>
            <MenuItem on_select=Callback::new(move|_|{if fixture || signing_out.get_untracked(){return;}signing_out.set(true);wasm_bindgen_futures::spawn_local(async move{match crate::ui::api("POST","/api/v1/logout",serde_json::Value::Null).await{Ok(_)=>{let _=window().location().set_href("/login");},Err(e)=>state.toast.set(e)}signing_out.set(false);});})><Icon name="arrow"/>{move||if signing_out.get(){"Signing out…"}else{"Sign out"}}</MenuItem>
        </DropdownMenu></div>
        </div>
    </aside>
    {let shown=present_value(editor.into());move||shown.get().map(|project: tickets_core::domain::Project|{let id=project.id;view!{<crate::pages::projects::ProjectEditor project=Some(project) archive_control=false restore_project=id open=Signal::derive(move||editor.get().is_some()) on_close=Callback::new(move|_|editor.set(None))/>}})}
    {let shown=present_value(archive.into());move||shown.get().map(|project|view!{<ArchiveProject project fixture open=Signal::derive(move||archive.get().is_some()) on_close=Callback::new(move|_|archive.set(None))/>})}
    <Presence show=shortcuts><Modal title="Keyboard shortcuts" on_close=Callback::new(move|_|shortcuts.set(false))><div class="modal-body shortcut-list">{[("Search tickets","⌘ / Ctrl K"),("New ticket","C"),("Home",navigation_shortcut(1)),("Analytics",navigation_shortcut(2)),("Board",navigation_shortcut(3)),("List",navigation_shortcut(4)),("Mission Control",navigation_shortcut(5)),("All projects",navigation_shortcut(6)),("Settings",navigation_shortcut(7)),("Next / previous list ticket","J / K"),("Open focused ticket","Enter"),("Close menu or dialog","Esc")].into_iter().map(|(name,key)|view!{<div><span>{name}</span><Shortcut>{key}</Shortcut></div>}).collect_view()}</div></Modal></Presence>}
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
        <div class="view-tabs"><nav aria-label="Ticket view"><A href="/design/board" attr:class=if view == "board" { "view-tab selected" } else { "view-tab" }><Icon name="board"/>"Board"</A><A href="/design/list" attr:class=if view == "list" { "view-tab selected" } else { "view-tab" }><Icon name="list"/>"List"</A></nav><a class="subtle-link" href="https://github.com/0x63616c/tickets" target="_blank" rel="noreferrer"><Icon name="github"/>"0x63616c/tickets"<Icon name="external"/></a></div>
    }
}

#[component]
fn ArchiveProject(
    project: tickets_core::domain::Project,
    fixture: bool,
    open: Signal<bool>,
    on_close: Callback<()>,
) -> impl IntoView {
    let board = expect_context::<crate::pages::board::BoardState>();
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let id = project.id;
    on_cleanup(move || request_animation_frame(move || focus_project_action(id)));
    view! {<Modal title="Archive project" open on_close locked=Signal::derive(move||busy.get())>
    <form on:submit=move|e|{e.prevent_default();if busy.get_untracked(){return;}busy.set(true);wasm_bindgen_futures::spawn_local(async move{
        let result=if fixture{Ok(serde_json::Value::Null)}else{crate::ui::api("PATCH",&format!("/api/v1/projects/{id}"),serde_json::json!({"archived":true})).await};
        busy.set(false);match result{Ok(_)=>{board.projects.update(|ps|{if let Some(p)=ps.iter_mut().find(|p|p.id==id){p.archived=true;}});on_close.run(());},Err(e)=>error.set(e)}
    });}>
    <div class="modal-body"><p>{format!("Archive {}?",project.name)}</p><p class="muted">"It will leave the sidebar. Tickets are kept. Restore it from All projects."</p><Show when=move||!error.get().is_empty()><p role="alert">{error}</p></Show></div>
    <div class="modal-footer"><button type="button" class="button button-ghost" disabled=busy on:click=move|_|on_close.run(())>"Cancel"</button><BusyButton busy>"Archive project"</BusyButton></div>
    </form></Modal>}
}

// Edits can replace rows and archiving removes them. Resolve the surviving
// control after the dialog leaves the top layer, rather than retaining a node.
pub(crate) fn focus_project_action(id: i64) {
    let control = document()
        .query_selector(&format!(
            ".sidebar-project-row[data-project-id='{id}'] .project-more-trigger"
        ))
        .ok()
        .flatten()
        .or_else(|| {
            document()
                .query_selector("#sidebar-projects > a")
                .ok()
                .flatten()
        });
    if let Some(control) = control.and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok()) {
        let _ = control.focus();
    }
}
