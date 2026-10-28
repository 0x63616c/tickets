use crate::{
    components::*,
    fixtures::{Status, Ticket},
};
use leptos::prelude::*;
use leptos_router::components::A;

#[component]
fn ViewToolbar(query: RwSignal<String>, #[prop(default = false)] board: bool) -> impl IntoView {
    view! {
        <div class="view-toolbar">
            <div class="filter-input"><Icon name="filter"/><input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  aria-label="Filter tickets" placeholder="Filter tickets…" prop:value=move || query.get() on:input=move |ev| query.set(event_target_value(&ev))/><Show when=move || !query.get().is_empty()><IconButton label="Clear filter" icon="close" on_click=Callback::new(move |_| query.set(String::new()))/></Show></div>
            <span class="toolbar-caption">{if board { "Drag tickets to change status" } else { "All tickets" }}</span><span class="toolbar-divider"/><span class="display-label">"Grouped by status"</span>
        </div>
    }
}

fn matches(ticket: &Ticket, query: &str, project: &str) -> bool {
    ticket.project == project
        && format!(
            "TKT-{} {} {} {}",
            ticket.id,
            ticket.title,
            ticket.label,
            ticket.extra_labels.join(" ")
        )
        .to_lowercase()
        .contains(&query.to_lowercase())
}

#[component]
pub fn Board() -> impl IntoView {
    let state = expect_context::<DesignState>();
    let query = RwSignal::new(String::new());
    view! {
        <h1 class="sr-only">"Board preview"</h1><TopBar/><ProjectHeader/><ViewToolbar query board=true/>
        <div class="board" aria-label="Ticket board">{Status::ALL.into_iter().map(|status| {
            let over = RwSignal::new(false);
            view! {
                <Show when=move || !matches!(status, Status::InReview | Status::Canceled) || state.tickets.get().iter().any(|t| t.status == status)>
                    <section class="board-column" class:drag-over=move || over.get() aria-label=status.name()
                        on:dragover=move |ev| { ev.prevent_default(); over.set(true); }
                        on:dragleave=move |_| over.set(false)
                        on:drop=move |ev| { ev.prevent_default(); over.set(false); if let Some(id) = state.dragged.get_untracked() { state.move_ticket(id, status); state.dragged.set(None); } }>
                        <div class="column-heading"><StatusIcon status/><h2>{status.name()}</h2><span class="count">{move || state.tickets.get().iter().filter(|t| t.status == status && matches(t, &query.get(), &state.current_project.get())).count()}</span><button class="icon-button" aria-label=format!("Add ticket to {}", status.name()) on:click=move |_| state.new_ticket(status)><Icon name="plus"/></button></div>
                        <div class="column-cards"><For each=move || { state.tickets.get().into_iter().filter(|t| t.status == status && matches(t, &query.get(), &state.current_project.get())).collect::<Vec<_>>() } key=|ticket| (ticket.id, ticket.label, ticket.extra_labels.clone(), ticket.comments) children=move |ticket| view! { <TicketCard ticket/> }/></div>
                        <button class="add-card" on:click=move |_| state.new_ticket(status)><Icon name="plus"/>"New ticket"</button>
                    </section>
                </Show>
            }
        }).collect_view()}</div>
        <ViewFooter query/>
    }
}

#[component]
pub fn TicketCard(ticket: Ticket) -> impl IntoView {
    let state = expect_context::<DesignState>();
    let id = ticket.id;
    view! {
        <article class="ticket-card" draggable="true" on:dragstart=move |ev| { state.dragged.set(Some(id)); if let Some(data) = ev.data_transfer() { let _ = data.set_data("text/plain", &id.to_string()); } } on:dragend=move |_| state.dragged.set(None)>
            <div class="card-top"><span class="mono ticket-id">{format!("TKT-{id}")}</span><span title=ticket.priority.name()><PriorityIcon priority=ticket.priority/></span></div>
            <A href=format!("/design/ticket?id={id}") attr:class="card-title">{ticket.title}</A>
            {ticket.blocked.then(|| view! { <span class="blocked-note"><Icon name="blocked"/>"Blocked by TKT-5"</span> })}
            <div class="card-labels"><LabelChip name=ticket.label/>{ticket.extra_labels.into_iter().map(|name| view! { <LabelChip name/> }).collect_view()}</div>
            <div class="card-bottom">
                <div class="card-status"><StatusIcon status=ticket.status/><select autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  aria-label=format!("Move TKT-{id} to status") prop:value=ticket.status.name() on:change=move |ev| state.move_ticket(id, Status::from_name(&event_target_value(&ev)))>{Status::ALL.into_iter().map(|s| view! { <option value=s.name() selected=s == ticket.status>{s.name()}</option> }).collect_view()}</select></div>
                <div class="card-meta">{(ticket.comments > 0).then(|| view! { <span class="comment-count" aria-label=crate::count::count_label(ticket.comments as usize, "comment")><Icon name="comment"/>{ticket.comments}</span> })}</div>
            </div>
        </article>
    }
}

#[component]
pub fn List() -> impl IntoView {
    let state = expect_context::<DesignState>();
    let query = RwSignal::new(String::new());
    view! {
        <h1 class="sr-only">"Ticket list preview"</h1><TopBar/><ProjectHeader view="list"/><ViewToolbar query/>
        <div class="ticket-list" aria-label="Tickets grouped by status">{Status::ALL.into_iter().map(|status| view! {
            <Show when=move || !matches!(status, Status::InReview | Status::Canceled) || state.tickets.get().iter().any(|t| t.status == status)>
                <section class="list-group" aria-label=status.name()>
                    <div class="list-group-heading"><StatusIcon status/><h2>{status.name()}</h2><span class="count">{move || state.tickets.get().iter().filter(|t| t.status == status && matches(t, &query.get(), &state.current_project.get())).count()}</span><button class="list-add" aria-label=format!("Add ticket to {}", status.name()) on:click=move |_| state.new_ticket(status)><Icon name="plus"/></button></div>
                    <For each=move || { state.tickets.get().into_iter().filter(|t| t.status == status && matches(t, &query.get(), &state.current_project.get())).collect::<Vec<_>>() } key=|ticket| ticket.id children=|ticket| view! { <TicketRow ticket/> }/>
                </section>
            </Show>
        }).collect_view()}</div>
        <ViewFooter query/>
    }
}

#[component]
fn TicketRow(ticket: Ticket) -> impl IntoView {
    view! {
        <A href=format!("/design/ticket?id={}", ticket.id) attr:class="ticket-row">
            <span title=ticket.priority.name()><PriorityIcon priority=ticket.priority/></span><span class="mono ticket-id">{format!("TKT-{}", ticket.id)}</span><StatusIcon status=ticket.status/><span class="row-title">{ticket.title}</span>
            {ticket.blocked.then(|| view! { <span class="row-blocked" title="Blocked by TKT-5"><Icon name="blocked"/>"Blocked"</span> })}
            <LabelChip name=ticket.label/><span class="row-comments">{(ticket.comments > 0).then(|| view! { <Icon name="comment"/>{ticket.comments} })}</span>
            <span class="row-project"><Icon name="project"/>{ticket.project}</span><span class="row-date">{match ticket.id % 4 { 0 => "Oct 24", 1 => "Oct 25", 2 => "Oct 26", _ => "Oct 27" }}</span>
        </A>
    }
}

#[component]
fn ViewFooter(query: RwSignal<String>) -> impl IntoView {
    let state = expect_context::<DesignState>();
    view! { <div class="view-footer"><span>{move || crate::count::count_label(state.tickets.get().iter().filter(|t| matches(t, &query.get(), &state.current_project.get())).count(), "ticket")}</span><span>"Create a ticket"<Shortcut>"C"</Shortcut></span></div> }
}

#[component]
pub fn Empty() -> impl IntoView {
    let state = expect_context::<DesignState>();
    if state.current_project.get_untracked() == "Tickets" {
        state.current_project.set("Quantum".into());
    }
    view! {
        <Show when=move || !state.tickets.get().iter().any(|t| t.project == state.current_project.get()) fallback=|| view! { <Board/> }><h1 class="sr-only">"Empty project preview"</h1><TopBar title="Project"/>
        <div class="view-tabs"><span class="view-tab selected"><Icon name="list"/>"Tickets"</span><span class="muted">{move || state.current_project.get()}</span></div>
        <div class="empty-project">
            <EmptyState title="No tickets yet" description="Add the first ticket to this project." action="New ticket" icon="ticket" on_action=Callback::new(move |_| state.new_ticket(Status::Todo))/>
        </div>
        <div class="empty-board" aria-label="Empty board">{Status::BOARD.into_iter().map(|status| view! { <div><div class="column-heading"><StatusIcon status/><h2>{status.name()}</h2><span class="count">"0"</span></div><button class="empty-column-add" on:click=move |_| state.new_ticket(status)><Icon name="plus"/>"New ticket"</button></div> }).collect_view()}</div></Show>
    }
}

#[component]
pub fn Projects() -> impl IntoView {
    let state = expect_context::<DesignState>();
    view! {
        <TopBar title="Projects"/>
        <div class="projects-page">
            <div class="projects-heading"><h1>"Projects"</h1><span class="muted">{move || crate::count::count_label(state.projects.get().len() + 2, "project")}</span></div>
            <div class="projects-table"><div class="projects-table-head"><span>"Name"</span><span>"Tickets"</span><span>"GitHub"</span></div>
                <A href="/design/board" attr:class="project-row" on:click=move |_| state.current_project.set("Tickets".into())><div><span class="project-mark">"T"</span><div><strong>"Tickets"</strong><p>"Projects, tickets, and dependencies"</p></div></div><span class="muted">{move || state.tickets.get().iter().filter(|t| t.project == "Tickets").count()}</span><span class="muted">"0x63616c/tickets"<Icon name="external"/></span></A>
                <A href="/design/empty" attr:class="project-row" on:click=move |_| state.current_project.set("Quantum".into())><div><span class="project-mark">"Q"</span><div><strong>"Quantum"</strong><p>"Voxel engine and editor"</p></div></div><span class="muted">"0"</span><span class="muted">"Not linked"</span></A>
                {move || state.projects.get().into_iter().map(|name| { let project = name.clone(); view! { <A href="/design/empty" attr:class="project-row" on:click=move |_| state.current_project.set(project.clone())><div><Icon name="project"/><strong>{name}</strong></div><span class="muted">"0"</span><span class="muted">"Not linked"</span></A> } }).collect_view()}
            </div>
        </div>
    }
}
