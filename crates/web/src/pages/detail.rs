use crate::{
    components::*,
    fixtures::{Priority, Status, Ticket},
};
use leptos::prelude::*;
use leptos_router::{components::A, hooks::use_query_map};

#[component]
pub fn TicketDetail() -> impl IntoView {
    let state = expect_context::<DesignState>();
    let query = use_query_map();
    view! {
        <For each=move || {
            let id = query.read().get("id").and_then(|id| id.parse::<u32>().ok()).unwrap_or(7);
            state.tickets.get().into_iter().filter(|ticket| ticket.id == id).collect::<Vec<_>>()
        } key=|ticket| ticket.id children=|ticket| view! { <TicketContent ticket/> }/>
    }
}

#[component]
fn TicketContent(ticket: Ticket) -> impl IntoView {
    let state = expect_context::<DesignState>();
    let id = ticket.id;
    state.selected_ticket.set(id);
    state.current_project.set(ticket.project.clone());
    let status = RwSignal::new(ticket.status.name().to_string());
    let priority = RwSignal::new(ticket.priority.name().to_string());
    let comment = RwSignal::new(String::new());
    let current = Memo::new(move |_| {
        state
            .tickets
            .get()
            .into_iter()
            .find(|t| t.id == id)
            .expect("fixture ticket")
    });
    view! {
        <TopBar detail=true/>
        <div class="detail-layout">
            <article class="ticket-detail">
                <div class="detail-overline"><A href="/design/board" attr:class="back-link"><Icon name="board"/>"Board"</A><span class="breadcrumb-slash">"/"</span><span class="mono">{format!("TKT-{id}")}</span></div>
                <h1>{ticket.title}</h1>
                <div class="detail-description"><Markdown source=ticket.description/></div>
                <section class="dependencies" aria-label="Dependencies">
                    <div class="section-heading"><h2>"Dependencies"</h2><Button variant="ghost" on_click=Callback::new(move |_| state.dialog.set(DialogKind::Dependency))><Icon name="plus"/>"Add dependency"</Button></div>
                    {move || {
                        let tickets = state.tickets.get();
                        let blocked_by = current.get().blocked_by;
                        let rows = tickets.into_iter().filter_map(|t| {
                            let kind = if blocked_by.contains(&t.id) { "Blocked by" } else if t.blocked_by.contains(&id) { "Blocks" } else { return None; };
                            Some(view! { <A href=format!("/design/ticket?id={}", t.id) attr:class="dependency-row">
                                <span class="dependency-kind"><Icon name=if kind == "Blocks" { "arrow" } else { "blocked" }/>{kind}</span>
                                <StatusIcon status=t.status/><span class="mono muted">{format!("TKT-{}", t.id)}</span>
                                <span class="dependency-title">{t.title.clone()}</span>
                                {(t.status == Status::Done).then(|| view! { <span class="resolved">"Resolved"</span> })}
                                <Icon name="chevron"/>
                            </A> })
                        }).collect::<Vec<_>>();
                        if rows.is_empty() { view! { <p class="muted dependency-empty">"No dependencies. This ticket can be worked on independently."</p> }.into_any() } else { rows.collect_view().into_any() }
                    }}
                </section>
                <section class="activity" aria-label="Comments and activity">
                    <div class="section-heading"><h2>"Activity"</h2><span class="muted">"Today"</span></div>
                    <div class="activity-event"><StatusIcon status=ticket.status/><span>"You set the status to "<strong>{ticket.status.name()}</strong></span><time>"10:24"</time></div>
                    {(id == 7).then(|| view! {
                        <Comment author="You" initials="Y" time="10:32" text="The status menu should work everywhere the ticket appears. Dragging can stay a convenience."/>
                        <Comment author="Agent" initials="A" time="10:41" text="Agreed. I’ll preserve **keyboard focus** and announce the new status after a move."/>
                    })}
                    {move || current.get().added_comments.into_iter().map(|text| view! { <Comment author="You" initials="Y" time="Just now" text/> }).collect_view()}
                    <form class="comment-form" on:submit=move |ev| {
                        ev.prevent_default();
                        let value = comment.get_untracked().trim().to_string();
                        if !value.is_empty() {
                            state.tickets.update(|tickets| { if let Some(t) = tickets.iter_mut().find(|t| t.id == id) { t.added_comments.push(value); t.comments += 1; } });
                            comment.set(String::new());
                            state.toast.set("Comment added".into());
                        }
                    }>
                        <textarea aria-label="Write a comment" rows="2" placeholder="Leave a comment…" prop:value=move || comment.get() on:input=move |ev| comment.set(event_target_value(&ev))/>
                        <div><span>"Markdown supported"</span><button class="button button-secondary" type="submit" disabled=move || comment.get().trim().is_empty()>"Comment"<Icon name="arrow"/></button></div>
                    </form>
                </section>
            </article>
            <aside class="properties" aria-label="Ticket properties">
                <h2>"Properties"</h2>
                <div class="property"><span>"Status"</span><div>{move || view! { <StatusIcon status=Status::from_name(&status.get())/> }}<Select label="Status" options=Status::ALL.map(Status::name).to_vec() value=status on_change=Callback::new(move |value: String| state.move_ticket(id, Status::from_name(&value)))/></div></div>
                <div class="property"><span>"Priority"</span><div>{move || view! { <PriorityIcon priority=Priority::ALL.into_iter().find(|p| p.name() == priority.get()).unwrap_or(Priority::None)/> }}<Select label="Priority" options=Priority::ALL.map(Priority::name).to_vec() value=priority on_change=Callback::new(move |value: String| state.tickets.update(|tickets| { if let Some(t) = tickets.iter_mut().find(|t| t.id == id) { t.priority = Priority::ALL.into_iter().find(|p| p.name() == value).unwrap_or(Priority::None); } }))/></div></div>
                <div class="property-section">
                    <h2>"Labels"</h2>
                    <div class="property-labels"><LabelChip name=ticket.label/>{move || current.get().extra_labels.into_iter().map(|name| view! { <LabelChip name/> }).collect_view()}</div>
                    <Menu label="Add label"><span class="menu-heading">"Labels"</span>
                        {["Feature", "Bug", "Infrastructure", "Design"].into_iter().filter(|label| *label != ticket.label).map(move |label| view! {
                            <button class="menu-item" aria-pressed=move || current.get().extra_labels.iter().any(|l| l == label) on:click=move |_| {
                                state.tickets.update(|tickets| { if let Some(t) = tickets.iter_mut().find(|t| t.id == id) { if t.extra_labels.iter().any(|l| l == label) { t.extra_labels.retain(|l| l != label); } else { t.extra_labels.push(label.into()); } } });
                            }><LabelChip name=label/>{move || current.get().extra_labels.iter().any(|l| l == label).then(|| view! { <Icon name="check"/> })}</button>
                        }).collect_view()}
                    </Menu>
                </div>
                <div class="property-section"><h2>"Project"</h2><A href="/design/board" attr:class="property-project"><span class="project-mark">"T"</span><span>{ticket.project}</span><Icon name="chevron"/></A></div>
                <div class="property-section"><h2>"Links"</h2><a class="subtle-link" href=format!("https://github.com/0x63616c/tickets/issues/{id}") target="_blank" rel="noreferrer"><Icon name="link"/>"GitHub issue"<Icon name="external"/></a></div>
                <div class="property-section property-dates"><span>"Created Oct 24"</span><span>"Updated today"</span></div>
            </aside>
        </div>
    }
}

#[component]
fn Comment(
    author: &'static str,
    initials: &'static str,
    time: &'static str,
    #[prop(into)] text: String,
) -> impl IntoView {
    view! { <div class="comment"><Avatar label=author initials/><div class="comment-content"><div class="comment-heading"><strong>{author}</strong>{(author == "Agent").then(|| view! { <span class="agent-badge">"API"</span> })}<time>{time}</time></div><Markdown source=text/></div></div> }
}
