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

fn id(v: &Value) -> i64 {
    v["id"].as_i64().unwrap_or_default()
}
fn text(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().into()
}
fn status(v: &Value) -> Status {
    serde_json::from_value(v["status"].clone()).unwrap_or(Status::Todo)
}
async fn all(path: &str) -> Result<Vec<Value>, String> {
    let mut items = vec![];
    let mut cursor = String::new();
    loop {
        let page = api(
            "GET",
            &format!("/api/v1/{path}?limit=100{cursor}"),
            Value::Null,
        )
        .await?;
        items.extend(
            page["data"]
                .as_array()
                .ok_or("Invalid list response")?
                .clone(),
        );
        match page["next_cursor"].as_str() {
            Some(c) => cursor = format!("&cursor={c}"),
            None => return Ok(items),
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) struct BoardState {
    tickets: RwSignal<Vec<Value>>,
    pub(crate) projects: RwSignal<Vec<Value>>,
    labels: RwSignal<Vec<Value>>,
    pub(crate) project: RwSignal<i64>,
    label: RwSignal<String>,
    assignee: RwSignal<String>,
    query: RwSignal<String>,
    busy: RwSignal<bool>,
    loading: RwSignal<bool>,
    error: RwSignal<String>,
    notice: RwSignal<String>,
    dragged: RwSignal<Option<i64>>,
    snapshot: RwSignal<Vec<Value>>,
    composer: RwSignal<Option<Status>>,
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
            error: RwSignal::new(String::new()),
            notice: RwSignal::new(String::new()),
            dragged: RwSignal::new(None),
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
    fn refresh(self) {
        self.loading.set(true);
        self.error.set(String::new());
        spawn_local(async move {
            let result = async {
                Ok::<_, String>((
                    all("projects").await?,
                    all("tickets").await?,
                    all("labels").await?,
                ))
            }
            .await;
            match result {
                Ok((projects, tickets, labels)) => {
                    if !projects
                        .iter()
                        .any(|p| id(p) == self.project.get_untracked())
                    {
                        self.project.set(projects.first().map(id).unwrap_or(0));
                    }
                    self.projects.set(projects);
                    self.tickets.set(tickets);
                    self.labels.set(labels);
                }
                Err(e) => self.error.set(e),
            }
            self.loading.set(false);
        });
    }
    fn visible(self, t: &Value) -> bool {
        t["project_id"].as_i64() == Some(self.project.get())
            && (self.label.get().is_empty()
                || t["labels"]
                    .as_array()
                    .is_some_and(|ls| ls.iter().any(|l| id(l).to_string() == self.label.get())))
            && (self.assignee.get().is_empty()
                || if self.assignee.get() == "__unassigned" {
                    t["assignee"].is_null()
                } else {
                    text(t, "assignee") == self.assignee.get()
                })
            && format!("{} {}", text(t, "key"), text(t, "title"))
                .to_lowercase()
                .contains(&self.query.get().to_lowercase())
    }
    fn column(self, s: Status, filtered: bool) -> Vec<Value> {
        let mut ts: Vec<_> = self
            .tickets
            .get()
            .into_iter()
            .filter(|t| {
                status(t) == s
                    && t["project_id"].as_i64() == Some(self.project.get())
                    && (!filtered || self.visible(t))
            })
            .collect();
        ts.sort_by(|a, b| {
            a["sort_order"]
                .as_f64()
                .unwrap_or(0.)
                .total_cmp(&b["sort_order"].as_f64().unwrap_or(0.))
                .then(id(a).cmp(&id(b)))
        });
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
            moving["status"] = json!(s);
            column.insert(at, moving);
            self.tickets.update(|ts| {
                for (index, t) in column.iter().enumerate() {
                    if let Some(row) = ts.iter_mut().find(|r| id(r) == id(t)) {
                        row["status"] = json!(s);
                        row["sort_order"] = json!((index + 1) as f64 * 1024.);
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
                    && (old["status"] != t["status"] || old["sort_order"] != t["sort_order"])
                {
                    match api(
                        "PATCH",
                        &format!("/api/v1/tickets/{}", id(t)),
                        json!({"status":t["status"],"sort_order":t["sort_order"]}),
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
                                    json!({"status":old["status"],"sort_order":old["sort_order"]}),
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
                    (at + 1).min(5)
                };
                self.place(ticket, Status::ALL[next], None);
            } else if let Some(target) = target
                && target != ticket
            {
                let full = self.column(s, false);
                let at = full.iter().position(|t| id(t) == target).unwrap();
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
            let at = Status::ALL.iter().position(|v| *v == s).unwrap();
            let next = if key == "ArrowLeft" {
                at.saturating_sub(1)
            } else {
                (at + 1).min(5)
            };
            if let Some(t) = self.column(Status::ALL[next], true).first() {
                focus(id(t));
            }
        }
    }
    /// #16 integration boundary: apply an authoritative ticket upsert/delete event.
    /// Defer events during local edits; transport should refetch after the edit settles.
    pub(crate) fn apply_change_event(self, ticket: Value, deleted: bool) -> bool {
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
    state.refresh();
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
        {
            e.prevent_default();
            state.composer.set(Some(Status::Todo));
        }
    });
    on_cleanup(move || handle.remove());
    view! {
        <header class="topbar"><div class="breadcrumbs board-project"><Icon name="project"/><select aria-label="Project" disabled=move||state.busy.get() || state.dragged.get().is_some() prop:value=move||state.project.get().to_string() on:change=move|e|{state.select_project(event_target_value(&e).parse().unwrap_or(0));}>{move||state.projects.get().into_iter().map(|p|view!{<option value=id(&p).to_string() selected=move||state.project.get()==id(&p)>{text(&p,"name")}</option>}).collect_view()}</select></div><div class="topbar-actions"><button class="button button-primary" disabled=move||state.project.get()==0 || state.busy.get() || state.loading.get() || state.dragged.get().is_some() on:click=move|_|state.composer.set(Some(Status::Todo))><Icon name="plus"/>"New ticket"<Shortcut>"C"</Shortcut></button></div></header>
        <div class="view-tabs"><span class="view-tab selected"><Icon name="board"/>"Board"</span></div>
        <div class="view-toolbar board-filters"><div class="filter-input"><Icon name="filter"/><input aria-label="Filter tickets" disabled=move||state.dragged.get().is_some() placeholder="Filter tickets…" prop:value=move||state.query.get() on:input=move|e|state.query.set(event_target_value(&e))/></div>
        <select aria-label="Filter by label" disabled=move||state.dragged.get().is_some() prop:value=move||state.label.get() on:change=move|e|state.label.set(event_target_value(&e))><option value="">"All labels"</option>{move||state.labels.get().into_iter().map(|l|view!{<option value=id(&l).to_string()>{text(&l,"name")}</option>}).collect_view()}</select>
        <select aria-label="Filter by assignee" disabled=move||state.dragged.get().is_some() prop:value=move||state.assignee.get() on:change=move|e|state.assignee.set(event_target_value(&e))><option value="">"All assignees"</option><option value="__unassigned">"Unassigned"</option>{move||{let mut names:Vec<_>=state.tickets.get().iter().filter(|t|t["project_id"].as_i64()==Some(state.project.get())).filter_map(|t|t["assignee"].as_str().map(str::to_owned)).collect();names.sort();names.dedup();names.into_iter().map(|n|view!{<option value=n.clone()>{n.clone()}</option>}).collect_view()}}</select></div>
        <Show when=move||state.loading.get()><p class="board-message" role="status">"Loading board…"</p></Show>
        <Show when=move||!state.error.get().is_empty()><div class="board-message" role="alert">{move||state.error.get()}" "<Show when=move||state.error.get().contains("sign in")><a href="/login">"Sign in"</a></Show>" "<button class="button" on:click=move|_|state.refresh() disabled=move||state.busy.get() || state.loading.get()>"Reload board"</button></div></Show>
        <Show when=move||!state.loading.get() && state.error.get().is_empty() && state.project.get()==0><p class="board-message">"No projects yet. Create a project through the API to start a board."</p></Show>
        <div class="board" aria-label="Ticket board" aria-busy=move||state.busy.get().to_string()>{Status::ALL.into_iter().map(|s|view!{
            <section class="board-column" aria-label=s.name() on:dragover=move|e|e.prevent_default() on:drop=move|e|{e.prevent_default();if let Some(ticket)=state.dragged.get_untracked(){state.move_to(ticket,s,None);}}>
            <div class="column-heading"><StatusIcon status=s/><h2>{s.name()}</h2><span class="count">{move||state.column(s,true).len()}</span><button class="icon-button" aria-label=format!("Add ticket to {}",s.name()) disabled=move||state.busy.get() || state.project.get()==0 || state.loading.get() || state.dragged.get().is_some() on:click=move|_|state.composer.set(Some(s))><Icon name="plus"/></button></div>
            <div class="column-cards"><For each=move||state.column(s,true) key=|t| (id(t),t.to_string()) children=move|ticket|view!{<ApiCard ticket/>}/></div><button class="add-card" disabled=move||state.busy.get() || state.project.get()==0 || state.loading.get() || state.dragged.get().is_some() on:click=move|_|state.composer.set(Some(s))><Icon name="plus"/>"New ticket"</button></section>
        }).collect_view()}</div>
        <div class="view-footer" id="board-keys"><span role="status" aria-live="polite">{move||state.notice.get()}</span><span>"Space: pick up / drop · Arrows: move · Esc: cancel"</span></div>
        {move||state.composer.get().map(|s|view!{<BoardComposer status=s/>})}
    }
}
#[component]
fn ApiCard(ticket: Value) -> impl IntoView {
    let state = expect_context::<BoardState>();
    let ticket_id = id(&ticket);
    let s = status(&ticket);
    let key = text(&ticket, "key");
    let priority =
        serde_json::from_value::<Priority>(ticket["priority"].clone()).unwrap_or(Priority::None);
    let blockers = ticket["blocked_by"].as_array().cloned().unwrap_or_default();
    let blocked = move || {
        blockers.iter().any(|b| {
            state
                .tickets
                .get()
                .iter()
                .any(|t| t["id"] == *b && !matches!(status(t), Status::Done | Status::Canceled))
        })
    };
    view! {<article id=format!("ticket-{ticket_id}") class="ticket-card api-ticket" tabindex="0" aria-label=format!("{key}: {}",text(&ticket,"title")) aria-describedby="board-keys" class:drag-over=move||state.dragged.get()==Some(ticket_id) draggable=move||(!state.busy.get()).to_string()
        on:keydown=move|e|state.keyboard(ticket_id,e) on:dragstart=move|e|{state.pickup(ticket_id);if let Some(data)=e.data_transfer(){let _=data.set_data("text/plain",&ticket_id.to_string());}} on:dragend=move|_|state.cancel()
        on:dragover=move|e|{e.prevent_default();e.stop_propagation();} on:drop=move|e|{e.prevent_default();e.stop_propagation();if let Some(moving)=state.dragged.get_untracked() && moving!=ticket_id{state.move_to(moving,s,Some(ticket_id));}}>
        <div class="card-top"><span class="mono ticket-id">{key.clone()}</span><span title=priority.name()><PriorityIcon priority/></span></div><div class="card-title">{text(&ticket,"title")}</div>
        <Show when=blocked><span class="blocked-note"><Icon name="blocked"/>"Blocked"</span></Show>
        <div class="card-labels">{ticket["labels"].as_array().cloned().unwrap_or_default().into_iter().map(|l|view!{<LabelChip name=text(&l,"name") color=text(&l,"color")/>}).collect_view()}</div>
        <div class="card-bottom"><div class="card-status"><StatusIcon status=s/><select aria-label=format!("Move {key} to status") disabled=move||state.busy.get() || state.dragged.get().is_some() prop:value=s.name() on:change=move|e|state.move_to(ticket_id,Status::from_name(&event_target_value(&e)),None)>{Status::ALL.into_iter().map(|v|view!{<option value=v.name() selected=v==s>{v.name()}</option>}).collect_view()}</select></div><span class="card-meta">{ticket["assignee"].as_str().unwrap_or("Unassigned").to_owned()}</span></div>
    </article>}
}
#[component]
fn BoardComposer(status: Status) -> impl IntoView {
    let state = expect_context::<BoardState>();
    let title = RwSignal::new(String::new());
    let body = RwSignal::new(String::new());
    let error = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    view! {<Modal title="New ticket" locked=Signal::derive(move||busy.get()) on_close=Callback::new(move|_|state.composer.set(None))><form on:submit=move|e|{e.prevent_default();if busy.get_untracked() || title.get_untracked().trim().is_empty(){return;}busy.set(true);spawn_local(async move{match api("POST","/api/v1/tickets",json!({"project_id":state.project.get_untracked(),"title":title.get_untracked(),"body":body.get_untracked(),"status":status,"sort_order":state.column(status,false).iter().filter_map(|t|t["sort_order"].as_f64()).fold(0.0,f64::max)+1024.})).await {Ok(ticket)=>{state.apply_change_event(ticket,false);state.composer.set(None);state.notice.set("Ticket created.".into());},Err(e)=>error.set(e)}busy.set(false);});}>
    <div class="modal-body"><TextInput id="board-title" label="Title" value=title/><label class="field"><span>"Description"</span><textarea prop:value=move||body.get() on:input=move|e|body.set(event_target_value(&e))/></label><p class="muted">{status.name()}</p><p role="alert">{move||error.get()}</p></div><div class="modal-footer"><button class="button button-primary" type="submit" disabled=move||busy.get() || title.get().trim().is_empty()>"Create ticket"</button></div></form></Modal>}
}
