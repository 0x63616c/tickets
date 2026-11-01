//! Charts consume only the shared, cached Home rollup. No ticket collection reads.
use super::home::link;
use crate::components::*;
use leptos::prelude::*;
use leptos_router::components::A;
use tickets_core::analytics::{Home, HomeProject, Week};

#[component]
pub fn AnalyticsPage() -> impl IntoView {
    view! { <super::home::OverviewPage analytics=true/> }
}

#[component]
fn ChartCard(
    title: &'static str,
    subtitle: String,
    metric: String,
    children: Children,
) -> impl IntoView {
    view! {<section class="analytics-card"><div class="chart-card-heading"><div><h2>{title}</h2><p class="home-note">{subtitle}</p></div><strong class="chart-card-metric">{metric}</strong></div>{children()}</section>}
}

pub(super) fn scoped_projects(home: &Home, scope: &str) -> Vec<HomeProject> {
    home.projects
        .iter()
        .filter(|p| scope.is_empty() || p.id.to_string() == scope)
        .cloned()
        .collect()
}

#[component]
pub(super) fn AnalyticsContent(home: Home, scope: String, base: &'static str) -> impl IntoView {
    let projects = scoped_projects(&home, &scope);
    let total: u64 = projects.iter().map(HomeProject::total).sum();
    let completed: u64 = home.weeks.iter().filter_map(|w| w.completed).sum();
    let observed = home.weeks.iter().any(|w| w.completed.is_some());
    let history = home
        .history_from
        .get(..10)
        .unwrap_or(&home.history_from)
        .to_string();
    let imported: u64 = home.weeks.iter().filter_map(|w| w.imported).sum();
    view! {
        <div class="analytics-intro"><div><h2>"Work over time"</h2><p>"Compare arrivals with delivery, then see where open work is waiting."</p></div><A href=home_link(base,&scope) attr:class="button button-secondary">"Back to Home"</A></div>
        <div class="analytics-grid">
            <ChartCard title="Created / completed" subtitle="Weekly flow · 8 UTC Monday weeks".into() metric=if observed{format!("{completed} completed")}else{"— completed".into()}>
                <div class="home-legend"><span><i class="chart-swatch mark-created"/>"Created"</span><span><i class="chart-swatch mark-completed"/>"Completed"</span></div>
                <WeeklyFlow weeks=home.weeks scope=scope.clone() base/>
                <p class="home-coverage">{format!("History starts {history}. — means unavailable; * marks a partial capture week. {imported} imported arrivals excluded.")}</p>
            </ChartCard>
            <ChartCard title="Open work by project" subtitle="Current inventory · shared ticket scale".into() metric=format!("{total} open")>
                <div class="home-legend">{[("Backlog","backlog"),("Todo","todo"),("In progress","in_progress"),("In review","in_review")].into_iter().map(|(label,status)|view!{<span><i class=format!("chart-swatch mark-{status}")/>{label}</span>}).collect_view()}</div>
                <ProjectInventory projects base/>
                <p class="home-coverage">"Open tickets only. Completed and canceled work is excluded."</p>
            </ChartCard>
        </div>
    }
}

/// Four equal, integer intervals; all ticks use the same scale as the marks.
fn home_link(base: &str, scope: &str) -> String {
    let path = if base.starts_with("/design") {
        "/design/home"
    } else {
        "/"
    };
    if scope.is_empty() {
        path.into()
    } else {
        format!("{path}?project={scope}")
    }
}
fn scale_max(value: u64) -> u64 {
    value.max(4).div_ceil(4) * 4
}
fn count(value: Option<u64>) -> String {
    value
        .map(|n| n.to_string())
        .unwrap_or_else(|| "Unavailable".into())
}
fn coverage(value: &str) -> &'static str {
    match value {
        "none" => "Unavailable",
        "partial" => "Since capture",
        _ => "Recorded",
    }
}
fn short_name(value: &str, limit: usize) -> String {
    if value.chars().count() > limit {
        format!("{}…", value.chars().take(limit - 1).collect::<String>())
    } else {
        value.into()
    }
}
fn chart_focus() -> RwSignal<Option<usize>> {
    let active = RwSignal::new(None);
    let listener = window_event_listener(leptos::ev::keydown, move |event| {
        if event.key() == "Escape" {
            active.set(None);
        }
    });
    on_cleanup(move || listener.remove());
    active
}
fn date(value: &str) -> String {
    value.get(5..10).unwrap_or(value).replace('-', "/")
}

#[component]
fn WeeklyFlow(weeks: Vec<Week>, scope: String, base: &'static str) -> impl IntoView {
    let max = scale_max(
        weeks
            .iter()
            .flat_map(|w| [w.created, w.completed])
            .flatten()
            .max()
            .unwrap_or(0),
    );
    let scale = 180. / max as f64;
    let active = chart_focus();
    let tooltip_weeks = StoredValue::new(weeks.clone());
    let has_history = weeks
        .iter()
        .any(|w| w.created.is_some() || w.completed.is_some());
    let zero = has_history
        && weeks
            .iter()
            .flat_map(|w| [w.created, w.completed])
            .flatten()
            .all(|n| n == 0);
    view! {
        <div class="chart-axis-label">"Tickets"</div>
        <div class="chart-plot-wrap"><svg class="analytics-plot" viewBox="0 0 560 240" role="img" aria-label="Weekly created and completed tickets. Shared scale starts at zero; exact values and coverage are in the data table.">
            {(0..=4).map(|i|{let n=max/4*i;let y=200.-n as f64*scale;view!{<g><line x1="36" x2="548" y1=y y2=y class="chart-grid"/><text x="28" y=y+4. text-anchor="end">{n}</text></g>}}).collect_view()}
            {weeks.iter().enumerate().map(|(i,w)|{
                let x=48.+i as f64*63.; let label=format!("Week of {} · Created: {} · Completed: {} · {}",&w.start[..10],count(w.created),count(w.completed),coverage(&w.coverage));
                view!{<g class="chart-point" tabindex="0" aria-label=label on:mouseenter=move|_|active.set(Some(i)) on:mouseleave=move|_|active.set(None) on:focus=move|_|active.set(Some(i)) on:blur=move|_|active.set(None)>
                    <rect class="chart-hit" x=x-6. y="16" width="54" height="188"/>
                    {w.created.map(|n|view!{<rect x=x y=200.-n as f64*scale width="17" height=n as f64*scale rx="2" class="mark-created"/>})}
                    {w.completed.map(|n|view!{<rect x=x+21. y=200.-n as f64*scale width="17" height=n as f64*scale rx="2" class="mark-completed"/>})}
                    {w.created.is_none().then(||view!{<text x=x+18. y="188" text-anchor="middle">"—"</text>})}
                    <text class="flow-week-label" x=x+18. y="222" text-anchor="middle">{format!("{}{}",date(&w.start),if w.coverage=="partial"{"*"}else{""})}</text>

                </g>}
            }).collect_view()}
        </svg>
        {move||active.get().and_then(|i|tooltip_weeks.get_value().get(i).cloned()).map(|w|view!{<div class="chart-tooltip" role="tooltip"><strong>{format!("Week of {} · {}",date(&w.start),coverage(&w.coverage))}</strong><span>{format!("Created: {}",count(w.created))}</span><span>{format!("Completed: {}",count(w.completed))}</span></div>})}
        </div>
        <div class="chart-axis-end">"Week of · UTC · current week in progress"</div>
        {(!has_history).then(||view!{<p class="chart-empty" role="status">"No recorded history yet. Weekly flow will appear as work moves."</p>})}
        {zero.then(||view!{<p class="chart-empty">"No native arrivals or recorded completions in this period."</p>})}
        <details class="chart-data"><summary>"View weekly counts and tickets"</summary><table><caption class="sr-only">"Weekly flow in UTC; unavailable is not zero"</caption><thead><tr><th>"Week of"</th><th>"Created"</th><th>"Completed"</th><th>"Coverage"</th></tr></thead><tbody>{weeks.into_iter().enumerate().map(|(i,w)|view!{<tr><th scope="row">{date(&w.start)}</th>{[("created",w.created),("completed",w.completed)].into_iter().map(|(m,n)|view!{<td>{n.map(|n|view!{<A href=format!("{}&week={i}",link(base,&scope,m))>{n}</A>}.into_any()).unwrap_or_else(||"—".into_any())}</td>}).collect_view()}<td>{match w.coverage.as_str(){"none"=>"Unavailable","partial"=>"Since capture",_=>"Recorded"}}</td></tr>}).collect_view()}</tbody></table></details>
    }
}

#[component]
fn ProjectInventory(projects: Vec<HomeProject>, base: &'static str) -> impl IntoView {
    let max = scale_max(projects.iter().map(HomeProject::total).max().unwrap_or(0));
    let height = projects.len().max(3) * 48 + 48;
    let total: u64 = projects.iter().map(HomeProject::total).sum();
    let active = chart_focus();
    let tooltip_projects = StoredValue::new(projects.clone());
    view! {
        <div class="chart-axis-label">"Projects"</div>
        <div class="chart-plot-wrap"><svg class="inventory-plot" viewBox=format!("0 0 560 {height}") role="img" aria-label=format!("{total} open tickets by project and status. Shared scale from zero to {max} tickets. Exact counts and links in the data table.")>
            {(0..=4).map(|i|{let n=max/4*i;let x=120.+n as f64/max as f64*416.;view!{<g><line x1=x x2=x y1="8" y2=height-32 class="chart-grid"/><text x=x y=height-12 text-anchor="middle">{n}</text></g>}}).collect_view()}
            {projects.iter().enumerate().map(|(i,p)|{
                let y=16.+i as f64*48.;let mut offset=120.; let label=format!("{}: Backlog {}, Todo {}, In progress {}, In review {}",p.name,p.backlog,p.todo,p.in_progress,p.in_review);
                view!{<g class="chart-point" tabindex="0" aria-label=label on:mouseenter=move|_|active.set(Some(i)) on:mouseleave=move|_|active.set(None) on:focus=move|_|active.set(Some(i)) on:blur=move|_|active.set(None)><rect class="chart-hit" x="0" y=y-8. width="550" height="40"/><text x="108" y=y+15. text-anchor="end"><tspan class="project-name-wide">{short_name(&p.name,13)}</tspan><tspan class="project-name-small">{short_name(&p.name,9)}</tspan></text>
                    {[("backlog",p.backlog),("todo",p.todo),("in_progress",p.in_progress),("in_review",p.in_review)].into_iter().map(|(status,n)|{let x=offset;let width=n as f64/max as f64*416.;offset+=width;view!{<rect x=x y=y width=width height="22" class=format!("mark-{status}")/>}}).collect_view()}

                </g>}
            }).collect_view()}
        </svg>
        {move||active.get().and_then(|i|tooltip_projects.get_value().get(i).cloned()).map(|p|view!{<div class="chart-tooltip" role="tooltip"><strong>{format!("{} · {} open",p.name,p.total())}</strong><span>{format!("Backlog: {} · Todo: {}",p.backlog,p.todo)}</span><span>{format!("In progress: {} · In review: {}",p.in_progress,p.in_review)}</span></div>})}
        </div><div class="chart-axis-end">"Open tickets"</div>
        {(total==0).then(||view!{<p class="chart-empty">"No open work in this scope."</p>})}
        <details class="chart-data"><summary>"View project counts and tickets"</summary><table><caption class="sr-only">"Open tickets by project and status"</caption><thead><tr><th>"Project"</th><th>"Backlog"</th><th>"Todo"</th><th>"Progress"</th><th>"Review"</th></tr></thead><tbody>{projects.into_iter().map(|p|{let project=p.id.to_string();view!{<tr><th scope="row">{p.name}</th>{[("backlog",p.backlog),("todo",p.todo),("in_progress",p.in_progress),("in_review",p.in_review)].into_iter().map(|(status,n)|view!{<td><A href=format!("{}&status={status}",link(base,&project,"inventory"))>{n}</A></td>}).collect_view()}</tr>}}).collect_view()}</tbody></table></details>
    }
}

#[component]
pub(super) fn AnalyticsSkeleton() -> impl IntoView {
    view! {<div class="analytics-loading" aria-hidden="true"><div class="analytics-intro"><div><AsciiSkeleton lines=1 class="skeleton-heading"/><AsciiSkeleton lines=1/></div></div><div class="analytics-grid">{[("Created / completed","Weekly flow · 8 UTC Monday weeks"),("Open work by project","Current inventory · shared ticket scale")].into_iter().map(|(title,subtitle)|view!{<ChartCard title subtitle=subtitle.into() metric="".into()><div class="home-legend"><AsciiSkeleton lines=1 class="skeleton-meta"/></div><div class="chart-axis-label"><AsciiSkeleton lines=1 class="skeleton-short"/></div><div class="analytics-plot-skeleton"><AsciiSkeleton rows=12/></div><div class="chart-axis-end"><AsciiSkeleton lines=1 class="skeleton-short"/></div><AsciiSkeleton lines=1 class="skeleton-heading"/><div class="home-coverage"><AsciiSkeleton lines=2/></div></ChartCard>}).collect_view()}</div></div>}
}
