//! Operate view: current work and honestly bounded completion history.
use crate::{
    components::*,
    pages::board::{BoardComposer, BoardState, all},
    ui::api,
};
use futures_util::future::{AbortHandle, Abortable};
use leptos::prelude::*;
use leptos_router::{
    components::A,
    hooks::{use_location, use_navigate, use_query_map},
};
use serde_json::Value;
use tickets_core::{
    analytics::{Home, HomeProject, HomeTicket, Week},
    domain::Project,
};

fn metric_name(metric: &str) -> &'static str {
    match metric {
        "in_flight" => "In flight",
        "review" => "In review",
        "blocked" => "Blocked",
        "created" => "Created",
        "completed" => "Completed",
        _ => "Open work",
    }
}
fn link(base: &str, project: &str, metric: &str) -> String {
    format!(
        "{base}?metric={metric}{}",
        if project.is_empty() {
            String::new()
        } else {
            format!("&project={project}")
        }
    )
}
fn date(s: &str) -> String {
    s.get(5..10).unwrap_or(s).replace('-', "/")
}

#[component]
pub fn HomePage() -> impl IntoView {
    let board = expect_context::<BoardState>();
    let location = use_location();
    let fixture = location.pathname.get_untracked().starts_with("/design");
    // Keep query-only navigation on the current alias so the router retains Home.
    let base = if fixture {
        "/design/home"
    } else if location.pathname.get_untracked() == "/" {
        "/"
    } else {
        "/home"
    };
    let query = use_query_map();
    let scope = Signal::derive(move || query.get().get("project").unwrap_or_default());
    let metric = Signal::derive(move || query.get().get("metric").unwrap_or_default());
    let response = RwSignal::new(None::<Home>);
    let error = RwSignal::new(String::new());
    let busy = RwSignal::new(true);
    // The page reveal belongs to initial settlement, never to refresh activity.
    let settled = RwSignal::new(false);
    let rendered_scope = RwSignal::new(String::new());
    let rendered_metric = RwSignal::new(String::new());
    let revision = RwSignal::new(0_u64);
    Effect::new(move |_| {
        let search = query.get().to_query_string();
        revision.track();
        board.catchup_revision.track();
        if fixture {
            response.set(Some(
                serde_json::from_str(include_str!("../../../../docs/fixtures/home.json"))
                    .expect("Home fixture"),
            ));
            rendered_scope.set(scope.get_untracked());
            rendered_metric.set(metric.get_untracked());
            busy.set(false);
            settled.set(true);
            return;
        }
        let request_scope = scope.get_untracked();
        let request_metric = metric.get_untracked();
        busy.set(true);
        error.set(String::new());
        let (abort, registration) = AbortHandle::new_pair();
        on_cleanup(move || abort.abort());
        wasm_bindgen_futures::spawn_local(async move {
            let _ = Abortable::new(
                async move {
                    // One gate for all Home data, including its project selector/sidebar.
                    // Home never downloads the board's ticket collection.
                    let url = format!("/api/v1/analytics/home{search}");
                    let (analytics, projects) = futures_util::join!(
                        api("GET", &url, Value::Null),
                        all::<Project>("projects")
                    );
                    let result = analytics
                        .and_then(|v| {
                            serde_json::from_value::<Home>(v)
                                .map_err(|_| "Couldn’t read Home data.".into())
                        })
                        .and_then(|home| projects.map(|projects| (home, projects)));
                    match result {
                        Ok((data, projects)) => {
                            if board.project.get_untracked() == 0 {
                                board.project.set(
                                    projects
                                        .iter()
                                        .find(|p| !p.archived)
                                        .map(|p| p.id)
                                        .unwrap_or(0),
                                );
                            }
                            board.projects.set(projects);
                            board.loading.set(false);
                            board.signed_in.set(true);
                            rendered_scope.set(request_scope);
                            rendered_metric.set(request_metric);
                            response.set(Some(data));
                        }
                        Err(e) => error.set(e),
                    }
                    busy.set(false);
                    settled.set(true);
                },
                registration,
            )
            .await;
        });
    });
    let navigate = use_navigate();
    let change_scope = Callback::new(move |value: String| {
        navigate(
            &if value.is_empty() {
                base.into()
            } else {
                format!("{base}?project={value}")
            },
            Default::default(),
        )
    });
    let create = Callback::new(move |_| {
        if fixture {
            expect_context::<DesignState>().new_ticket(crate::fixtures::Status::Todo);
        } else {
            board.refresh();
            board.composer.set(Some(crate::fixtures::Status::Todo));
        }
    });
    let reveal_root = NodeRef::<leptos::html::Div>::new();
    use_ascii_reveal(reveal_root, Signal::derive(move || !settled.get()), true);
    view! {
        <header class="topbar"><h1 class="home-title">"Home"</h1><div class="topbar-actions"><button class="button button-primary" disabled=move||!fixture && board.projects.get().is_empty() on:click=move|_|create.run(())><Icon name="plus"/>"New ticket"</button></div></header>
        <div node_ref=reveal_root class="home-page" aria-busy=move||busy.get().to_string()>
            <div class="home-toolbar"><SelectField label="Home project" value=scope options=Signal::derive(move||{
                let mut items=vec![SelectOption::new("","All active projects")];
                items.extend(board.projects.get().into_iter().filter(|p|!p.archived).map(|p|SelectOption::new(p.id.to_string(),p.name)));items
            }) on_change=change_scope/><span class="home-period">"8 weeks · UTC"</span><span class="home-freshness">{move||response.get().map(|h|format!("Updated {} UTC",h.as_of.get(11..16).unwrap_or("")))}</span><Show when=move||busy.get()><LoadingIndicator label="Refreshing Home"/></Show><IconButton label="Refresh Home" icon="refresh" on_click=Callback::new(move|_|{crate::ui::invalidate_reads();revision.update(|r|*r+=1)}) disabled=Signal::derive(move||busy.get())/></div>
            <Show when=move||!error.get().is_empty()><div class="home-error" role="alert"><span>{move||if response.get().is_some(){format!("Couldn’t refresh. Showing the last update. {}",error.get())}else{error.get()}}</span><button class="button button-secondary" on:click=move|_|{crate::ui::invalidate_reads();revision.update(|r|*r+=1)}>"Retry"</button></div></Show>
            {move||response.get().map(|home|view!{<HomeContent home scope=rendered_scope.get() metric=rendered_metric.get() base/>}.into_any()).unwrap_or_else(||if busy.get() || error.get().is_empty() {view!{<HomeSkeleton/>}.into_any()}else{().into_any()})}
        </div>
        {let shown=present_value(board.composer.into());move||shown.get().map(|status|view!{<BoardComposer status/>})}
    }
}

#[component]
fn HomeContent(home: Home, scope: String, metric: String, base: &'static str) -> impl IntoView {
    if home.projects.is_empty() {
        return view!{<div class="home-empty"><h2>"Start with a project"</h2><p>"Create a project to organize your first tickets. History will build as work moves."</p><A href="/projects?new=true" attr:class="button button-secondary">"New project"</A></div>}.into_any();
    }
    let last = home.weeks.last();
    let completed = last.and_then(|w| w.completed);
    let partial = last.is_some_and(|w| w.coverage == "partial");
    let projects = home
        .projects
        .iter()
        .filter(|p| scope.is_empty() || p.id.to_string() == scope)
        .cloned()
        .collect::<Vec<_>>();
    let history = home
        .history_from
        .get(..10)
        .unwrap_or(&home.history_from)
        .to_string();
    let selected = !metric.is_empty();
    let metric_title = metric_name(&metric);
    let cohort = home.cohort.clone();
    let cohort_total = home.cohort_total;
    let query = use_query_map();
    let cohort_context = if matches!(metric.as_str(), "completed" | "created") {
        let index = query
            .get_untracked()
            .get("week")
            .and_then(|w| w.parse::<usize>().ok())
            .unwrap_or(7);
        home.weeks
            .get(index)
            .map(|w| {
                format!(
                    "Week of {} · UTC. Tickets may have moved or reopened since then.",
                    &w.start[..10]
                )
            })
            .unwrap_or_default()
    } else if metric == "inventory" {
        let status = query.get_untracked().get("status").unwrap_or_default();
        format!(
            "Current open work{}.",
            if status.is_empty() {
                String::new()
            } else {
                format!(" · {}", status.replace('_', " "))
            }
        )
    } else {
        "Tickets contributing to the selected metric.".into()
    };
    let next = home.next_cursor.map(|cursor| {
        let mut q = query.get_untracked();
        q.insert("cursor", cursor.to_string());
        format!("{base}{}", q.to_query_string())
    });
    view! {
        <section class="home-metrics" aria-label="Work overview">
            {[("In flight",Some(home.in_flight),"Progress + review","in_flight"),("In review",Some(home.in_review),"Ready for a decision","review"),("Blocked",Some(home.blocked),"Unresolved dependencies","blocked"),("Completed this week",completed,if partial{"Since capture began · so far"}else{"So far · observed transitions"},"completed")].into_iter().map(|(name,value,note,key)|view!{
                <A href=link(base,&scope,key) attr:class="home-metric"><span>{name}</span><strong>{value.map(|n|n.to_string()).unwrap_or_else(||"—".into())}</strong><small>{note}</small></A>
            }).collect_view()}
        </section>
        {selected.then(||view!{
            <section class="home-cohort" aria-label="Selected metric tickets"><div class="home-section-heading"><h2>{metric_title}<span class="count">{cohort_total}</span></h2><A href=if scope.is_empty(){base.into()}else{format!("{base}?project={scope}")} attr:class="button button-ghost">"Close details"</A></div>
            <p class="home-note">{cohort_context}</p>
            <TicketRows items=cohort fixture=base.starts_with("/design") empty="No tickets in this cohort."/>
            {next.map(|url|view!{<A href=url attr:class="button button-secondary">"Next 50 tickets"</A>})}
            {(query.get_untracked().get("cursor").is_some()).then(||{let mut q=query.get_untracked();q.remove("cursor");view!{<A href=format!("{base}{}",q.to_query_string()) attr:class="button button-ghost">"First page"</A>}})}
            </section>
        })}
        <div class="home-charts">
            <section class="home-chart" aria-labelledby="workload-title"><div class="home-section-heading"><h2 id="workload-title">"Open work by project"</h2><span class="home-note">"Current"</span></div><p class="home-note">"Where work is waiting, by status."</p><ProjectBars projects base/>
                <div class="home-legend">{[("Backlog","backlog"),("Todo","todo"),("In progress","in_progress"),("In review","in_review")].into_iter().map(|(label,status)|view!{<span><i class=format!("chart-swatch mark-{status}")/>{label}</span>}).collect_view()}</div>
            </section>
            <section class="home-chart" aria-labelledby="flow-title"><div class="home-section-heading"><h2 id="flow-title">"Created / completed"</h2><span class="home-note">"Weekly"</span></div><p class="home-note">"Native arrivals and recorded completions."</p><WeeklyFlow weeks=home.weeks scope=scope.clone() base/>
                <p class="home-coverage">{format!("History starts {history}. Earlier weeks are unavailable.")}</p>
            </section>
        </div>
        <div class="home-queues">
            <section><div class="home-section-heading"><h2>"In review"<span class="count">{home.in_review}</span></h2><A href=link(base,&scope,"review")>"View all"</A></div><TicketRows items=home.review_queue fixture=base.starts_with("/design") empty="Nothing waiting for review."/></section>
            <section><div class="home-section-heading"><h2>"Blocked"<span class="count">{home.blocked}</span></h2><A href=link(base,&scope,"blocked")>"View all"</A></div><TicketRows items=home.blocked_queue fixture=base.starts_with("/design") empty="No unresolved dependencies."/></section>
        </div>
        {home.projects.is_empty().then(||view!{<div class="home-empty"><h2>"Start with a project"</h2><p>"Create a project to organize your first tickets. History will build as work moves."</p><A href="/projects?new=true" attr:class="button button-secondary">"New project"</A></div>})}
    }.into_any()
}

#[component]
fn ProjectBars(projects: Vec<HomeProject>, base: &'static str) -> impl IntoView {
    let max = projects
        .iter()
        .map(HomeProject::total)
        .max()
        .unwrap_or(0)
        .max(1) as f64;
    view! {<div class="project-chart" role="list" aria-label="Open work by project, shared count scale">
        {projects.into_iter().map(|p|{
            let values=[("backlog","Backlog",p.backlog),("todo","Todo",p.todo),("in_progress","In progress",p.in_progress),("in_review","In review",p.in_review)];
            let description=values.iter().map(|(_,label,n)|format!("{label}: {n}")).collect::<Vec<_>>().join(", ");
            let counts_label=format!("Status counts for {}",p.name);let project=p.id.to_string();let total=p.total();
            view!{<div class="project-chart-row" role="listitem"><div class="project-chart-label"><ProjectIcon name=p.name.clone() icon=p.icon.unwrap_or_default() color=p.color/><A href=link(base,&project,"inventory")>{p.name}</A><span class="mono">{total}</span></div>
            <div class="project-chart-track" role="img" aria-label=description.clone() title=description>{values.into_iter().map(|(status,_,n)|view!{<span class=format!("mark-{status}") style=format!("width:{}%",n as f64/max*100.)/>}).collect_view()}</div>
            <details class="chart-project-details"><summary aria-label=counts_label>"Counts"</summary><div>{values.into_iter().map(|(status,label,n)|view!{<A href=format!("{}&status={status}",link(base,&project,"inventory"))>{format!("{label} · {n}")}</A>}).collect_view()}</div></details></div>}
        }).collect_view()}
    </div>}
}

#[component]
fn WeeklyFlow(weeks: Vec<Week>, scope: String, base: &'static str) -> impl IntoView {
    let max = weeks
        .iter()
        .flat_map(|w| [w.created, w.completed])
        .flatten()
        .max()
        .unwrap_or(0)
        .max(4);
    let scale = 120. / max as f64;
    let imported: u64 = weeks.iter().filter_map(|w| w.imported).sum();
    view! {
        <div class="home-legend"><span><i class="chart-swatch mark-created"/>"Created"</span><span><i class="chart-swatch mark-completed"/>"Completed"</span><span class="home-note">{format!("{imported} imported arrivals excluded")}</span></div>
        <svg class="flow-chart" viewBox="0 0 320 166" role="img" aria-labelledby="flow-svg-title flow-svg-desc"><title id="flow-svg-title">"Weekly created and completed tickets"</title><desc id="flow-svg-desc">"Paired bars on a shared count scale from zero. The table below provides counts and ticket links. A dash means history is unavailable."</desc>
            {[0,max.div_ceil(2),max].into_iter().map(|n|{let y=134.-n as f64*scale;view!{<line x1="30" x2="316" y1=y y2=y class="chart-grid"/><text x="23" y=y+4. text-anchor="end">{n}</text>}}).collect_view()}
            {weeks.iter().enumerate().map(|(i,w)|{let x=32.+i as f64*35.;let label=if i==7{"So far".into()}else{date(&w.start)};view!{<g><title>{format!("{}: created {}, completed {} ({})",w.start,w.created.map(|n|n.to_string()).unwrap_or_else(||"unavailable".into()),w.completed.map(|n|n.to_string()).unwrap_or_else(||"unavailable".into()),w.coverage)}</title>
                {w.created.map(|n|view!{<rect x=x y=134.-n as f64*scale width="9" height=n as f64*scale rx="2" class="mark-created"/>})}
                {w.completed.map(|n|view!{<rect x=x+11. y=134.-n as f64*scale width="9" height=n as f64*scale rx="2" class="mark-completed"/>})}
                {w.created.is_none().then(||view!{<text x=x+10. y="126" text-anchor="middle">"—"</text>})}
                {(i%2==0 && i<6 || i==7).then(||view!{<text class="flow-week-label" x=x+10. y="158" text-anchor="middle">{label}</text>})}</g>}}).collect_view()}
        </svg>
        <details class="chart-data"><summary>"View counts and tickets"</summary><table><caption class="sr-only">"Weekly flow in UTC"</caption><thead><tr><th>"Week of"</th><th>"Created"</th><th>"Completed"</th><th>"Coverage"</th></tr></thead><tbody>{weeks.into_iter().enumerate().map(|(i,w)|view!{<tr><th scope="row">{date(&w.start)}{if i==7{" · so far"}else{""}}</th>{[("created",w.created),("completed",w.completed)].into_iter().map(|(m,n)|view!{<td>{n.map(|n|view!{<A href=format!("{}&week={i}",link(base,&scope,m))>{n}</A>}.into_any()).unwrap_or_else(||"—".into_any())}</td>}).collect_view()}<td>{match w.coverage.as_str(){"none"=>"Unavailable","partial"=>"Since capture",_=>"Recorded"}}</td></tr>}).collect_view()}</tbody></table></details>
    }
}

#[component]
fn TicketRows(items: Vec<HomeTicket>, fixture: bool, empty: &'static str) -> impl IntoView {
    let is_empty = items.is_empty();
    view! {<div class="home-ticket-rows">{is_empty.then(||view!{<p class="home-queue-empty">{empty}</p>})}{items.into_iter().map(|t|{
        let href=t.ticket_id.map(|id|if fixture{format!("/design/ticket?id={id}")}else{format!("/ticket?id={id}")});
        view!{<div class="home-ticket-row">{href.map(|href|view!{<A href attr:class="home-ticket-link"><span class="mono">{t.key.unwrap_or_default()}</span><span>{t.title.unwrap_or_default()}</span></A>}.into_any()).unwrap_or_else(||view!{<span class="home-note">"Deleted ticket"</span>}.into_any())}
            {t.blocker_id.map(|id|view!{<div class="home-blocker"><Icon name="blocked"/><span>"Blocked by "</span><A href=format!("/ticket?id={id}")>{t.blocker_key.unwrap_or_default()}</A>{(t.blocker_count>1).then(||format!(" +{}",t.blocker_count-1))}</div>})}
        </div>}
    }).collect_view()}</div>}
}

#[component]
fn HomeSkeleton() -> impl IntoView {
    view! {<div class="home-loading" role="status" aria-label="Loading Home"><div class="home-metrics">{(0..4).map(|_|view!{<div class="home-metric"><AsciiSkeleton lines=1 class="skeleton-short"/><AsciiSkeleton lines=1 class="skeleton-heading"/><AsciiSkeleton lines=1 class="skeleton-meta"/></div>}).collect_view()}</div><div class="home-charts">{(0..2).map(|_|view!{<div class="home-chart"><AsciiSkeleton lines=1 class="skeleton-heading"/><AsciiSkeleton rows=12 class="home-plot-skeleton"/><AsciiSkeleton lines=1 class="skeleton-short"/></div>}).collect_view()}</div><div class="home-queues">{(0..2).map(|_|view!{<div><AsciiSkeleton lines=1 class="skeleton-heading"/>{(0..5).map(|_|view!{<div class="home-ticket-row"><AsciiSkeleton lines=1/></div>}).collect_view()}</div>}).collect_view()}</div></div>}
}
