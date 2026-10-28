use super::*;
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_location;
use web_sys::wasm_bindgen::JsCast;

#[component]
pub fn AppShell(children: Children) -> impl IntoView {
    let state = expect_context::<DesignState>();
    let board = expect_context::<crate::pages::board::BoardState>();
    let location = use_location();
    wasm_bindgen_futures::spawn_local(async move {
        board.signed_in.set(
            crate::ui::api("GET", "/api/v1/me", serde_json::Value::Null)
                .await
                .is_ok(),
        );
        if board.signed_in.get_untracked()
            && location.pathname.get_untracked() == "/settings/tokens"
        {
            board.refresh();
        }
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
            <aside class="sidebar" id="workspace-sidebar" aria-label="Workspace">
                <div class="workspace"><a class="workspace-switcher" href="/" aria-label="Tickets home"><span class="workspace-mark">"T"</span><strong>"Tickets"</strong></a><Avatar/></div>
                <button class="search-trigger" on:click=move |_| state.dialog.set(DialogKind::Command)><Icon name="search"/><span>"Search tickets…"</span><Shortcut>"⌘ K"</Shortcut></button>
                <Show when=move || location.pathname.get().starts_with("/design") fallback=move || view! {
                    <nav class="sidebar-nav" aria-label="Main navigation" on:click=move |_| mobile_open.set(false)>
                        <div class="nav-section-label">"Projects"</div>
                        {move || board.projects.get().into_iter().map(|p| {
                            let id = p.id;
                            view! { <A href="/" attr:class="nav-item project-nav" on:click=move |ev| { if !board.select_project(id) {ev.prevent_default();} }><span class="project-dot" style=format!("--dot:{}",p.color)/><span>{p.name}</span><span class="nav-count">{move||board.tickets.get().iter().filter(|t|t.project_id==id).count()}</span></A> }
                        }).collect_view()}
                        <div class="nav-section-label">"Views"</div>
                        <A href="/" attr:class="nav-item"><Icon name="board"/>"Board"</A>
                        <A href="/list" attr:class="nav-item"><Icon name="list"/>"List"</A>
                    </nav>
                }>

                <nav class="sidebar-nav" aria-label="Main navigation" on:click=move |_| mobile_open.set(false)>
                    <A href="/design/projects" attr:class="nav-item"><Icon name="project"/><span>"Projects"</span><span class="nav-count">{move || state.projects.get().len() + 2}</span></A>
                    <div class="nav-section-label">"Projects"</div>
                    <A href="/design/board" attr:class="nav-item project-nav" on:click=move |_| state.current_project.set("Tickets".into())><span class="project-dot" style="--dot:var(--accent)"/><span>"Tickets"</span><span class="nav-count">{move || state.tickets.get().iter().filter(|t| t.project == "Tickets").count()}</span></A>
                    <A href="/design/empty" attr:class="nav-item" on:click=move |_| state.current_project.set("Quantum".into())><span class="project-dot" style="--dot:var(--label-infra)"/><span>"Quantum"</span></A>
                    {move || state.projects.get().into_iter().map(|name| { let project = name.clone(); view! { <A href="/design/empty" attr:class="nav-item project-nav" on:click=move |_| state.current_project.set(project.clone())><span class="project-dot" style="--dot:var(--success)"/><span>{name}</span></A> } }).collect_view()}
                    <button class="nav-item quiet" on:click=move |_| state.dialog.set(DialogKind::Project)><Icon name="plus"/><span>"New project"</span></button>
                    <div class="nav-section-label">"Views"</div>
                    <A href="/design/board" attr:class="nav-item"><Icon name="board"/><span>"Board"</span></A>
                    <A href="/design/list" attr:class="nav-item"><Icon name="list"/><span>"List"</span></A>
                </nav>
                </Show>
                <div class="sidebar-bottom">
                    <A href="/settings/tokens" attr:class="nav-item"><Icon name="settings"/><span>"API tokens"</span></A>
                    <Show when=move||board.signed_in.get() fallback=||view!{<A href="/login" attr:class="nav-item"><Icon name="arrow"/><span>"Sign in"</span></A>}><button class="nav-item" on:click=move|_|wasm_bindgen_futures::spawn_local(async move{match crate::ui::api("POST","/api/v1/logout",serde_json::Value::Null).await{Ok(_)=>{board.signed_in.set(false);let _=window().location().set_href("/login");},Err(e)=>state.toast.set(e)}})><Icon name="arrow"/><span>"Sign out"</span></button></Show>
                    <A href="/design" exact=true attr:class="nav-item"><Icon name="grid"/><span>"Design system"</span></A>
                    <Show when=move || location.pathname.get().starts_with("/design")><div class="preview-note"><span>"Design preview"</span><span>"Sample data"</span></div></Show>
                    <div class="sidebar-account"><Avatar/><span>"Personal workspace"</span><Show when=move || location.pathname.get().starts_with("/design")><IconButton label="Open commands" icon="help" on_click=Callback::new(move |_| state.dialog.set(DialogKind::Command))/></Show></div>
                </div>
            </aside>
            <div class="app-main">
                <div class="mobile-bar"><button node_ref=toggle class="icon-button" aria-label="Toggle sidebar" aria-controls="workspace-sidebar" aria-expanded=move||mobile_open.get().to_string() on:click=move|_|{ mobile_open.update(|v| *v = !*v); if mobile_open.get_untracked() { request_animation_frame(move||{if let Ok(Some(first))=document().query_selector(".sidebar a") && let Some(first)=first.dyn_ref::<web_sys::HtmlElement>() {let _=first.focus();}}); } }><Icon name="list"/></button><strong>"Tickets"</strong></div>
                <main id="main" tabindex="-1">{children()}</main>
            </div>
        </div>
        <Overlays/>
        <Toast/>
    }
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
    let (Some(first), Some(last), Some(active)) = (
        items.item(0),
        items.item(items.length().saturating_sub(1)),
        document().active_element(),
    ) else {
        return;
    };
    let target = if event.shift_key() && first.is_same_node(Some(&active)) {
        Some(last)
    } else if !event.shift_key() && last.is_same_node(Some(&active)) {
        Some(first)
    } else {
        None
    };
    if let Some(target) = target.and_then(|node| node.dyn_into::<web_sys::HtmlElement>().ok()) {
        event.prevent_default();
        let _ = target.focus();
    }
}

#[component]
pub fn TopBar(
    #[prop(default = "Tickets")] title: &'static str,
    #[prop(default = false)] detail: bool,
) -> impl IntoView {
    let state = expect_context::<DesignState>();
    view! {
        <header class="topbar">
            <div class="breadcrumbs"><A href="/design/projects"><Icon name="project"/>"Projects"</A>{(title != "Projects").then(|| view! { <span class="breadcrumb-slash">"/"</span><span class="breadcrumb-current">{move || if title == "Project" || title == "Tickets" || detail { state.current_project.get() } else { title.into() }}</span> })}{detail.then(|| view! { <span class="breadcrumb-slash">"/"</span><span class="mono">{move || format!("TKT-{}", state.selected_ticket.get())}</span> })}</div>
            <div class="topbar-actions"><ConnectionIndicator state=ConnectionState::Connected/><span class="toolbar-divider"/>{if title == "Projects" { view! { <Button variant="primary" on_click=Callback::new(move |_| state.dialog.set(DialogKind::Project))><Icon name="plus"/>"New project"</Button> }.into_any() } else { view! { <Button variant="primary" on_click=Callback::new(move |_| state.new_ticket(Status::Todo))><Icon name="plus"/>"New ticket"<Shortcut>"C"</Shortcut></Button> }.into_any() }}</div>
        </header>
    }
}

#[component]
pub fn ProjectHeader(#[prop(default = "board")] view: &'static str) -> impl IntoView {
    view! {
        <div class="view-tabs"><nav aria-label="Ticket view"><A href="/design/board" attr:class=if view == "board" { "view-tab selected" } else { "view-tab" }><Icon name="board"/>"Board"</A><A href="/design/list" attr:class=if view == "list" { "view-tab selected" } else { "view-tab" }><Icon name="list"/>"List"</A></nav><a class="subtle-link" href="https://github.com/0x63616c/tickets" target="_blank" rel="noreferrer"><Icon name="link"/>"0x63616c/tickets"<Icon name="external"/></a></div>
    }
}
