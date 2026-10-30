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
    let navigate = leptos_router::hooks::use_navigate();
    let open =
        Callback::new(move |_| navigate(&format!("/design/ticket?id={id}"), Default::default()));
    let was_dragged = RwSignal::new(false);
    let title_tooltip = ticket.title.clone();
    let title_label = format!("TKT-{id}: {}", ticket.title);
    let labels = std::iter::once(ticket.label.to_string())
        .chain(ticket.extra_labels)
        .enumerate()
        .map(|(i, name)| {
            let color = match name.as_str() {
                "Feature" => "#8b82d9",
                "Bug" => "#eb5757",
                "Infrastructure" => "#4ea7fc",
                "Design" => "#c6a1b9",
                _ => "#8a8f98",
            };
            tickets_core::domain::Label {
                id: i as i64,
                name: name.clone(),
                color: color.into(),
            }
        })
        .collect();
    view! {
        <article class="ticket-card" data-motion-key=format!("fixture-{id}") tabindex="0" aria-label=title_label draggable="true" class:drag-over=move||state.dragged.get()==Some(id)
            on:pointerdown=move|_|was_dragged.set(false)
            on:click=move|e|{if !was_dragged.get_untracked() && !ticket_interactive_target(e.target()){open.run(());}}
            on:keydown=move|e|{if e.key()=="Enter" && !ticket_interactive_target(e.target()) && state.dragged.get_untracked().is_none(){e.prevent_default();open.run(());}}
            on:dragstart=move |ev| { was_dragged.set(true);state.dragged.set(Some(id)); if let Some(data) = ev.data_transfer() { let _ = data.set_data("text/plain", &id.to_string()); } } on:dragend=move |_| state.dragged.set(None)>
            <div class="card-top"><span class="mono ticket-id">{format!("TKT-{id}")}</span><span class="card-indicators"><span title=ticket.priority.name()><PriorityIcon priority=ticket.priority/></span><span class="card-status" title=ticket.status.name()><SelectField label=format!("Move TKT-{id} to status") value=Signal::derive(move||ticket.status.name().to_string()) options=Signal::derive(move||Status::ALL.into_iter().map(SelectOption::status).collect()) on_change=Callback::new(move|next:String|state.move_ticket(id,Status::from_name(&next)))/></span></span></div>
            <A href=format!("/design/ticket?id={id}") attr:class="card-title" attr:title=title_tooltip>{ticket.title.clone()}</A>
            {ticket.blocked.then(|| view! { <span class="blocked-note" title="Blocked by TKT-5"><Icon name="blocked"/><span>"Blocked by TKT-5"</span></span> })}
            <div class="card-bottom"><CardLabels labels/><span class="card-meta">{(ticket.comments > 0).then(|| view! { <span class="comment-count" aria-label=crate::count::count_label(ticket.comments as usize, "comment")><Icon name="comment"/>{ticket.comments}</span> })}</span></div>
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
        <A href=format!("/design/ticket?id={}", ticket.id) attr:class="ticket-row" attr:data-motion-key=format!("fixture-{}",ticket.id)>
            <span title=ticket.priority.name()><PriorityIcon priority=ticket.priority/></span><span class="mono ticket-id">{format!("TKT-{}", ticket.id)}</span><StatusIcon status=ticket.status/><span class="row-title">{ticket.title}</span>
            {ticket.blocked.then(|| view! { <span class="row-blocked" title="Blocked by TKT-5"><Icon name="blocked"/>"Blocked"</span> })}
            <LabelChip name=ticket.label/><span class="row-comments">{(ticket.comments > 0).then(|| view! { <Icon name="comment"/>{ticket.comments} })}</span>
            <span class="row-project"><ProjectIcon name=ticket.project.clone()/>{ticket.project}</span><span class="row-date">{match ticket.id % 4 { 0 => "Oct 24", 1 => "Oct 25", 2 => "Oct 26", _ => "Oct 27" }}</span>
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
                <A href="/design/board" attr:class="project-row" on:click=move |_| state.current_project.set("Tickets".into())><div><ProjectIcon name="Tickets" icon="ticket"/><div><strong>"Tickets"</strong><p>"Projects, tickets, and dependencies"</p></div></div><span class="muted">{move || state.tickets.get().iter().filter(|t| t.project == "Tickets").count()}</span><span class="muted">"0x63616c/tickets"<Icon name="external"/></span></A>
                <A href="/design/empty" attr:class="project-row" on:click=move |_| state.current_project.set("Quantum".into())><div><ProjectIcon name="Quantum" icon="box" color="#4ea7fc"/><div><strong>"Quantum"</strong><p>"Voxel engine and editor"</p></div></div><span class="muted">"0"</span><span class="muted">"Not linked"</span></A>
                {move || state.projects.get().into_iter().map(|name| { let project = name.clone(); view! { <A href="/design/empty" attr:class="project-row" on:click=move |_| state.current_project.set(project.clone())><div><Icon name="project"/><strong>{name}</strong></div><span class="muted">"0"</span><span class="muted">"Not linked"</span></A> } }).collect_view()}
            </div>
        </div>
    }
}
