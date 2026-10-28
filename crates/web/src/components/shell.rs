use super::*;
use leptos::prelude::*;
use leptos_router::components::A;

#[component]
pub fn AppShell(children: Children) -> impl IntoView {
    let state = expect_context::<DesignState>();
    let mobile_open = RwSignal::new(false);
    view! {
        <a class="skip-link" href="#main">"Skip to content"</a>
        <div class="app-shell" class:sidebar-open=move || mobile_open.get()>
            <aside class="sidebar" aria-label="Workspace">
                <div class="workspace"><span class="workspace-mark"><Icon name="ticket"/></span><strong>"Tickets"</strong><span class="workspace-personal">"Personal"</span></div>
                <button class="search-trigger" on:click=move |_| state.dialog.set(DialogKind::Command)><Icon name="search"/><span>"Search tickets…"</span><Shortcut>"⌘ K"</Shortcut></button>
                <nav class="sidebar-nav" aria-label="Main navigation" on:click=move |_| mobile_open.set(false)>
                    <A href="/design/projects" attr:class="nav-item"><Icon name="project"/><span>"Projects"</span><span class="nav-count">{move || state.projects.get().len() + 2}</span></A>
                    <div class="nav-section-label">"Projects"</div>
                    <A href="/design/board" attr:class="nav-item project-nav" on:click=move |_| state.current_project.set("Tickets".into())><span class="project-mark">"T"</span><span>"Tickets"</span><span class="nav-count">{move || state.tickets.get().iter().filter(|t| t.project == "Tickets").count()}</span></A>
                    <A href="/design/empty" attr:class="nav-item" on:click=move |_| state.current_project.set("Quantum".into())><span class="project-mark muted-mark">"Q"</span><span>"Quantum"</span></A>
                    {move || state.projects.get().into_iter().map(|name| { let project = name.clone(); view! { <A href="/design/empty" attr:class="nav-item project-nav" on:click=move |_| state.current_project.set(project.clone())><Icon name="project"/><span>{name}</span></A> } }).collect_view()}
                    <button class="nav-item quiet" on:click=move |_| state.dialog.set(DialogKind::Project)><Icon name="plus"/><span>"New project"</span></button>
                    <div class="nav-section-label">"Views"</div>
                    <A href="/design/board" attr:class="nav-item"><Icon name="board"/><span>"Board"</span></A>
                    <A href="/design/list" attr:class="nav-item"><Icon name="list"/><span>"List"</span></A>
                </nav>
                <div class="sidebar-bottom">
                    <A href="/settings/tokens" attr:class="nav-item"><Icon name="settings"/><span>"API tokens"</span></A>
                    <A href="/login" attr:class="nav-item"><Icon name="arrow"/><span>"Sign in"</span></A>
                    <A href="/design" exact=true attr:class="nav-item"><Icon name="grid"/><span>"Design system"</span></A>
                    <div class="preview-note"><span>"Design preview"</span><span>"Sample data"</span></div>
                    <div class="sidebar-account"><Avatar/><span>"Personal workspace"</span><IconButton label="Open commands" icon="help" on_click=Callback::new(move |_| state.dialog.set(DialogKind::Command))/></div>
                </div>
            </aside>
            <div class="app-main">
                <div class="mobile-bar"><IconButton label="Toggle sidebar" icon="list" on_click=Callback::new(move |_| mobile_open.update(|v| *v = !*v))/><strong>"Tickets"</strong></div>
                <main id="main" tabindex="-1">{children()}</main>
            </div>
        </div>
        <Overlays/>
        <Toast/>
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
