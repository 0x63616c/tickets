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
    pub pull_requests: Vec<PullRequest>,
    #[serde(default)]
    pub external_url: Option<String>,
    #[serde(default)]
    pub external_id: Option<String>,
}
impl BoardTicket {
    pub(crate) fn ui_status(&self) -> Status {
        if self.status == Status::Todo
            && self
                .labels
                .iter()
                .any(|l| l.name.eq_ignore_ascii_case("later"))
        {
            Status::Backlog
        } else {
            self.status
        }
    }
}
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub(crate) struct PullRequest {
    pub repo: String,
    pub number: u64,
    pub url: String,
    pub title: String,
    pub state: String,
    pub closes: bool,
}
fn id(t: &BoardTicket) -> i64 {
    t.id
}
fn status(t: &BoardTicket) -> Status {
    t.ui_status()
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
    loaded: RwSignal<bool>,
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
            loaded: RwSignal::new(false),
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
        if !background && !self.loaded.get_untracked() {
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
                    self.loaded.set(true);
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
                t.ui_status() == s
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
                        // A legacy `later` ticket can share the displayed Backlog
                        // column while retaining its stored Todo status.
                        row.status = t.status;
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
            &format!("/board{}", params.to_query_string()),
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
    let reveal_root = NodeRef::<leptos::html::Div>::new();
    use_ascii_reveal(
        reveal_root,
        Signal::derive(move || state.loading.get()),
        true,
    );
    view! {
        <h1 class="sr-only">"Board"</h1><header class="topbar"><div class="breadcrumbs"><a href="/projects"><Icon name="project"/>"Projects"</a><span class="breadcrumb-slash">"/"</span><span class="breadcrumb-current">{move||state.projects.get().into_iter().find(|p|p.id==state.project.get()).map(|p|view!{<ProjectIcon name=p.name.clone() icon=p.icon.unwrap_or_default() color=p.color/>{p.name}})}</span></div><div class="topbar-actions"><crate::live::LiveIndicator/></div></header>
        <div class="view-tabs"><nav aria-label="Ticket view"><span class="view-tab selected"><Icon name="board"/>"Board"</span><a class="view-tab" href="/list"><Icon name="list"/>"List"</a></nav><crate::pages::projects::ProjectLinks/></div>
        <Show when=move||state.tickets.get().iter().any(|t|t.project_id==state.project.get())><div class="view-toolbar board-filters"><div class="filter-input"><Icon name="filter"/><input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  aria-label="Filter tickets" disabled=move||state.dragged.get().is_some() placeholder="Filter tickets…" prop:value=move||state.query.get() on:input=move|e|state.query.set(event_target_value(&e))/></div>
        <SelectField label="Filter by label" value=Signal::derive(move||state.label.get()) options=Signal::derive(move||std::iter::once(SelectOption::new("","All labels")).chain(state.labels.get().into_iter().filter(|l|visible_label(&l.name)).map(|l|SelectOption::new(l.id,l.name))).collect()) on_change=Callback::new(move|next:String|state.label.set(next)) disabled=Signal::derive(move||state.dragged.get().is_some())/>
        <SelectField label="Filter by assignee" value=Signal::derive(move||state.assignee.get()) options=Signal::derive(move||{let mut names:Vec<_>=state.tickets.get().iter().filter(|t|t.project_id==state.project.get()).filter_map(|t|t.assignee.clone()).collect();names.sort();names.dedup();vec![SelectOption::new("","All assignees"),SelectOption::new("__unassigned","Unassigned")].into_iter().chain(names.into_iter().map(|n|SelectOption::new(&n,&n))).collect()}) on_change=Callback::new(move|next:String|state.assignee.set(next)) disabled=Signal::derive(move||state.dragged.get().is_some())/><label class="canceled-toggle"><input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  type="checkbox" prop:checked=move||show_canceled.get() disabled=move||state.dragged.get().is_some() on:change=move|_|toggle_canceled.run(())/>"Show canceled"</label></div></Show>
        <Show when=move||state.loading.get()><LoadingSkeleton layout="board"/></Show>
        <Show when=move||!state.error.get().is_empty()><div class="board-message" role="alert">{move||state.error.get()}" "" "<button class="button" on:click=move|_|state.refresh() disabled=move||state.busy.get() || state.loading.get()>"Reload board"</button></div></Show>
        <Show when=move||!state.loading.get() && state.error.get().is_empty() && state.project.get()==0><div class="empty-project"><div class="empty-state"><Icon name="project"/><h2>"No projects yet"</h2><a class="button button-primary" href="/projects">"Create project"</a></div></div></Show>
        <Show when=move||!state.loading.get() && state.error.get().is_empty() && state.project.get()!=0 && !state.tickets.get().iter().any(|t|t.project_id==state.project.get())><div class="empty-project"><EmptyState title="No tickets yet" description="Add the first ticket to this project." action="New ticket" icon="ticket" on_action=Callback::new(move|_|state.composer.set(Some(Status::Todo)))/></div></Show>
        <Show when=move||!state.loading.get() && state.error.get().is_empty() && state.tickets.get().iter().any(|t|t.project_id==state.project.get()) && !state.tickets.get().iter().any(|t|state.visible(t) && (t.status!=Status::Canceled || show_canceled.get()))><div class="board-message empty-state"><h2>"No tickets match this view"</h2><button class="button" on:click=move|_|{state.query.set(String::new());state.label.set(String::new());state.assignee.set(String::new());}>"Clear filters"</button></div></Show>
        <Show when=move||state.project.get()!=0><div node_ref=reveal_root class="board api-board" aria-label="Ticket board" aria-busy=move||state.busy.get().to_string()>{Status::ALL.into_iter().map(|s|view!{
            <Show when=move|| (s!=Status::Canceled || show_canceled.get()) && (s!=Status::InReview || !state.column(s,false).is_empty())>
            <section class="board-column" aria-label=s.name() on:dragover=move|e|e.prevent_default() on:drop=move|e|{e.prevent_default();if let Some(ticket)=state.dragged.get_untracked(){state.move_to(ticket,s,None);}}>
            <div class="column-heading"><StatusIcon status=s/><h2>{s.name()}</h2><span class="count">{move||state.column(s,true).len()}</span><button class="icon-button" aria-label=format!("Add ticket to {}",s.name()) disabled=move||state.busy.get() || state.project.get()==0 || state.loading.get() || state.dragged.get().is_some() on:click=move|_|state.composer.set(Some(s))><Icon name="plus"/></button></div>
            <div class="column-cards"><For each=move||state.column(s,true) key=|t| (id(t),format!("{t:?}")) children=move|ticket|view!{<ApiCard ticket/>}/></div><button class="add-card" disabled=move||state.busy.get() || state.project.get()==0 || state.loading.get() || state.dragged.get().is_some() on:click=move|_|state.composer.set(Some(s))><Icon name="plus"/>"New ticket"</button></section>
            </Show>
        }).collect_view()}</div></Show>
        <div class="view-footer" id="board-keys"><span hidden=move||state.loading.get()>{move||crate::count::count_label(state.tickets.get().iter().filter(|t|state.visible(t) && (t.status!=Status::Canceled || show_canceled.get())).count(), "ticket")}</span><span>"Space: pick up / drop · Arrows: move · Esc: cancel"</span></div>
        {let shown=present_value(state.composer.into());move||shown.get().map(|status|view!{<BoardComposer status/>})}
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
                .map(|t| (t.key, t.title))
                .collect::<Vec<_>>()
        };
    let navigate = leptos_router::hooks::use_navigate();
    let href = format!("/t/{key}");
    let open = Callback::new({
        let href = href.clone();
        move |_| navigate(&href, Default::default())
    });
    // Keep the drag latch until the next pointer gesture: dragend clears board state,
    // but must not turn a trailing click into navigation. TIK-51 can replace this
    // alongside the drag implementation without a pointer-distance state machine.
    let was_dragged = RwSignal::new(false);
    view! {<article id=format!("ticket-{ticket_id}") class="ticket-card api-ticket" data-motion-key=format!("ticket-{ticket_id}") tabindex="0" aria-label=format!("{key}: {}",ticket.title.clone()) aria-describedby="board-keys" class:drag-over=move||state.dragged.get()==Some(ticket_id) draggable=move||(!state.busy.get()).to_string()
        on:pointerdown=move|_|was_dragged.set(false)
        on:click=move|e|{if !was_dragged.get_untracked() && state.dragged.get_untracked().is_none() && !ticket_interactive_target(e.target()){open.run(());}}
        on:keydown=move|e|{if e.key()=="Enter" && !ticket_interactive_target(e.target()) && state.dragged.get_untracked().is_none(){e.prevent_default();open.run(());}else{state.keyboard(ticket_id,e);}}
        on:dragstart=move|e|{was_dragged.set(true);state.pickup(ticket_id);if let Some(data)=e.data_transfer(){let _=data.set_data("text/plain",&ticket_id.to_string());}} on:dragend=move|_|state.cancel()
        on:dragover=move|e|{e.prevent_default();e.stop_propagation();} on:drop=move|e|{e.prevent_default();e.stop_propagation();if let Some(moving)=state.dragged.get_untracked() && moving!=ticket_id{state.move_to(moving,s,Some(ticket_id));}}>
        <div class="card-top"><span class="mono ticket-id">{key.clone()}</span><span class="card-indicators">{move||{let blockers=blocked();(!blockers.is_empty()).then(||view!{<BlockedIndicator blockers/>})}}<span title=priority.name()><PriorityIcon priority/></span>
            <span class="card-status" title=s.name()><SelectField label=format!("Move {key} to status") value=Signal::derive(move||s.name().to_string()) options=Signal::derive(move||Status::ALL.into_iter().map(SelectOption::status).collect()) on_change=Callback::new(move|next:String|state.move_to(ticket_id,Status::from_name(&next),None)) disabled=Signal::derive(move||state.busy.get() || state.dragged.get().is_some())/></span>
        </span></div><a class="card-title" href=href title=ticket.title.clone()>{ticket.title.clone()}</a>
        <div class="card-bottom"><CardLabels labels=ticket.labels/><span class="card-meta">{ticket.comment_count.filter(|n|*n>0).map(|n|view!{<span class="comment-count" aria-label=crate::count::count_label(n,"comment")><Icon name="comment"/>{n}</span>})}</span></div>
    </article>}
}
#[component]
pub(crate) fn BoardComposer(status: Status) -> impl IntoView {
    let state = expect_context::<BoardState>();
    let error = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    // Remember a successful create so retrying a label failure cannot duplicate it.
    let created = RwSignal::new(None::<BoardTicket>);
    let submit = Callback::new(move |draft: TicketDraft| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        error.set(String::new());
        spawn_local(async move {
            let result = async {
                let mut ticket = match created.get_untracked() {
                    Some(ticket) => ticket,
                    None => {
                        let rank = state.tickets.get_untracked().iter()
                            .filter(|t|t.project_id == draft.project_id && t.status == draft.status)
                            .map(|t|t.sort_order).fold(0.0, f64::max) + 1024.;
                        let value = api("POST", "/api/v1/tickets", json!({
                            "project_id":draft.project_id, "title":draft.title, "body":draft.body,
                            "status":draft.status, "priority":draft.priority, "sort_order":rank
                        })).await?;
                        let ticket = serde_json::from_value::<BoardTicket>(value)
                            .map_err(|e|format!("Invalid created ticket response: {e}. Reload the board."))?;
                        created.set(Some(ticket.clone()));
                        state.apply_change_event(ticket.clone(), false);
                        ticket
                    }
                };
                let has_labels = !draft.labels.is_empty();
                for label in draft.labels {
                    if ticket.labels.iter().any(|l|l.id.to_string() == label) { continue; }
                    api("POST", &format!("/api/v1/tickets/{}/labels/{label}",ticket.id), Value::Null).await
                        .map_err(|e|format!("{} was created, but a label could not be added. Retry labels or close and edit the ticket. {e}",ticket.key))?;
                    if let Some(label) = state.labels.get_untracked().into_iter().find(|l|l.id.to_string()==label) {
                        ticket.labels.push(label);
                    }
                    created.set(Some(ticket.clone()));
                    state.apply_change_event(ticket.clone(), false);
                }
                if has_labels {
                    ticket = serde_json::from_value(api("GET", &format!("/api/v1/tickets/{}",ticket.id), Value::Null).await?)
                        .map_err(|e|format!("Could not refresh saved ticket: {e}"))?;
                }
                Ok::<_, String>(ticket)
            }.await;
            match result {
                Ok(ticket) => {
                    state.apply_change_event(ticket, false);
                    state.composer.set(None);
                    state.notice.set("Ticket created.".into());
                }
                Err(e) => error.set(e),
            }
            busy.set(false);
        });
    });
    view! {<TicketComposer project_id=state.project.get_untracked() status
    projects=state.projects.into() labels=state.labels.into()
    open=Signal::derive(move||state.composer.get().is_some()) busy=busy.into() error=error.into()
    created=Signal::derive(move||created.get().is_some()) on_submit=submit
    on_close=Callback::new(move|_|state.composer.set(None))/>}
}
