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
            (t.status != Status::Canceled
                || param(location, "canceled") == "true"
                || param(location, "status") == "Canceled")
                && (param(location, "project").is_empty()
                    || param(location, "project") == t.project_id.to_string())
                && (param(location, "status").is_empty()
                    || param(location, "status") == t.ui_status().name())
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
    view! {<SelectField label=label value=Signal::derive(move||param(location,name)) options=Signal::derive(move||std::iter::once(SelectOption::new("",label)).chain(options.get().into_iter().map(|(v,t)|SelectOption::new(v,t))).collect()) on_change=Callback::new(move|next:String|set_param(location,name,next))/>}
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
                    .filter(|t| t.ui_status() == status),
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
    let reveal_root = NodeRef::<leptos::html::Div>::new();
    use_ascii_reveal(
        reveal_root,
        Signal::derive(move || state.loading.get()),
        true,
    );
    view! {
        <h1 class="sr-only">"Ticket list"</h1><header class="topbar"><div class="breadcrumbs"><a href="/projects"><Icon name="project"/>"Projects"</a><span class="breadcrumb-slash">"/"</span><span class="breadcrumb-current">{move||{let id=param(location, "project");state.projects.get().into_iter().find(|p|p.id.to_string()==id).map(|p|view!{<ProjectIcon name=p.name.clone() icon=p.icon.unwrap_or_default() color=p.color/>{p.name}}.into_any()).unwrap_or_else(||"All projects".into_any())}}</span></div><div class="topbar-actions"><crate::live::LiveIndicator/><span class="toolbar-divider"/><button class="button button-primary" disabled=move||state.loading.get() || state.busy.get() || state.project.get()==0 on:click=move|_|state.composer.set(Some(Status::Todo))><Icon name="plus"/>"New ticket"<Shortcut>"C"</Shortcut></button></div></header>
        <div class="view-tabs list-tabs"><a href="/board" class="view-tab"><Icon name="board"/>"Board"</a><span class="view-tab selected"><Icon name="list"/>"List"</span><Show when=move||!param(location, "project").is_empty()><crate::pages::projects::ProjectLinks/></Show></div>
        <div class="view-toolbar board-filters list-filters"><div class="filter-input"><Icon name="filter"/><input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  aria-label="Filter tickets" placeholder="Filter tickets…" prop:value=move||param(location, "q") on:input=move|e|set_param(location, "q",event_target_value(&e))/></div>
        <SelectField label="All projects" value=Signal::derive(move||param(location,"project")) options=Signal::derive(move||std::iter::once(SelectOption::new("","All projects")).chain(state.projects.get().into_iter().map(SelectOption::project)).collect()) on_change=Callback::new(move|v:String|set_param(location,"project",v))/>
        <Filter name="label" label="All labels" options=Signal::derive(move||state.labels.get().into_iter().filter(|l|visible_label(&l.name)).map(|l|(l.id.to_string(),l.name)).collect())/>
        <Filter name="assignee" label="All assignees" options=Signal::derive(move||{let mut names:Vec<_>=state.tickets.get().into_iter().filter_map(|t|t.assignee).collect();names.sort();names.dedup();let mut options=vec![("__unassigned".into(),"Unassigned".into())];options.extend(names.into_iter().map(|n|(n.clone(),n)));options})/>
        <Filter name="status" label="All statuses" options=Signal::derive(||Status::ALL.into_iter().map(|s|(s.name().into(),s.name().into())).collect())/>
        <SelectField label="Sort tickets" value=Signal::derive(move||{let s=param(location,"sort");if s.is_empty(){"priority".into()}else{s}}) options=Signal::derive(move||vec![SelectOption::new("priority","Sort: Priority"),SelectOption::new("updated","Sort: Updated"),SelectOption::new("created","Sort: Created")]) on_change=Callback::new(move|next:String|set_param(location,"sort",next))/>
        <label class="canceled-toggle"><input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other" type="checkbox" prop:checked=move||param(location,"canceled")=="true" on:change=move|e|set_param(location,"canceled",if event_target_checked(&e){"true".into()}else{String::new()})/>"Show canceled"</label>
        <button class="button button-ghost" on:click=move|_|location.navigate.run("/list".into())>"Clear filters"</button></div>
        <Show when=move||state.loading.get()><LoadingSkeleton layout="list"/></Show>
        <Show when=move||!state.error.get().is_empty()><div class="board-message" role="alert">{move||state.error.get()}<button class="button" disabled=move||state.loading.get() || state.busy.get() on:click=move|_|state.refresh()>"Reload list"</button></div></Show>
        <div node_ref=reveal_root class="ticket-list" aria-label="Tickets grouped by status" aria-busy=move||state.busy.get().to_string()>{Status::ALL.into_iter().map(|status|view!{
            <Show when=move||rows(state, location).iter().any(|t|t.ui_status()==status)><section class="list-group" aria-label=status.name()><div class="list-group-heading"><StatusIcon status/><h2>{status.name()}</h2><span class="count">{move||rows(state, location).iter().filter(|t|t.ui_status()==status).count()}</span><button class="list-add" disabled=move||state.busy.get() || state.loading.get() aria-label=format!("Add ticket to {}",status.name()) on:click=move|_|state.composer.set(Some(status))><Icon name="plus"/></button></div>
            <For each=move||{rows(state, location).into_iter().filter(|t|t.ui_status()==status).collect::<Vec<_>>()} key=|t|(t.id,format!("{t:?}")) children=move|ticket|view!{<ListRow ticket selected/>}/></section></Show>
        }).collect_view()}</div>
        <Show when=move||!state.loading.get() && state.error.get().is_empty() && rows(state, location).is_empty()><div class="empty-project"><div class="empty-state"><Icon name="filter"/><h2>"No tickets match this view"</h2><p>"Try another filter or add a ticket."</p><button class="button" on:click=move|_|location.navigate.run("/list".into())>"Clear filters"</button><button class="button button-primary" disabled=move||state.project.get()==0 on:click=move|_|state.composer.set(Some(Status::Todo))><Icon name="plus"/>"New ticket"</button></div></div></Show>
        <div class="view-footer"><span hidden=move||state.loading.get()>{move||crate::count::count_label(rows(state, location).len(), "ticket")}</span><span>"J / K: navigate · Enter: open · C: create"</span></div>
        {let shown=present_value(state.composer.into());move||shown.get().map(|status|view!{<BoardComposer status/>})}
    }
}
#[component]
fn ListRow(ticket: BoardTicket, selected: Callback<i64>) -> impl IntoView {
    let state = expect_context::<BoardState>();
    let id = ticket.id;
    let status = ticket.ui_status();
    let priority = ticket.priority;
    let blockers = ticket.blocked_by.clone();
    view! {<article class="ticket-row api-list-row" data-motion-key=format!("ticket-{id}") id=format!("list-ticket-{id}") tabindex="0" aria-label=format!("{}: {}",ticket.key,ticket.title) on:click=move|e|{if !ticket_interactive_target(e.target()){selected.run(id);}}>
        <span class="inline-property"><PriorityIcon priority/><SelectField label=format!("Priority for {}",ticket.key) value=Signal::derive(move||priority.name().to_string()) options=Signal::derive(move||options(&Priority::ALL.map(Priority::name))) on_change=Callback::new(move|next:String|{if let Some(p)=Priority::ALL.into_iter().find(|p|p.name()==next){edit(state,id,"priority",json!(p));}}) disabled=Signal::derive(move||state.busy.get())/></span>
        <span class="mono ticket-id" title=ticket.key.clone()>{ticket.key.clone()}</span><span class="inline-property"><StatusIcon status/><SelectField label=format!("Status for {}",ticket.key) value=Signal::derive(move||status.name().to_string()) options=Signal::derive(move||options(&Status::ALL.map(Status::name))) on_change=Callback::new(move|next:String|edit(state,id,"status",json!(Status::from_name(&next)))) disabled=Signal::derive(move||state.busy.get())/></span>
        <a class="row-title" href=format!("/t/{}",ticket.key) title=ticket.title.clone()>{ticket.title.clone()}</a>
        <span class="row-blocked-slot">{move||{let blockers=blockers.iter().filter_map(|id|state.tickets.get().into_iter().find(|t|t.id==*id && !matches!(t.status,Status::Done|Status::Canceled))).map(|t|(t.key,t.title)).collect::<Vec<_>>();(!blockers.is_empty()).then(||view!{<BlockedIndicator blockers/>})}}</span>
        <ListLabels labels=ticket.labels/>
        <span class="row-comments">{ticket.comment_count.filter(|n|*n>0).map(|n|view!{<Icon name="comment"/>{n}})}</span>
        <span class="row-project">{move||state.projects.get().into_iter().find(|p|p.id==ticket.project_id).map(|p|view!{<span title=p.name.clone()><ProjectIcon name=p.name.clone() icon=p.icon.unwrap_or_default() color=p.color/><span class="row-project-name">{p.name.clone()}</span></span>})}</span><time class="row-date" title=ticket.updated_at.clone() datetime=ticket.updated_at.clone()>{display_date(&ticket.updated_at)}</time>
    </article>}
}

/// Measure full-size chips independently of the visible subset. Observing both
/// tracks handles viewport changes and the bundled font finishing its load.
#[component]
fn ListLabels(labels: Vec<tickets_core::domain::Label>) -> impl IntoView {
    use leptos::html;
    use web_sys::wasm_bindgen::closure::Closure;

    let labels = labels
        .into_iter()
        .filter(|label| visible_label(&label.name))
        .collect::<Vec<_>>();
    let total = labels.len();
    let data = StoredValue::new(labels.clone());
    let visible = RwSignal::new(0usize);
    let cell = NodeRef::<html::Span>::new();
    let measure = NodeRef::<html::Span>::new();
    Effect::new(move |_| {
        let (Some(cell), Some(measure)) = (cell.get(), measure.get()) else {
            return;
        };
        let observed_cell = cell.clone();
        let observed_measure = measure.clone();
        let update = move || {
            let width = cell.get_bounding_client_rect().width();
            let chips = measure.query_selector_all(".label-chip").unwrap();
            let counts = measure.query_selector_all(".row-label-count").unwrap();
            let widths = (0..chips.length())
                .map(|i| {
                    chips
                        .item(i)
                        .unwrap()
                        .unchecked_into::<web_sys::Element>()
                        .get_bounding_client_rect()
                        .width()
                })
                .collect::<Vec<_>>();
            // This is the same token-based gap used by the visible track.
            let gap = window()
                .get_computed_style(&cell)
                .ok()
                .flatten()
                .and_then(|style| style.get_property_value("column-gap").ok())
                .and_then(|value| value.trim_end_matches("px").parse::<f64>().ok())
                .unwrap_or(0.0);
            let full_width = widths.iter().sum::<f64>() + gap * total.saturating_sub(1) as f64;
            let mut fit = total;
            if full_width > width {
                fit = 0;
                let mut used = 0.0;
                for (i, chip_width) in widths.iter().enumerate() {
                    let overflow_width = counts
                        .item(i as u32)
                        .unwrap()
                        .unchecked_into::<web_sys::Element>()
                        .get_bounding_client_rect()
                        .width();
                    if used + overflow_width <= width {
                        fit = i;
                    }
                    used += chip_width + gap;
                }
            }
            visible.set(fit);
        };
        update();
        let callback = Closure::<dyn FnMut()>::new(update);
        let observer = web_sys::ResizeObserver::new(callback.as_ref().unchecked_ref()).unwrap();
        observer.observe(&observed_cell);
        observer.observe(&observed_measure);
        let _callback = StoredValue::new_local(callback);
        on_cleanup(move || observer.disconnect());
    });
    view! {<span class="row-labels" node_ref=cell>
        <span class="row-label-measure" node_ref=measure aria-hidden="true">
            {labels.into_iter().map(|label|view!{<LabelChip name=label.name color=label.color/>}).collect_view()}
            {(0..total).map(|i|view!{<span class="row-label-count row-label-overflow">{format!("+{}",total-i)}</span>}).collect_view()}
        </span>
        {move||data.with_value(|labels|labels.iter().take(visible.get()).cloned().map(|label|view!{<LabelChip name=label.name color=label.color/>}).collect_view())}
        {move||{
            let shown=visible.get();
            (shown<total).then(||{
                let text=data.with_value(|labels|labels.iter().skip(shown).map(|label|label.name.as_str()).collect::<Vec<_>>().join(", "));
                view!{<Tooltip text=text.clone()><button type="button" class="row-label-overflow" aria-label=format!("{} more labels: {text}",total-shown)>{format!("+{}",total-shown)}</button></Tooltip>}
            })
        }}
    </span>}
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
