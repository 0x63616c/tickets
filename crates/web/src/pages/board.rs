//! API-backed board. The fixture routes deliberately retain their own state.
use crate::{
    components::*,
    fixtures::{Priority, Status},
    ui::api,
};
use leptos::{ev, prelude::*};
use serde_json::{Value, json};
use wasm_bindgen_futures::spawn_local;
use web_sys::wasm_bindgen::JsCast;

use serde::{Deserialize, de::DeserializeOwned};
use tickets_core::domain::{Label, Project};

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub(crate) struct BoardTicket {
    pub id: i64,
    pub project_id: i64,
    pub key: String,
    pub title: String,
    pub status: Status,
    pub priority: Priority,
    pub assignee: Option<String>,
    pub sort_order: f64,
    pub labels: Vec<Label>,
    pub blocked_by: Vec<i64>,
    pub comment_count: Option<usize>,
    pub created_at: String,
    pub updated_at: String,
    pub body: String,
    pub blocks: Vec<i64>,
    #[serde(default)]
    pub external_url: Option<String>,
    #[serde(default)]
    pub external_id: Option<String>,
}
fn id(t: &BoardTicket) -> i64 {
    t.id
}
fn status(t: &BoardTicket) -> Status {
    t.status
}
pub(crate) async fn all<T: DeserializeOwned>(path: &str) -> Result<Vec<T>, String> {
    #[derive(Deserialize)]
    struct Page<T> {
        data: Vec<T>,
        next_cursor: Option<String>,
    }
    let mut items = vec![];
    let mut cursor = String::new();
    loop {
        let response = api(
            "GET",
            &format!("/api/v1/{path}?limit=100{cursor}"),
            Value::Null,
        )
        .await?;
        let page: Page<T> = serde_json::from_value(response)
            .map_err(|e| format!("Invalid {path} response: {e}"))?;
        items.extend(page.data);
        match page.next_cursor {
            Some(c) => cursor = format!("&cursor={c}"),
            None => return Ok(items),
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) struct BoardState {
    pub(crate) tickets: RwSignal<Vec<BoardTicket>>,
    pub(crate) projects: RwSignal<Vec<Project>>,
    pub(crate) labels: RwSignal<Vec<Label>>,
    pub(crate) project: RwSignal<i64>,
    label: RwSignal<String>,
    assignee: RwSignal<String>,
    query: RwSignal<String>,
    pub(crate) busy: RwSignal<bool>,
    pub(crate) loading: RwSignal<bool>,
    pub(crate) refreshing: RwSignal<bool>,
    pub(crate) error: RwSignal<String>,
    pub(crate) notice: RwSignal<String>,
    pub(crate) signed_in: RwSignal<bool>,
    pub(crate) dragged: RwSignal<Option<i64>>,
    pub(crate) live_revision: RwSignal<u64>,
    pub(crate) catchup_revision: RwSignal<u64>,
    pub(crate) connection: RwSignal<ConnectionState>,
    snapshot: RwSignal<Vec<BoardTicket>>,
    pub(crate) composer: RwSignal<Option<Status>>,
}
impl BoardState {
    pub(crate) fn new() -> Self {
        Self {
            tickets: RwSignal::new(vec![]),
            projects: RwSignal::new(vec![]),
            labels: RwSignal::new(vec![]),
            project: RwSignal::new(0),
            label: RwSignal::new(String::new()),
            assignee: RwSignal::new(String::new()),
            query: RwSignal::new(String::new()),
            busy: RwSignal::new(false),
            loading: RwSignal::new(true),
            refreshing: RwSignal::new(false),
            error: RwSignal::new(String::new()),
            notice: expect_context::<DesignState>().toast,
            signed_in: RwSignal::new(false),
            dragged: RwSignal::new(None),
            live_revision: RwSignal::new(0),
            catchup_revision: RwSignal::new(0),
            connection: RwSignal::new(ConnectionState::Offline),
            snapshot: RwSignal::new(vec![]),
            composer: RwSignal::new(None),
        }
    }
    pub(crate) fn select_project(self, project: i64) -> bool {
        if self.busy.get_untracked() {
            return false;
        }
        self.cancel();
        self.project.set(project);
        self.label.set(String::new());
        self.assignee.set(String::new());
        self.query.set(String::new());
        true
    }
    pub(crate) fn refresh(self) {
        self.fetch(false);
    }
    pub(crate) fn catch_up(self) {
        self.fetch(true);
    }
    fn fetch(self, background: bool) {
        if self.refreshing.get_untracked() {
            return;
        }
        let snapshot = self.tickets.get_untracked();
        self.refreshing.set(true);
        if !background {
            self.loading.set(true);
        }
        self.error.set(String::new());
        spawn_local(async move {
            let result = async {
                Ok::<_, String>((
                    all::<Project>("projects").await?,
                    all::<BoardTicket>("tickets").await?,
                    all::<Label>("labels").await?,
                ))
            }
            .await;
            match result {
                Ok((projects, tickets, labels)) => {
                    if background
                        && (self.busy.get_untracked()
                            || self.dragged.get_untracked().is_some()
                            || self.composer.get_untracked().is_some()
                            || self.tickets.get_untracked() != snapshot
                            || crate::live::editing())
                    {
                        self.loading.set(false);
                        self.refreshing.set(false);
                        self.live_revision.update(|r| *r += 1);
                        return;
                    }
                    self.signed_in.set(true);
                    if !projects
                        .iter()
                        .any(|p| p.id == self.project.get_untracked())
                    {
                        self.project
                            .set(projects.first().map(|p| p.id).unwrap_or(0));
                    }
                    self.projects.set(projects);
                    self.tickets.set(tickets);
                    self.labels.set(labels);
                }
                Err(e) => self.error.set(e),
            }
            self.loading.set(false);
            self.refreshing.set(false);
        });
    }
    fn visible(self, t: &BoardTicket) -> bool {
        t.project_id == self.project.get()
            && (self.label.get().is_empty()
                || t.labels
                    .iter()
                    .any(|l| l.id.to_string() == self.label.get()))
            && (self.assignee.get().is_empty()
                || if self.assignee.get() == "__unassigned" {
                    t.assignee.is_none()
                } else {
                    t.assignee.as_deref() == Some(self.assignee.get().as_str())
                })
            && format!("{} {}", t.key, t.title)
                .to_lowercase()
                .contains(&self.query.get().to_lowercase())
    }
    fn column(self, s: Status, filtered: bool) -> Vec<BoardTicket> {
        let mut ts: Vec<_> = self
            .tickets
            .get()
            .into_iter()
            .filter(|t| {
                t.status == s
                    && t.project_id == self.project.get()
                    && (!filtered || self.visible(t))
            })
            .collect();
        ts.sort_by(|a, b| a.sort_order.total_cmp(&b.sort_order).then(a.id.cmp(&b.id)));
        ts
    }
    fn pickup(self, ticket: i64) {
        if self.busy.get_untracked() || self.loading.get_untracked() {
            return;
        }
        self.cancel();
        self.snapshot.set(self.tickets.get_untracked());
        self.dragged.set(Some(ticket));
        self.notice
            .set("Picked up ticket. Arrows move; Space drops; Escape cancels.".into());
    }
    fn place(self, ticket: i64, s: Status, before: Option<i64>) {
        let mut column = self.column(s, false);
        column.retain(|t| id(t) != ticket);
        let at = before
            .and_then(|b| column.iter().position(|t| id(t) == b))
            .unwrap_or(column.len());
        if let Some(mut moving) = self
            .tickets
            .get_untracked()
            .into_iter()
            .find(|t| id(t) == ticket)
        {
            moving.status = s;
            column.insert(at, moving);
            self.tickets.update(|ts| {
                for (index, t) in column.iter().enumerate() {
                    if let Some(row) = ts.iter_mut().find(|r| id(r) == id(t)) {
                        row.status = s;
                        row.sort_order = (index + 1) as f64 * 1024.;
                    }
                }
            });
        }
        focus(ticket);
    }
    fn cancel(self) {
        if self.dragged.get_untracked().is_some() {
            self.tickets.set(self.snapshot.get_untracked());
            self.dragged.set(None);
            self.notice.set("Move canceled.".into());
        }
    }
    fn save(self, ticket: i64) {
        let before = self.snapshot.get_untracked();
        let after = self.tickets.get_untracked();
        self.dragged.set(None);
        self.busy.set(true);
        self.error.set(String::new());
        spawn_local(async move {
            let mut saved = vec![];
            for t in &after {
                if let Some(old) = before.iter().find(|o| id(o) == id(t))
                    && (old.status != t.status || old.sort_order != t.sort_order)
                {
                    match api(
                        "PATCH",
                        &format!("/api/v1/tickets/{}", id(t)),
                        json!({"status":t.status,"sort_order":t.sort_order}),
                    )
                    .await
                    {
                        Ok(_) => saved.push(old.clone()),
                        Err(e) => {
                            let mut restored = true;
                            for old in saved {
                                restored &= api(
                                    "PATCH",
                                    &format!("/api/v1/tickets/{}", id(&old)),
                                    json!({"status":old.status,"sort_order":old.sort_order}),
                                )
                                .await
                                .is_ok();
                            }
                            self.tickets.set(before);
                            self.busy.set(false);
                            self.error.set(format!(
                                "Couldn’t move ticket: {e} {}",
                                if restored {
                                    "Previous order restored. Try the move again."
                                } else {
                                    "Reload to reconcile the saved order."
                                }
                            ));
                            focus(ticket);
                            return;
                        }
                    }
                }
            }
            self.busy.set(false);
            self.notice.set("Ticket moved. Order saved.".into());
            focus(ticket);
        });
    }
    fn move_to(self, ticket: i64, s: Status, before: Option<i64>) {
        if self.dragged.get_untracked().is_none() {
            self.pickup(ticket);
        }
        if self.dragged.get_untracked() == Some(ticket) {
            self.place(ticket, s, before);
            self.save(ticket);
        }
    }
    fn keyboard(self, ticket: i64, e: web_sys::KeyboardEvent) {
        let last_column = if window().location().search().is_ok_and(|q| {
            q.split('&')
                .any(|part| part.trim_start_matches('?') == "canceled=true")
        }) {
            5
        } else {
            4
        };
        if e.target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .is_some_and(|t| t.tag_name() != "ARTICLE")
        {
            return;
        }
        if self.busy.get_untracked() {
            return;
        }
        let key = e.key();
        if ![
            " ",
            "Escape",
            "ArrowUp",
            "ArrowDown",
            "ArrowLeft",
            "ArrowRight",
        ]
        .contains(&key.as_str())
        {
            return;
        }
        e.prevent_default();
        if key == "Escape" {
            self.cancel();
            focus(ticket);
            return;
        }
        if key == " " {
            if self.dragged.get_untracked() == Some(ticket) {
                self.save(ticket);
            } else {
                self.pickup(ticket);
            }
            return;
        }
        let Some(t) = self
            .tickets
            .get_untracked()
            .into_iter()
            .find(|t| id(t) == ticket)
        else {
            return;
        };
        let s = status(&t);
        let column = self.column(s, true);
        let index = column.iter().position(|t| id(t) == ticket).unwrap_or(0);
        let target = match key.as_str() {
            "ArrowUp" => column.get(index.saturating_sub(1)).map(id),
            "ArrowDown" => column.get(index + 1).map(id),
            _ => None,
        };
        if self.dragged.get_untracked() == Some(ticket) {
            if key == "ArrowLeft" || key == "ArrowRight" {
                let at = Status::ALL.iter().position(|v| *v == s).unwrap_or(0);
                let next = if key == "ArrowLeft" {
                    at.saturating_sub(1)
                } else {
                    (at + 1).min(last_column)
                };
                self.place(ticket, Status::ALL[next], None);
            } else if let Some(target) = target
                && target != ticket
            {
                let full = self.column(s, false);
                let Some(at) = full.iter().position(|t| id(t) == target) else {
                    return;
                };
                let before = if key == "ArrowDown" {
                    full.get(at + 1).map(id)
                } else {
                    Some(target)
                };
                self.place(ticket, s, before);
            }
            self.notice
                .set("Position updated. Space to save; Escape to cancel.".into());
        } else if let Some(target) = target {
            focus(target);
        } else if key == "ArrowLeft" || key == "ArrowRight" {
            let Some(at) = Status::ALL.iter().position(|v| *v == s) else {
                return;
            };
            let candidates: Vec<_> = if key == "ArrowLeft" {
                (0..at).rev().collect()
            } else {
                (at + 1..=last_column).collect()
            };
            for next in candidates {
                if let Some(t) = self.column(Status::ALL[next], true).first() {
                    focus(id(t));
                    break;
                }
            }
        }
    }
    /// #16 integration boundary: apply an authoritative ticket upsert/delete event.
    /// Defer events during local edits; transport should refetch after the edit settles.
    pub(crate) fn apply_change_event(self, ticket: BoardTicket, deleted: bool) -> bool {
        if self.busy.get_untracked() || self.dragged.get_untracked().is_some() {
            return false;
        }
        self.tickets.update(|ts| {
            ts.retain(|t| id(t) != id(&ticket));
            if !deleted {
                ts.push(ticket);
            }
        });
        true
    }
}
fn focus(ticket: i64) {
    request_animation_frame(move || {
        if let Some(e) = document().get_element_by_id(&format!("ticket-{ticket}"))
            && let Some(e) = e.dyn_ref::<web_sys::HtmlElement>()
        {
            let _ = e.focus();
        }
    });
}

#[component]
pub fn ApiBoard() -> impl IntoView {
    let state = expect_context::<BoardState>();
    let design = expect_context::<DesignState>();
    state.refresh();
    let query = leptos_router::hooks::use_query_map();
    let navigate = leptos_router::hooks::use_navigate();
    let show_canceled =
        Signal::derive(move || query.get().get("canceled").as_deref() == Some("true"));
    let toggle_canceled = Callback::new(move |_| {
        let mut params = query.get_untracked();
        if show_canceled.get_untracked() {
            params.remove("canceled");
        } else {
            params.insert("canceled", "true".into());
        }
        navigate(
            &format!("/{}", params.to_query_string()),
            Default::default(),
        );
    });
    let handle = window_event_listener(ev::keydown, move |e| {
        let editing = e
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .is_some_and(|t| ["INPUT", "TEXTAREA", "SELECT"].contains(&t.tag_name().as_str()));
        if !editing
            && !e.ctrl_key()
            && !e.meta_key()
            && e.key() == "c"
            && state.project.get_untracked() != 0
            && !state.busy.get_untracked()
            && !state.loading.get_untracked()
            && state.dragged.get_untracked().is_none()
            && state.composer.get_untracked().is_none()
            && design.dialog.get_untracked() == DialogKind::None
        {
            e.prevent_default();
            state.composer.set(Some(Status::Todo));
        }
    });
    on_cleanup(move || handle.remove());
    view! {
        <h1 class="sr-only">"Board"</h1><header class="topbar"><div class="breadcrumbs"><a href="/projects"><Icon name="project"/>"Projects"</a><span class="breadcrumb-slash">"/"</span><span class="breadcrumb-current">{move||state.projects.get().iter().find(|p|p.id==state.project.get()).map(|p|p.name.clone()).unwrap_or_default()}</span></div><div class="topbar-actions"><crate::live::LiveIndicator/><span class="toolbar-divider"/><button class="button button-primary" disabled=move||state.project.get()==0 || state.busy.get() || state.loading.get() || state.dragged.get().is_some() on:click=move|_|state.composer.set(Some(Status::Todo))><Icon name="plus"/>"New ticket"<Shortcut>"C"</Shortcut></button></div></header>
        <div class="view-tabs"><nav aria-label="Ticket view"><span class="view-tab selected"><Icon name="board"/>"Board"</span><a class="view-tab" href="/list"><Icon name="list"/>"List"</a></nav><crate::pages::projects::ProjectLinks/></div>
        <Show when=move||state.tickets.get().iter().any(|t|t.project_id==state.project.get())><div class="view-toolbar board-filters"><div class="filter-input"><Icon name="filter"/><input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  aria-label="Filter tickets" disabled=move||state.dragged.get().is_some() placeholder="Filter tickets…" prop:value=move||state.query.get() on:input=move|e|state.query.set(event_target_value(&e))/></div>
        <select autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  aria-label="Filter by label" disabled=move||state.dragged.get().is_some() prop:value=move||state.label.get() on:change=move|e|state.label.set(event_target_value(&e))><option value="">"All labels"</option>{move||state.labels.get().into_iter().map(|l|view!{<option value=l.id.to_string()>{l.name}</option>}).collect_view()}</select>
        <select autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  aria-label="Filter by assignee" disabled=move||state.dragged.get().is_some() prop:value=move||state.assignee.get() on:change=move|e|state.assignee.set(event_target_value(&e))><option value="">"All assignees"</option><option value="__unassigned">"Unassigned"</option>{move||{let mut names:Vec<_>=state.tickets.get().iter().filter(|t|t.project_id==state.project.get()).filter_map(|t|t.assignee.clone()).collect();names.sort();names.dedup();names.into_iter().map(|n|view!{<option value=n.clone()>{n.clone()}</option>}).collect_view()}}</select><label class="canceled-toggle"><input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  type="checkbox" prop:checked=move||show_canceled.get() disabled=move||state.dragged.get().is_some() on:change=move|_|toggle_canceled.run(())/>"Show canceled"</label></div></Show>
        <Show when=move||state.loading.get()><p class="board-message" role="status">"Loading board…"</p></Show>
        <Show when=move||!state.error.get().is_empty()><div class="board-message" role="alert">{move||state.error.get()}" "" "<button class="button" on:click=move|_|state.refresh() disabled=move||state.busy.get() || state.loading.get()>"Reload board"</button></div></Show>
        <Show when=move||!state.loading.get() && state.error.get().is_empty() && state.project.get()==0><div class="empty-project"><div class="empty-state"><Icon name="project"/><h2>"No projects yet"</h2><a class="button button-primary" href="/projects">"Create project"</a></div></div></Show>
        <Show when=move||!state.loading.get() && state.error.get().is_empty() && state.project.get()!=0 && !state.tickets.get().iter().any(|t|t.project_id==state.project.get())><div class="empty-project"><EmptyState title="No tickets yet" description="Add the first ticket to this project." action="New ticket" icon="ticket" on_action=Callback::new(move|_|state.composer.set(Some(Status::Todo)))/></div></Show>
        <Show when=move||!state.loading.get() && state.error.get().is_empty() && state.tickets.get().iter().any(|t|t.project_id==state.project.get()) && !state.tickets.get().iter().any(|t|state.visible(t) && (t.status!=Status::Canceled || show_canceled.get()))><div class="board-message empty-state"><h2>"No tickets match this view"</h2><button class="button" on:click=move|_|{state.query.set(String::new());state.label.set(String::new());state.assignee.set(String::new());}>"Clear filters"</button></div></Show>
        <Show when=move||state.project.get()!=0><div class="board api-board" aria-label="Ticket board" aria-busy=move||state.busy.get().to_string()>{Status::ALL.into_iter().map(|s|view!{
            <Show when=move|| (s!=Status::Canceled || show_canceled.get()) && (s!=Status::InReview || !state.column(s,false).is_empty())>
            <section class="board-column" aria-label=s.name() on:dragover=move|e|e.prevent_default() on:drop=move|e|{e.prevent_default();if let Some(ticket)=state.dragged.get_untracked(){state.move_to(ticket,s,None);}}>
            <div class="column-heading"><StatusIcon status=s/><h2>{s.name()}</h2><span class="count">{move||state.column(s,true).len()}</span><button class="icon-button" aria-label=format!("Add ticket to {}",s.name()) disabled=move||state.busy.get() || state.project.get()==0 || state.loading.get() || state.dragged.get().is_some() on:click=move|_|state.composer.set(Some(s))><Icon name="plus"/></button></div>
            <div class="column-cards"><For each=move||state.column(s,true) key=|t| (id(t),format!("{t:?}")) children=move|ticket|view!{<ApiCard ticket/>}/></div><button class="add-card" disabled=move||state.busy.get() || state.project.get()==0 || state.loading.get() || state.dragged.get().is_some() on:click=move|_|state.composer.set(Some(s))><Icon name="plus"/>"New ticket"</button></section>
            </Show>
        }).collect_view()}</div></Show>
        <div class="view-footer" id="board-keys"><span>{move||crate::count::count_label(state.tickets.get().iter().filter(|t|state.visible(t) && (t.status!=Status::Canceled || show_canceled.get())).count(), "ticket")}</span><span>"Space: pick up / drop · Arrows: move · Esc: cancel"</span></div>
        {move||state.composer.get().map(|s|view!{<BoardComposer status=s/>})}
    }
}
#[component]
fn ApiCard(ticket: BoardTicket) -> impl IntoView {
    let state = expect_context::<BoardState>();
    let ticket_id = id(&ticket);
    let s = status(&ticket);
    let key = ticket.key.clone();
    let priority = ticket.priority;
    let blockers = ticket.blocked_by.clone();
    let blocked =
        move || {
            blockers
                .iter()
                .filter_map(|b| {
                    state.tickets.get().into_iter().find(|t| {
                        t.id == *b && !matches!(t.status, Status::Done | Status::Canceled)
                    })
                })
                .map(|t| t.key)
                .collect::<Vec<_>>()
        };
    view! {<article id=format!("ticket-{ticket_id}") class="ticket-card api-ticket" tabindex="0" aria-label=format!("{key}: {}",ticket.title.clone()) aria-describedby="board-keys" class:drag-over=move||state.dragged.get()==Some(ticket_id) draggable=move||(!state.busy.get()).to_string()
        on:keydown=move|e|state.keyboard(ticket_id,e) on:dragstart=move|e|{state.pickup(ticket_id);if let Some(data)=e.data_transfer(){let _=data.set_data("text/plain",&ticket_id.to_string());}} on:dragend=move|_|state.cancel()
        on:dragover=move|e|{e.prevent_default();e.stop_propagation();} on:drop=move|e|{e.prevent_default();e.stop_propagation();if let Some(moving)=state.dragged.get_untracked() && moving!=ticket_id{state.move_to(moving,s,Some(ticket_id));}}>
        <div class="card-top"><span class="mono ticket-id">{key.clone()}</span><span title=priority.name()><PriorityIcon priority/></span></div><a class="card-title" href=format!("/ticket?id={ticket_id}")>{ticket.title.clone()}</a>
        {move||{let keys=blocked(); (!keys.is_empty()).then(||view!{<span class="blocked-note"><Icon name="blocked"/>{format!("Blocked by {}",keys.join(", "))}</span>})}}
        <div class="card-labels">{ticket.labels.into_iter().map(|l|view!{<LabelChip name=l.name color=l.color/>}).collect_view()}</div>
        <div class="card-bottom"><div class="card-status"><StatusIcon status=s/><select autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  aria-label=format!("Move {key} to status") disabled=move||state.busy.get() || state.dragged.get().is_some() prop:value=s.name() on:change=move|e|state.move_to(ticket_id,Status::from_name(&event_target_value(&e)),None)>{Status::ALL.into_iter().map(|v|view!{<option value=v.name() selected=v==s>{v.name()}</option>}).collect_view()}</select></div><span class="card-meta">{ticket.comment_count.filter(|n|*n>0).map(|n|view!{<span class="comment-count"><Icon name="comment"/>{n}</span>})}{ticket.assignee.map(|name|{let initials=name.split_whitespace().take(2).filter_map(|s|s.chars().next()).collect::<String>().to_uppercase();view!{<Avatar label=name initials/>}})}</span></div>
    </article>}
}
#[component]
pub(crate) fn BoardComposer(status: Status) -> impl IntoView {
    let state = expect_context::<BoardState>();
    let title = RwSignal::new(String::new());
    let body = RwSignal::new(String::new());
    let project = RwSignal::new(state.project.get_untracked());
    let error = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    view! {<Modal title="New ticket" locked=Signal::derive(move||busy.get()) on_close=Callback::new(move|_|state.composer.set(None))><form autocomplete="off" data-1p-ignore="true" data-form-type="other" on:submit=move|e|{e.prevent_default();if busy.get_untracked() || title.get_untracked().trim().is_empty(){return;}busy.set(true);spawn_local(async move{match api("POST","/api/v1/tickets",json!({"project_id":project.get_untracked(),"title":title.get_untracked(),"body":body.get_untracked(),"status":status,"sort_order":state.tickets.get_untracked().iter().filter(|t|t.project_id==project.get_untracked() && t.status==status).map(|t|t.sort_order).fold(0.0,f64::max)+1024.})).await {Ok(value)=>{match serde_json::from_value::<BoardTicket>(value){Ok(ticket)=>{state.apply_change_event(ticket,false);state.composer.set(None);state.notice.set("Ticket created.".into());},Err(e)=>{let message=format!("Invalid created ticket response: {e}. Reload the board.");state.error.set(message.clone());error.set(message);}}},Err(e)=>{state.error.set(e.clone());error.set(e)}}busy.set(false);});}>
    <div class="modal-body"><TextInput id="board-title" label="Title" placeholder="What needs to be done?" value=title/><super::api_detail::MarkdownEditor body label="Description"/><label class="field"><span>"Project"</span><select autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  aria-label="New ticket project" prop:value=move||project.get().to_string() on:change=move|e|{if let Ok(id)=event_target_value(&e).parse::<i64>(){project.set(id);}}>{move||state.projects.get().into_iter().map(|p|view!{<option value=p.id.to_string() selected=move||p.id==project.get()>{p.name}</option>}).collect_view()}</select></label><div class="form-project"><span>"Status"</span><StatusIcon status/>{status.name()}</div><p role="alert">{move||error.get()}</p></div><div class="modal-footer"><button class="button button-ghost" type="button" disabled=move||busy.get() on:click=move|_|state.composer.set(None)>"Cancel"</button><button class="button button-primary" type="submit" disabled=move||busy.get() || title.get().trim().is_empty()>"Create ticket"</button></div></form></Modal>}
}
