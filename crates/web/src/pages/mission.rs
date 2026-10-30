//! Cross-project operating queue; source snapshots remain link-only in v1.
use crate::{components::*, ui::api};
use futures_util::future::{AbortHandle, Abortable};
use leptos::prelude::*;
use leptos_router::hooks::use_location;
use serde_json::{Value, json};
use tickets_core::mission::{Mission, Ticket};
use wasm_bindgen::{JsCast, closure::Closure};

fn text(v: &Value, field: &str) -> String {
    match &v[field] {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}
fn age(stamp: &str) -> String {
    let delta = (js_sys::Date::now() - js_sys::Date::parse(stamp)) / 60_000.;
    if !delta.is_finite() {
        return "Age unknown".into();
    }
    if delta < 1. {
        "just now".into()
    } else if delta < 60. {
        format!("{} min ago", delta as u64)
    } else if delta < 1440. {
        format!("{} h ago", (delta / 60.) as u64)
    } else {
        format!("{} d ago", (delta / 1440.) as u64)
    }
}
fn recent(v: &Value) -> bool {
    let delta = js_sys::Date::now() - js_sys::Date::parse(&text(v, "finished_at"));
    delta.is_finite() && (0. ..=172_800_000.).contains(&delta)
}
fn ticket_key(v: &Value) -> String {
    let key = text(v, "ticket_key");
    if key.is_empty() {
        text(v, "ticket")
    } else {
        key
    }
}
fn href(v: &Value) -> String {
    let url = text(v, "url");
    if url.starts_with("https://") || url.starts_with("http://") {
        return url;
    }
    let key = ticket_key(v);
    if !key.is_empty() && key.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') {
        format!("/t/{key}")
    } else {
        String::new()
    }
}
fn ci(v: &Value) -> String {
    let status = text(v, "ci");
    let status = if status.is_empty() {
        text(v, "ci_status")
    } else {
        status
    };
    match status.as_str() {
        "pass" | "success" => "pass",
        "fail" | "failure" | "error" => "fail",
        "pending" | "in_progress" | "queued" => "pending",
        _ => "none",
    }
    .into()
}
#[derive(Clone)]
struct Row {
    title: String,
    detail: String,
    url: String,
    key: String,
    badge: String,
    stamp: String,
    auto_merge: bool,
}
impl Row {
    fn report(v: Value, kind: &str) -> Self {
        let key = ticket_key(&v);
        let url = href(&v);
        let title = ["title", "reason", "detail", "branch", "app", "repo"]
            .into_iter()
            .map(|f| text(&v, f))
            .find(|s| !s.is_empty())
            .unwrap_or_else(|| "Untitled report item".into());
        let detail = match kind {
            "builds" => format!(
                "{} · {} {} · {}",
                text(&v, "app"),
                text(&v, "kind"),
                text(&v, "build_number"),
                text(&v, "status")
            ),
            "jobs" => format!(
                "{} · {} · {} {}",
                text(&v, "repo"),
                text(&v, "state"),
                text(&v, "age"),
                text(&v, "detail")
            ),
            "pull_requests" => format!("{} #{}", text(&v, "repo"), text(&v, "number")),
            "merges" => format!("{} #{} · merged", text(&v, "repo"), text(&v, "number")),
            _ => text(&v, "detail"),
        };
        let badge = if kind == "pull_requests" {
            ci(&v)
        } else {
            String::new()
        };
        let stamp = match kind {
            "pull_requests" => text(&v, "created_at"),
            "merges" => text(&v, "merged_at"),
            "builds" => text(&v, "finished_at"),
            "blocked" => text(&v, "since"),
            _ => String::new(),
        };
        Self {
            title,
            detail,
            url,
            key,
            badge,
            stamp,
            auto_merge: v["auto_merge"] == true,
        }
    }
    fn ticket(t: &Ticket) -> Self {
        Self {
            title: t.title.clone(),
            detail: format!("{} · {}", t.project, t.status.replace('_', " ")),
            url: format!("/t/{}", t.key),
            key: t.key.clone(),
            badge: String::new(),
            stamp: t.updated_at.clone(),
            auto_merge: false,
        }
    }
}

#[component]
pub fn MissionPage() -> impl IntoView {
    let fixture = use_location()
        .pathname
        .get_untracked()
        .starts_with("/design");
    let board = expect_context::<crate::pages::board::BoardState>();
    let data = RwSignal::new(None::<Mission>);
    let error = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(Vec::<i64>::new());
    let revision = RwSignal::new(0_u64);
    let mutation_revision = StoredValue::new(0_u64);
    let request = move || {
        if !fixture
            && !document().hidden()
            && !busy.get_untracked()
            && pending.get_untracked().is_empty()
        {
            revision.update(|r| *r += 1);
        }
    };
    let focus = window_event_listener(leptos::ev::focus, move |_| request());
    let visibility = Closure::<dyn Fn()>::new(request);
    let _ = document()
        .add_event_listener_with_callback("visibilitychange", visibility.as_ref().unchecked_ref());
    let _timer = StoredValue::new_local(gloo_timers::callback::Interval::new(30_000, request));
    let visibility = StoredValue::new_local(visibility);
    on_cleanup(move || {
        focus.remove();
        visibility.with_value(|v| {
            let _ = document().remove_event_listener_with_callback(
                "visibilitychange",
                v.as_ref().unchecked_ref(),
            );
        });
    });
    Effect::new(move |_| {
        revision.track();
        if fixture {
            data.set(Some(crate::fixtures::mission()));
            return;
        }
        busy.set(true);
        let started_at_revision = mutation_revision.get_value();
        let (abort, registration) = AbortHandle::new_pair();
        on_cleanup(move || abort.abort());
        wasm_bindgen_futures::spawn_local(async move {
            let _ = Abortable::new(async move {
                let result = api("GET", "/api/v1/mission", Value::Null).await.and_then(|value| {
                    serde_json::from_value(value).map_err(|_| "Could not read Mission Control data.".into())
                });
                if mutation_revision.get_value() == started_at_revision {
                    match result {
                        Ok(value) => { data.set(Some(value)); error.set(String::new()); }
                        Err(message) => error.set(if data.get_untracked().is_some() {
                            format!("Could not refresh Mission Control. Showing the last available data. {message}")
                        } else { format!("Could not load Mission Control. {message}") }),
                    }
                }
                busy.set(false);
                board.signed_in.set(true);
                if board.loading.get_untracked()
                    && let Ok(projects) = crate::pages::board::all::<tickets_core::domain::Project>("projects").await
                {
                    if board.project.get_untracked() == 0 {
                        board.project.set(projects.iter().find(|p| !p.archived).map(|p| p.id).unwrap_or(0));
                    }
                    board.projects.set(projects);
                    board.loading.set(false);
                }
            }, registration).await;
        });
    });
    let approve = Callback::new(move |ticket: Ticket| {
        if pending.get_untracked().contains(&ticket.id) {
            return;
        }
        let Some(label) = ticket
            .labels
            .iter()
            .find(|l| l.name.eq_ignore_ascii_case("waiting-on-calum"))
        else {
            return;
        };
        let label = label.id;
        mutation_revision.update_value(|r| *r += 1);
        let had_focus = document().active_element().is_some_and(|element| {
            element.get_attribute("aria-label") == Some(format!("Approve {}", ticket.key))
        });
        pending.update(|ids| ids.push(ticket.id));
        if had_focus {
            request_animation_frame(move || {
                if let Some(heading) = document()
                    .get_element_by_id("mission-needs-heading")
                    .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
                {
                    let _ = heading.focus();
                }
            });
        }
        error.set(String::new());
        wasm_bindgen_futures::spawn_local(async move {
            let result = if fixture {
                Ok(Value::Null)
            } else {
                let path = format!("/api/v1/tickets/{}/labels/{label}", ticket.key);
                match api("DELETE", &path, Value::Null).await {
                    Err(e) => Err(e),
                    Ok(_) => match api(
                        "POST",
                        &format!("/api/v1/tickets/{}/comments", ticket.key),
                        json!({"body":"Approved from Mission Control."}),
                    )
                    .await
                    {
                        Ok(v) => Ok(v),
                        Err(e) => match api("POST", &path, Value::Null).await {
                            Ok(_) => Err(format!("The comment failed; approval was undone. {e}")),
                            Err(_) => Err(format!(
                                "The label was removed, but the comment failed. Open the ticket to reconcile approval. {e}"
                            )),
                        },
                    },
                }
            };
            if data.is_disposed() {
                return;
            }
            match result {
                Ok(_) => data.update(|value| {
                    if let Some(value) = value {
                        value.waiting_on_calum.retain(|t| t.id != ticket.id);
                    }
                }),
                Err(e) => error.set(format!("Could not approve {}. {e}", ticket.key)),
            }
            pending.update(|ids| ids.retain(|id| *id != ticket.id));
        });
    });
    let root = NodeRef::<leptos::html::Div>::new();
    use_ascii_reveal(
        root,
        Signal::derive(move || busy.get() && data.get().is_none()),
        true,
    );
    view! {
        <header class="topbar"><h1 class="home-title">"Mission Control"</h1><div class="topbar-actions"><IconButton label="Refresh Mission Control" icon="refresh" disabled=Signal::derive(move ||busy.get() || !pending.get().is_empty()) on_click=Callback::new(move |_|revision.update(|r|*r+=1))/></div></header>
        <div class="page-container mission-page" node_ref=root aria-busy=move ||busy.get().to_string()>
            <Show when=move ||!error.get().is_empty()><div class="home-error" role="alert"><span>{move ||error.get()}</span><button class="button button-secondary" on:click=move |_|revision.update(|r|*r+=1)>"Retry"</button></div></Show>
            {move ||data.get().map(|mission|view!{<MissionContent mission pending approve fixture/>})}
            <Show when=move ||data.get().is_none() && busy.get()><LoadingIndicator label="Loading Mission Control"/></Show>
        </div>
    }
}

#[component]
fn MissionContent(
    mission: Mission,
    pending: RwSignal<Vec<i64>>,
    approve: Callback<Ticket>,
    fixture: bool,
) -> impl IntoView {
    let reports = mission.reports.clone();
    let items = |field: &str| {
        reports
            .iter()
            .flat_map(|r| r.report[field].as_array().cloned().unwrap_or_default())
            .collect::<Vec<_>>()
    };
    let mut ready = items("builds")
        .into_iter()
        .filter(|v| {
            v["status"] == "success"
                && ["testflight", "deploy"].contains(&text(v, "kind").as_str())
                && recent(v)
        })
        .map(|v| Row::report(v, "builds"))
        .collect::<Vec<_>>();
    ready.extend(mission.recently_done.iter().map(Row::ticket));
    let running = items("jobs")
        .into_iter()
        .filter(|v| {
            ["running", "queued", "starting", "in_progress"].contains(&text(v, "state").as_str())
        })
        .map(|v| Row::report(v, "jobs"))
        .collect::<Vec<_>>();
    let mut prs = items("pull_requests")
        .into_iter()
        .filter(|v| ["open", "draft"].contains(&text(v, "state").as_str()))
        .map(|v| Row::report(v, "pull_requests"))
        .collect::<Vec<_>>();
    for ticket in &mission.in_review {
        for pr in &ticket.pull_requests {
            if !["open", "draft"].contains(&text(pr, "state").as_str()) {
                continue;
            }
            let mut pr = pr.clone();
            pr["ticket_key"] = ticket.key.clone().into();
            let mut row = Row::report(pr.clone(), "pull_requests");
            if row.stamp.is_empty() {
                row.stamp = text(&pr, "updated_at");
            }
            if !prs.iter().any(|r| r.url == row.url) {
                prs.push(row);
            }
        }
    }
    let needs = items("needs_calum")
        .into_iter()
        .map(|v| Row::report(v, "needs_calum"))
        .collect::<Vec<_>>();
    let blocked = items("blocked")
        .into_iter()
        .map(|v| Row::report(v, "blocked"))
        .collect::<Vec<_>>();
    let merges = items("merges")
        .into_iter()
        .map(|v| Row::report(v, "merges"))
        .collect::<Vec<_>>();
    let builds = items("builds")
        .into_iter()
        .map(|v| Row::report(v, "builds"))
        .collect::<Vec<_>>();
    let tickets = mission.waiting_on_calum;
    let empty_tickets = tickets.clone();
    view! {
        <div class="mission-intro"><span>"All active projects"{if fixture {" · Sample data"}else{""}}</span><span>"Refreshes every 30 seconds"</span></div>
        <div class="mission-freshness" aria-label="Report freshness">{mission.reports.into_iter().map(|r|view!{<span class:mission-stale=r.stale title=r.received_at.clone()>{format!("{} report {}{}",r.source,age(&r.received_at),if r.stale {" · stale"}else{""})}</span>}).collect_view()}{reports.is_empty().then(||view!{<span>"No reports received yet. Tickets data is available below."</span>})}</div>
        <section class="mission-section"><h2 id="mission-needs-heading" tabindex="-1">"Needs Calum"</h2>
            {move ||tickets.iter().filter(|t|!pending.get().contains(&t.id)).cloned().map(|t|{let row=Row::ticket(&t);view!{<div class="mission-approval"><MissionRow row/><button class="button button-secondary" aria-label=format!("Approve {}",t.key) on:click=move |_|approve.run(t.clone())>"Approve"</button></div>}}).collect_view()}
            <Show when=move ||!pending.get().is_empty()><p class="mission-empty" role="status">"Approving…"</p></Show>
            <MissionRows rows=needs.clone()/>
            {move ||(pending.get().is_empty() && needs.is_empty() && empty_tickets.iter().all(|t|pending.get().contains(&t.id))).then(||view!{<p class="mission-empty">"Nothing needs your approval."</p>})}
        </section>
        <MissionSection title="Ready to test" rows=ready empty="No successful builds or completed tickets in the last 48 hours."/>
        <MissionSection title="Running jobs" rows=running empty="No jobs running."/>
        <MissionSection title="Open PRs" rows=prs empty="No open pull requests reported or linked to tickets in review."/>
        <MissionSection title="Blocked" rows=blocked empty="No blockers reported."/>
        <MissionSection title="Recent merges" rows=merges empty="No recent merges reported."/>
        <MissionSection title="Builds" rows=builds empty="No builds reported yet."/>
    }
}
#[component]
fn MissionSection(title: &'static str, rows: Vec<Row>, empty: &'static str) -> impl IntoView {
    let count = rows.len();
    view! {<section class="mission-section"><h2>{title}<span class="mission-count">{count}</span></h2><MissionRows rows/>{(count==0).then(||view!{<p class="mission-empty">{empty}</p>})}</section>}
}
#[component]
fn MissionRows(rows: Vec<Row>) -> impl IntoView {
    rows.into_iter()
        .map(|row| view! {<MissionRow row/>})
        .collect_view()
}
#[component]
fn MissionRow(row: Row) -> impl IntoView {
    let link = (!row.url.is_empty()).then(|| row.url.clone());
    let key = (!row.key.is_empty()).then(|| row.key.clone());
    view! {<div class="mission-row"><div class="mission-row-main"><a href=link class="mission-row-title">{crate::ui::is_github_url(&row.url).then(||view!{<Icon name="github"/>})}<span>{row.title}</span></a><span class="mission-detail">{row.detail}</span></div><div class="mission-row-meta">
        {key.map(|key|view!{<a class="mission-key" href=format!("/t/{key}")>{key.clone()}</a>})}
        {(!row.badge.is_empty()).then(||view!{<span class=format!("mission-ci mission-ci-{}",row.badge)>{format!("CI {}",row.badge)}</span>})}
        {row.auto_merge.then(||view!{<span>"Auto-merge"</span>})}
        {(!row.stamp.is_empty()).then(||view!{<time datetime=row.stamp.clone() title=row.stamp.clone()>{age(&row.stamp)}</time>})}
    </div></div>}
}
