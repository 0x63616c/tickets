//! Compact API-backed list; view state is entirely represented in the query string.
use super::board::{BoardComposer, BoardState, BoardTicket};
use crate::{
    components::*,
    fixtures::{Priority, Status},
    ui::api,
};
use leptos::{ev, prelude::*};
use leptos_router::hooks::{use_navigate, use_query_map};
use serde_json::json;
use wasm_bindgen_futures::spawn_local;
use web_sys::wasm_bindgen::JsCast;

#[derive(Clone, Copy)]
struct ListLocation {
    query: Memo<leptos_router::params::ParamsMap>,
    navigate: Callback<String>,
}

fn param(location: ListLocation, name: &str) -> String {
    location.query.get().get(name).unwrap_or_default()
}
fn set_param(location: ListLocation, name: &'static str, value: String) {
    let mut query = location.query.get_untracked();
    if value.is_empty() {
        query.remove(name);
    } else {
        query.insert(name, value);
    }
    location
        .navigate
        .run(format!("/list{}", query.to_query_string()));
}
fn rows(state: BoardState, location: ListLocation) -> Vec<BoardTicket> {
    let mut rows: Vec<_> = state
        .tickets
        .get()
        .into_iter()
        .filter(|t| {
            (param(location, "project").is_empty()
                || param(location, "project") == t.project_id.to_string())
                && (param(location, "status").is_empty()
                    || param(location, "status") == t.status.name())
                && (param(location, "label").is_empty()
                    || t.labels
                        .iter()
                        .any(|l| l.id.to_string() == param(location, "label")))
                && (param(location, "assignee").is_empty()
                    || if param(location, "assignee") == "__unassigned" {
                        t.assignee.is_none()
                    } else {
                        t.assignee.as_deref() == Some(param(location, "assignee").as_str())
                    })
                && format!("{} {}", t.key, t.title)
                    .to_lowercase()
                    .contains(&param(location, "q").to_lowercase())
        })
        .collect();
    rows.sort_by(|a, b| {
        let order = match param(location, "sort").as_str() {
            "created" => b.created_at.cmp(&a.created_at),
            "updated" => b.updated_at.cmp(&a.updated_at),
            _ => priority_rank(a.priority).cmp(&priority_rank(b.priority)),
        };
        order.then(a.id.cmp(&b.id))
    });
    rows
}
fn priority_rank(p: Priority) -> u8 {
    match p {
        Priority::Urgent => 1,
        Priority::High => 2,
        Priority::Medium => 3,
        Priority::Low => 4,
        Priority::None => 5,
    }
}
fn focus(id: i64) {
    request_animation_frame(move || {
        if let Some(e) = document()
            .get_element_by_id(&format!("list-ticket-{id}"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = e.focus();
        }
    });
}
fn edit(state: BoardState, id: i64, field: &'static str, value: serde_json::Value) {
    if state.busy.get_untracked() {
        return;
    }
    state.busy.set(true);
    state.error.set(String::new());
    spawn_local(async move {
        match api(
            "PATCH",
            &format!("/api/v1/tickets/{id}"),
            json!({field:value}),
        )
        .await
        {
            Ok(value) => match serde_json::from_value::<BoardTicket>(value) {
                Ok(ticket) => {
                    state.tickets.update(|ts| {
                        if let Some(t) = ts.iter_mut().find(|t| t.id == id) {
                            *t = ticket;
                        }
                    });
                    state.notice.set("Ticket updated.".into());
                }
                Err(e) => state
                    .error
                    .set(format!("Invalid ticket response: {e}. Reload the list.")),
            },
            Err(e) => {
                state.error.set(e);
                if let Some(ticket) = state.tickets.get_untracked().iter().find(|t| t.id == id) {
                    for (property, value) in [
                        ("Status", ticket.status.name()),
                        ("Priority", ticket.priority.name()),
                    ] {
                        if let Ok(Some(element)) = document().query_selector(&format!(
                            "[aria-label='{property} for {}']",
                            ticket.key
                        )) && let Ok(select) = element.dyn_into::<web_sys::HtmlSelectElement>()
                        {
                            select.set_value(value);
                        }
                    }
                }
            }
        }
        state.busy.set(false);
        focus(id);
    });
}
#[component]
fn Filter(
    name: &'static str,
    label: &'static str,
    options: Signal<Vec<(String, String)>>,
) -> impl IntoView {
    let location = expect_context::<ListLocation>();
    view! {<select autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  aria-label=label prop:value=move||param(location, name) on:change=move|e|set_param(location, name,event_target_value(&e))><option value="">{label}</option>{move||options.get().into_iter().map(|(value,text)|view!{<option value=value.clone() selected=move||param(location,name)==value>{text}</option>}).collect_view()}</select>}
}
#[component]
pub fn ApiList() -> impl IntoView {
    let state = expect_context::<BoardState>();
    let navigate = use_navigate();
    let location = ListLocation {
        query: use_query_map(),
        navigate: Callback::new(move |url: String| navigate(&url, Default::default())),
    };
    provide_context(location);
    Effect::new(move |_| {
        if let Ok(project) = param(location, "project").parse::<i64>()
            && state.projects.get().iter().any(|p| p.id == project)
        {
            state.project.set(project);
        }
    });
    let selected = Callback::new(move |id: i64| {
        if let Some(ticket) = state.tickets.get_untracked().iter().find(|t| t.id == id) {
            let _ = window().location().set_href(&format!("/t/{}", ticket.key));
        }
    });
    state.refresh();
    let handle = window_event_listener(ev::keydown, move |e| {
        if e.ctrl_key()
            || e.meta_key()
            || e.alt_key()
            || document().query_selector("dialog").ok().flatten().is_some()
        {
            return;
        }
        if e.target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .is_some_and(|t| {
                ["INPUT", "TEXTAREA", "SELECT", "BUTTON", "A"].contains(&t.tag_name().as_str())
            })
        {
            return;
        }
        let mut visible = Vec::new();
        for status in Status::ALL {
            visible.extend(
                rows(state, location)
                    .into_iter()
                    .filter(|t| t.status == status),
            );
        }
        let active = document()
            .active_element()
            .map(|e| e.id())
            .unwrap_or_default();
        let at = visible
            .iter()
            .position(|t| active == format!("list-ticket-{}", t.id));
        match e.key().as_str() {
            "j" | "k" => {
                e.prevent_default();
                let next = match at {
                    Some(at) if e.key() == "k" => at.saturating_sub(1),
                    Some(at) => (at + 1).min(visible.len().saturating_sub(1)),
                    None => 0,
                };
                if let Some(t) = visible.get(next) {
                    focus(t.id);
                }
            }
            "Enter" => {
                if let Some(at) = at {
                    e.prevent_default();
                    selected.run(visible[at].id);
                }
            }
            "c" if !state.loading.get_untracked() && !state.busy.get_untracked() => {
                state.composer.set(Some(Status::Todo))
            }
            _ => {}
        }
    });
    on_cleanup(move || handle.remove());
    view! {
        <h1 class="sr-only">"Ticket list"</h1><header class="topbar"><div class="breadcrumbs"><a href="/projects"><Icon name="project"/>"Projects"</a><span class="breadcrumb-slash">"/"</span><span class="breadcrumb-current">{move||{let id=param(location, "project");state.projects.get().into_iter().find(|p|p.id.to_string()==id).map(|p|p.name).unwrap_or_else(||"All projects".into())}}</span></div><div class="topbar-actions"><crate::live::LiveIndicator/><span class="toolbar-divider"/><button class="button button-primary" disabled=move||state.loading.get() || state.busy.get() || state.project.get()==0 on:click=move|_|state.composer.set(Some(Status::Todo))><Icon name="plus"/>"New ticket"<Shortcut>"C"</Shortcut></button></div></header>
        <div class="view-tabs list-tabs"><a href="/" class="view-tab"><Icon name="board"/>"Board"</a><span class="view-tab selected"><Icon name="list"/>"List"</span><Show when=move||!param(location, "project").is_empty()><crate::pages::projects::ProjectLinks/></Show></div>
        <div class="view-toolbar board-filters list-filters"><div class="filter-input"><Icon name="filter"/><input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  aria-label="Filter tickets" placeholder="Filter tickets…" prop:value=move||param(location, "q") on:input=move|e|set_param(location, "q",event_target_value(&e))/></div>
        <Filter name="project" label="All projects" options=Signal::derive(move||state.projects.get().into_iter().map(|p|(p.id.to_string(),p.name)).collect())/>
        <Filter name="label" label="All labels" options=Signal::derive(move||state.labels.get().into_iter().map(|l|(l.id.to_string(),l.name)).collect())/>
        <Filter name="assignee" label="All assignees" options=Signal::derive(move||{let mut names:Vec<_>=state.tickets.get().into_iter().filter_map(|t|t.assignee).collect();names.sort();names.dedup();let mut options=vec![("__unassigned".into(),"Unassigned".into())];options.extend(names.into_iter().map(|n|(n.clone(),n)));options})/>
        <Filter name="status" label="All statuses" options=Signal::derive(||Status::ALL.into_iter().map(|s|(s.name().into(),s.name().into())).collect())/>
        <select autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  aria-label="Sort tickets" prop:value=move||{let s=param(location, "sort");if s.is_empty(){"priority".into()}else{s}} on:change=move|e|set_param(location, "sort",event_target_value(&e))><option value="priority">"Sort: Priority"</option><option value="updated">"Sort: Updated"</option><option value="created">"Sort: Created"</option></select>
        <button class="button button-ghost" on:click=move|_|location.navigate.run("/list".into())>"Clear filters"</button></div>
        <Show when=move||state.loading.get()><p class="board-message" role="status">"Loading tickets…"</p></Show>
        <Show when=move||!state.error.get().is_empty()><div class="board-message" role="alert">{move||state.error.get()}<button class="button" disabled=move||state.loading.get() || state.busy.get() on:click=move|_|state.refresh()>"Reload list"</button></div></Show>
        <div class="ticket-list" aria-label="Tickets grouped by status" aria-busy=move||state.busy.get().to_string()>{Status::ALL.into_iter().map(|status|view!{
            <Show when=move||rows(state, location).iter().any(|t|t.status==status)><section class="list-group" aria-label=status.name()><div class="list-group-heading"><StatusIcon status/><h2>{status.name()}</h2><span class="count">{move||rows(state, location).iter().filter(|t|t.status==status).count()}</span><button class="list-add" disabled=move||state.busy.get() || state.loading.get() aria-label=format!("Add ticket to {}",status.name()) on:click=move|_|state.composer.set(Some(status))><Icon name="plus"/></button></div>
            <For each=move||{rows(state, location).into_iter().filter(|t|t.status==status).collect::<Vec<_>>()} key=|t|(t.id,format!("{t:?}")) children=move|ticket|view!{<ListRow ticket selected/>}/></section></Show>
        }).collect_view()}</div>
        <Show when=move||!state.loading.get() && state.error.get().is_empty() && rows(state, location).is_empty()><div class="empty-project"><div class="empty-state"><Icon name="filter"/><h2>"No tickets match this view"</h2><p>"Try another filter or add a ticket."</p><button class="button" on:click=move|_|location.navigate.run("/list".into())>"Clear filters"</button><button class="button button-primary" disabled=move||state.project.get()==0 on:click=move|_|state.composer.set(Some(Status::Todo))><Icon name="plus"/>"New ticket"</button></div></div></Show>
        <div class="view-footer"><span>{move||crate::count::count_label(rows(state, location).len(), "ticket")}</span><span>"J / K: navigate · Enter: open · C: create"</span></div>
        {move||state.composer.get().map(|status|view!{<BoardComposer status/>})}
    }
}
#[component]
fn ListRow(ticket: BoardTicket, selected: Callback<i64>) -> impl IntoView {
    let state = expect_context::<BoardState>();
    let id = ticket.id;
    let status = ticket.status;
    let priority = ticket.priority;
    let blockers = ticket.blocked_by.clone();
    view! {<article class="ticket-row api-list-row" id=format!("list-ticket-{id}") tabindex="0" aria-label=format!("{}: {}",ticket.key,ticket.title)>
        <span class="inline-property"><PriorityIcon priority/><select autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  aria-label=format!("Priority for {}",ticket.key) disabled=move||state.busy.get() prop:value=priority.name() on:change=move|e|{if let Some(p)=Priority::ALL.into_iter().find(|p|p.name()==event_target_value(&e)){edit(state,id,"priority",json!(p));}}>{Priority::ALL.into_iter().map(|p|view!{<option value=p.name() selected=p==priority>{p.name()}</option>}).collect_view()}</select></span>
        <span class="mono ticket-id">{ticket.key.clone()}</span><span class="inline-property"><StatusIcon status/><select autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  aria-label=format!("Status for {}",ticket.key) disabled=move||state.busy.get() prop:value=status.name() on:change=move|e|edit(state,id,"status",json!(Status::from_name(&event_target_value(&e))))>{Status::ALL.into_iter().map(|s|view!{<option value=s.name() selected=s==status>{s.name()}</option>}).collect_view()}</select></span>
        <button class="row-title" title=ticket.title.clone() on:click=move|_|selected.run(id)>{ticket.title.clone()}</button>
        {move||{let keys=blockers.iter().filter_map(|id|state.tickets.get().into_iter().find(|t|t.id==*id && !matches!(t.status,Status::Done|Status::Canceled))).map(|t|t.key).collect::<Vec<_>>();(!keys.is_empty()).then(||view!{<span class="row-blocked" title=format!("Blocked by {}",keys.join(", "))><Icon name="blocked"/>"Blocked"</span>})}}
        <span class="row-labels">{ticket.labels.into_iter().map(|l|view!{<LabelChip name=l.name color=l.color/>}).collect_view()}</span>
        <span class="row-comments">{ticket.comment_count.filter(|n|*n>0).map(|n|view!{<Icon name="comment"/>{n}})}</span>
        <span class="row-project"><Icon name="project"/>{move||state.projects.get().iter().find(|p|p.id==ticket.project_id).map(|p|p.name.clone()).unwrap_or_default()}</span><time class="row-date" datetime=ticket.updated_at.clone()>{display_date(&ticket.updated_at)}</time>
    </article>}
}

fn display_date(value: &str) -> String {
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let month = value
        .get(5..7)
        .and_then(|s| s.parse::<usize>().ok())
        .and_then(|n| n.checked_sub(1))
        .and_then(|n| months.get(n));
    match (month, value.get(8..10)) {
        (Some(month), Some(day)) => format!("{month} {day}"),
        _ => value.chars().take(10).collect(),
    }
}
