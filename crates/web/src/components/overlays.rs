use super::*;
use crate::fixtures::{Priority, Ticket};
use leptos::{ev, html, prelude::*};
use leptos_router::{components::A, hooks::use_navigate};
use tickets_core::{
    dependency::{DependencyError, DependencyGraph},
    domain::Dependency,
};
use web_sys::wasm_bindgen::JsCast;

/// Native dialog provides focus containment, Escape and inert background.
#[component]
pub fn Modal(
    title: &'static str,
    children: Children,
    on_close: Callback<()>,
    #[prop(default = Signal::derive(|| false))] locked: Signal<bool>,
) -> impl IntoView {
    let dialog = NodeRef::<html::Dialog>::new();
    let opener = StoredValue::new_local(document().active_element());
    on_cleanup(move || {
        opener.with_value(|opener| {
            if let Some(opener) = opener
                && opener.is_connected()
                && let Some(opener) = opener.dyn_ref::<web_sys::HtmlElement>()
            {
                let _ = opener.focus();
            }
        });
    });
    Effect::new(move || {
        if let Some(node) = dialog.get() {
            let _ = node.show_modal();
            if let Ok(Some(input)) = node.query_selector("input")
                && let Some(input) = input.dyn_ref::<web_sys::HtmlElement>()
            {
                let _ = input.focus();
            }
        }
    });
    let close = Callback::new(move |_| {
        if locked.get_untracked() {
            return;
        }
        if let Some(node) = dialog.get_untracked() {
            node.close();
        }
        on_close.run(());
    });
    view! {
        <dialog node_ref=dialog class="modal" aria-label=title on:cancel=move |ev: web_sys::Event| { ev.prevent_default(); close.run(()); }>
            <div class="modal-header"><h2>{title}</h2><IconButton label="Close dialog" icon="close" on_click=close/></div>
            {children()}
        </dialog>
    }
}

#[component]
pub fn Overlays() -> impl IntoView {
    let state = expect_context::<DesignState>();
    let location = leptos_router::hooks::use_location();
    let handle = window_event_listener(ev::keydown, move |event| {
        if event.key().eq_ignore_ascii_case("k") && (event.meta_key() || event.ctrl_key()) {
            event.prevent_default();
            if state.dialog.get_untracked() != DialogKind::Command
                && document()
                    .query_selector("dialog[open]")
                    .ok()
                    .flatten()
                    .is_some()
            {
                return;
            }
            state
                .dialog
                .set(if state.dialog.get_untracked() == DialogKind::Command {
                    DialogKind::None
                } else {
                    DialogKind::Command
                });
        }
        let target = event
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok());
        let editing = target.is_some_and(|t| {
            ["INPUT", "TEXTAREA", "SELECT"].contains(&t.tag_name().as_str())
                || t.is_content_editable()
        });
        if location.pathname.get_untracked().starts_with("/design")
            && !editing
            && !event.meta_key()
            && !event.ctrl_key()
            && !event.alt_key()
            && event.key() == "c"
            && state.dialog.get_untracked() == DialogKind::None
        {
            state.new_ticket(Status::Todo);
        }
    });
    on_cleanup(move || handle.remove());
    view! { {move || match state.dialog.get() {
        DialogKind::None => ().into_any(),
        DialogKind::Ticket => view! { <CreateTicket/> }.into_any(),
        DialogKind::Project => view! { <CreateProject/> }.into_any(),
        DialogKind::Dependency => view! { <AddDependency/> }.into_any(),
        DialogKind::Command => view! { <CommandPalette/> }.into_any(),
    }} }
}

#[component]
fn CreateTicket() -> impl IntoView {
    let state = expect_context::<DesignState>();
    let title = RwSignal::new(String::new());
    let status = RwSignal::new(state.new_status.get_untracked().name().to_string());
    let description = RwSignal::new(String::new());
    let navigate = use_navigate();
    view! {
        <Modal title="New ticket" on_close=Callback::new(move |_| state.dialog.set(DialogKind::None))>
            <form autocomplete="off" data-1p-ignore="true" data-form-type="other" on:submit=move |ev| {
                ev.prevent_default();
                let value = title.get_untracked().trim().to_string();
                if value.is_empty() { return; }
                let id = state.tickets.get_untracked().iter().map(|t| t.id).max().unwrap_or(0) + 1;
                state.tickets.update(|tickets| { tickets.push(Ticket { id, title: value, status: Status::from_name(&status.get_untracked()), priority: Priority::None, label: "Feature", comments: 0, blocked: false, description: description.get_untracked(), project: state.current_project.get_untracked(), extra_labels: vec![], added_comments: vec![], blocked_by: vec![] }); });
                state.dialog.set(DialogKind::None); state.toast.set(format!("TKT-{id} created"));
                navigate(&format!("/design/ticket?id={id}"), Default::default());
            }>
                <div class="modal-body"><div class="form-project"><Icon name="project"/>{move || state.current_project.get()}<Icon name="chevron"/>"New ticket"</div><TextInput id="ticket-title" label="Title" placeholder="What needs to be done?" value=title/><label class="field">"Description"<textarea autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  rows="5" placeholder="Add details… Markdown is supported." prop:value=move || description.get() on:input=move |ev| description.set(event_target_value(&ev))/></label><label class="field">"Status"<Select label="Ticket status" options=Status::ALL.map(Status::name).to_vec() value=status/></label></div>
                <div class="modal-footer"><Button variant="ghost" on_click=Callback::new(move |_| state.dialog.set(DialogKind::None))>"Cancel"</Button><button class="button button-primary" type="submit" disabled=move || title.get().trim().is_empty()>"Create ticket"</button></div>
            </form>
        </Modal>
    }
}

#[component]
fn CreateProject() -> impl IntoView {
    let state = expect_context::<DesignState>();
    let name = RwSignal::new(String::new());
    let navigate = use_navigate();
    view! {
        <Modal title="New project" on_close=Callback::new(move |_| state.dialog.set(DialogKind::None))>
            <form autocomplete="off" data-1p-ignore="true" data-form-type="other" on:submit=move |ev| { ev.prevent_default(); let value = name.get_untracked().trim().to_string(); if !value.is_empty() { state.current_project.set(value.clone()); state.projects.update(|projects| projects.push(value)); state.dialog.set(DialogKind::None); state.toast.set("Project created".into()); navigate("/design/empty", Default::default()); } }>
                <div class="modal-body"><TextInput id="project-title" label="Project name" placeholder="e.g. Quantum" value=name/><p class="muted">"Group related tickets in a project. You can link a GitHub project later."</p></div><div class="modal-footer"><Button variant="ghost" on_click=Callback::new(move |_| state.dialog.set(DialogKind::None))>"Cancel"</Button><button class="button button-primary" type="submit" disabled=move || name.get().trim().is_empty()>"Create project"</button></div>
            </form>
        </Modal>
    }
}

#[component]
fn AddDependency() -> impl IntoView {
    let state = expect_context::<DesignState>();
    let query = RwSignal::new(String::new());
    let direction = RwSignal::new("Is blocked by".to_string());
    view! {
        <Modal title="Add dependency" on_close=Callback::new(move |_| state.dialog.set(DialogKind::None))>
            <div class="modal-body">
                <div class="dependency-direction"><span class="mono">{move || format!("TKT-{}", state.selected_ticket.get())}</span><Select label="Dependency direction" options=vec!["Is blocked by", "Blocks"] value=direction/></div>
                <p class="muted">{move || if direction.get() == "Is blocked by" { "The selected ticket must be finished first." } else { "This ticket must be finished before the selected ticket." }}</p>
                <TextInput id="dependency-search" label="Find a ticket" placeholder="Search by title or ID…" value=query/>
                <div class="dependency-results">{move || {
                    let selected = state.selected_ticket.get();
                    let tickets = state.tickets.get();
                    let graph = DependencyGraph::new(tickets.iter().flat_map(|ticket| ticket.blocked_by.iter().map(|blocker| Dependency { blocker_id: i64::from(*blocker), blocked_id: i64::from(ticket.id) }))).expect("valid fixture graph");
                    tickets.iter().filter(|t| t.id != selected && format!("TKT-{} {}", t.id, t.title).to_lowercase().contains(&query.get().to_lowercase())).take(6).map(|t| {
                        let id = t.id;
                        let (dependent, prerequisite) = if direction.get() == "Is blocked by" { (selected, id) } else { (id, selected) };
                        let exists = tickets.iter().any(|t| t.id == dependent && t.blocked_by.contains(&prerequisite));
                        let cycle = graph.check(Dependency { blocker_id: i64::from(prerequisite), blocked_id: i64::from(dependent) }) == Err(DependencyError::Cycle);
                        view! { <button class="dependency-option" disabled=exists || cycle on:click=move |_| {
                            state.tickets.update(|tickets| { if let Some(t) = tickets.iter_mut().find(|t| t.id == dependent) { t.blocked_by.push(prerequisite); } });
                            state.dialog.set(DialogKind::None);
                            state.toast.set(format!("TKT-{dependent} is blocked by TKT-{prerequisite}"));
                        }><StatusIcon status=t.status/><span class="mono muted">{format!("TKT-{id}")}</span><span>{t.title.clone()}</span>{if exists { "Added" } else if cycle { "Circular" } else { "+" }}</button> }
                    }).collect_view()
                }}</div>
            </div>
        </Modal>
    }
}

#[component]
pub fn CommandPalette() -> impl IntoView {
    let state = expect_context::<DesignState>();
    let query = RwSignal::new(String::new());
    let board = expect_context::<crate::pages::board::BoardState>();
    let location = leptos_router::hooks::use_location();
    if !location.pathname.get_untracked().starts_with("/design")
        && board.signed_in.get_untracked()
        && board.projects.get_untracked().is_empty()
    {
        board.refresh();
    }
    view! {
        <Modal title="Search and commands" on_close=Callback::new(move |_| state.dialog.set(DialogKind::None))>
            <div class="command-palette" on:keydown=command_key>
            <div class="command-search"><Icon name="search"/><input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  aria-label="Search tickets or commands" placeholder="Search tickets…" on:input=move |ev| query.set(event_target_value(&ev))/></div>
            <Show when=move||!location.pathname.get().starts_with("/design") fallback=move||view!{<div class="command-results"><p class="menu-heading">"Actions"</p><button class="menu-item" on:click=move |_| state.new_ticket(Status::Todo)><Icon name="plus"/>"New ticket"<Shortcut>"C"</Shortcut></button><button class="menu-item" on:click=move |_| state.dialog.set(DialogKind::Project)><Icon name="project"/>"New project"</button><p class="menu-heading">"Tickets"</p>{move || {
                let filtered = state.tickets.get().into_iter().filter(|t| format!("TKT-{} {}", t.id, t.title).to_lowercase().contains(&query.get().to_lowercase())).take(6).collect::<Vec<_>>();
                if filtered.is_empty() { view! { <p class="command-empty">"No tickets match your search."</p> }.into_any() } else { filtered.into_iter().map(|ticket| view! { <A href=format!("/design/ticket?id={}", ticket.id) attr:class="menu-item" on:click=move |_| state.dialog.set(DialogKind::None)><StatusIcon status=ticket.status/><span class="mono muted">{format!("TKT-{}", ticket.id)}</span><span>{ticket.title}</span></A> }).collect_view().into_any() }
            }}</div>}><div class="command-results"><p class="menu-heading">"Tickets"</p>{move||{
                let filtered=board.tickets.get().into_iter().filter(|t|format!("{} {}",t.key,t.title).to_lowercase().contains(&query.get().to_lowercase())).take(20).collect::<Vec<_>>();
                if filtered.is_empty() {view!{<p class="command-empty" role="status">{move||if board.loading.get(){"Loading tickets…"}else{"No tickets match your search."}}</p>}.into_any()} else {filtered.into_iter().map(|t|view!{<A href=format!("/ticket?id={}",t.id) attr:class="menu-item" on:click=move|_|state.dialog.set(DialogKind::None)><StatusIcon status=t.status/><span class="mono muted">{t.key}</span><span class="menu-title" title=t.title.clone()>{t.title.clone()}</span></A>}).collect_view().into_any()}
            }}</div></Show><div class="command-footer"><span><Shortcut>"↑ ↓"</Shortcut>" to navigate"</span><span><Shortcut>"↵"</Shortcut>" to open"</span><span><Shortcut>"esc"</Shortcut>" to close"</span></div>
            </div>
        </Modal>
    }
}

fn command_key(event: web_sys::KeyboardEvent) {
    let key = event.key();
    if !["ArrowDown", "ArrowUp"].contains(&key.as_str()) {
        return;
    }
    let Ok(items) = document().query_selector_all(".command-results .menu-item") else {
        return;
    };
    let len = items.length();
    if len == 0 {
        return;
    }
    event.prevent_default();
    let active = document().active_element();
    let index = (0..len).find(|i| {
        items.item(*i).is_some_and(|item| {
            active
                .as_ref()
                .is_some_and(|active| item.is_same_node(Some(active)))
        })
    });
    let next = match (key.as_str(), index) {
        ("ArrowUp", Some(i)) => (i + len - 1) % len,
        ("ArrowUp", None) => len - 1,
        (_, Some(i)) => (i + 1) % len,
        _ => 0,
    };
    if let Some(item) = items
        .item(next)
        .and_then(|item| item.dyn_into::<web_sys::HtmlElement>().ok())
    {
        let _ = item.focus();
    }
}

#[component]
pub fn Toast() -> impl IntoView {
    let state = expect_context::<DesignState>();
    view! { <Show when=move || !state.toast.get().is_empty()><div class="toast" role="status" aria-live="polite"><Icon name="check"/><span>{move || state.toast.get()}</span><IconButton label="Dismiss notification" icon="close" on_click=Callback::new(move |_| state.toast.set(String::new()))/></div></Show> }
}
