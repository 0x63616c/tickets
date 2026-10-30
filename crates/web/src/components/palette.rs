//! Workspace command palette. Fixture routes exercise the same interaction surface.
use super::*;
use crate::fixtures::Priority;
use crate::{
    pages::{
        board::{BoardState, BoardTicket},
        settings::{apply_appearance, persist, stored},
    },
    ui::api,
};
use leptos::{html, prelude::*};
use leptos_router::hooks::{use_location, use_navigate};
use serde_json::{Value, json};
use tickets_core::{
    domain::Project,
    search::{SearchQuery, SearchTicket, fuzzy_match},
};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::wasm_bindgen::JsCast;

const RECENT: &str = "tickets.recently-viewed";
fn recent_ids(fixture: bool) -> Vec<i64> {
    if fixture {
        return vec![7, 10];
    }
    let mut ids = serde_json::from_str::<Vec<i64>>(&stored(RECENT, "[]")).unwrap_or_default();
    ids.retain(|id| *id > 0);
    ids.truncate(20);
    ids
}
fn remember(id: i64) {
    let mut ids = recent_ids(false);
    ids.retain(|v| *v != id);
    ids.insert(0, id);
    ids.truncate(20);
    persist(RECENT, &serde_json::to_string(&ids).unwrap_or_default());
}
pub(super) fn track_recent_views() {
    let location = use_location();
    let board = expect_context::<BoardState>();
    let last = StoredValue::new(String::new());
    Effect::new(move |_| {
        let path = location.pathname.get();
        let search = location.search.get();
        let tickets = board.tickets.get();
        let route = format!("{path}{search}");
        if route == last.get_value() {
            return;
        }
        let id = if let Some(key) = path.strip_prefix("/t/") {
            tickets
                .iter()
                .find(|t| t.key.eq_ignore_ascii_case(key))
                .map(|t| t.id)
        } else if path == "/ticket" {
            search
                .trim_start_matches('?')
                .split('&')
                .find_map(|p| p.strip_prefix("id="))
                .and_then(|id| id.parse().ok())
        } else {
            None
        };
        if let Some(id) = id {
            remember(id);
            last.set_value(route);
        }
    });
}
fn compact(t: &BoardTicket) -> SearchTicket {
    SearchTicket {
        id: t.id,
        project_id: t.project_id,
        key: t.key.clone(),
        title: t.title.clone(),
        status: t.status,
        priority: t.priority,
        assignee: t.assignee.clone(),
        updated_at: t.updated_at.clone(),
        external_url: t.external_url.clone(),
    }
}
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Search,
    Actions,
    Status,
    Priority,
    Project,
}
#[derive(Clone)]
enum Action {
    Open(SearchTicket),
    Actions(SearchTicket),
    Navigate(String),
    Project(Project),
    Create,
    Theme,
    Mode(Mode),
    Status(Status),
    Priority(Priority),
    Move(i64),
    Copy(bool),
    GitHub(String),
}
#[derive(Clone)]
struct Row {
    group: &'static str,
    title: String,
    meta: String,
    hint: &'static str,
    icon: &'static str,
    ticket: Option<SearchTicket>,
    status: Option<Status>,
    priority: Option<Priority>,
    project: Option<Project>,
    checked: bool,
    action: Action,
}
impl Row {
    fn command(
        group: &'static str,
        title: impl Into<String>,
        icon: &'static str,
        hint: &'static str,
        action: Action,
    ) -> Self {
        Self {
            group,
            title: title.into(),
            meta: String::new(),
            hint,
            icon,
            ticket: None,
            status: None,
            priority: None,
            project: None,
            checked: false,
            action,
        }
    }
    fn ticket(group: &'static str, t: SearchTicket) -> Self {
        Self {
            group,
            title: t.title.clone(),
            meta: t.key.clone(),
            hint: "↵",
            icon: "ticket",
            status: Some(t.status),
            priority: None,
            project: None,
            checked: false,
            action: Action::Open(t.clone()),
            ticket: Some(t),
        }
    }
}
fn fixture_tickets(state: DesignState) -> Vec<SearchTicket> {
    state
        .tickets
        .get()
        .into_iter()
        .map(|t| SearchTicket {
            id: i64::from(t.id),
            project_id: 1,
            key: format!("TKT-{}", t.id),
            title: t.title,
            status: t.status,
            priority: t.priority,
            assignee: Some("sample-user".into()),
            updated_at: "2026-10-29T12:00:00.000Z".into(),
            external_url: Some(format!(
                "https://github.com/example/tickets/issues/{}",
                t.id
            )),
        })
        .collect()
}
fn fixture_search(state: DesignState, q: &SearchQuery, recent: &[i64]) -> Vec<SearchTicket> {
    let bodies = state.tickets.get();
    let mut ts = fixture_tickets(state)
        .into_iter()
        .filter(|t| {
            q.status.is_none_or(|s| t.status == s)
                && q.project.as_deref().is_none_or(|p| p == "TKT")
                && (t.key.eq_ignore_ascii_case(&q.text)
                    || fuzzy_match(
                        &format!(
                            "{} {}",
                            t.title,
                            bodies
                                .iter()
                                .find(|b| i64::from(b.id) == t.id)
                                .map(|b| b.description.as_str())
                                .unwrap_or("")
                        ),
                        &q.text,
                    ))
                && (!q.text.is_empty()
                    || q.filtered()
                    || recent.contains(&t.id)
                    || !matches!(t.status, Status::Done | Status::Canceled))
        })
        .collect::<Vec<_>>();
    ts.sort_by_key(|t| {
        (
            !t.key.eq_ignore_ascii_case(&q.text),
            !t.title.to_lowercase().contains(&q.text),
            !recent.contains(&t.id),
            t.id,
        )
    });
    ts.truncate(30);
    ts
}

#[component]
pub fn CommandPalette() -> impl IntoView {
    let design = expect_context::<DesignState>();
    let board = expect_context::<BoardState>();
    let location = use_location();
    let fixture = location.pathname.get_untracked().starts_with("/design");
    if fixture && board.projects.get_untracked().is_empty() {
        super::shell::fixture_account(expect_context::<crate::pages::settings::AccountState>());
    }
    let navigate = use_navigate();
    let query = RwSignal::new(String::new());
    let mode = RwSignal::new(Mode::Search);
    let target = RwSignal::new(None::<SearchTicket>);
    let results = RwSignal::new(Vec::<SearchTicket>::new());
    let active = RwSignal::new(0usize);
    let loading = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let retry = RwSignal::new(0u64);
    let saved = StoredValue::new((String::new(), 0usize));
    let input = NodeRef::<html::Input>::new();
    let recent = StoredValue::new(recent_ids(fixture));
    Effect::new(move |_| {
        if !busy.get()
            && let Some(input) = input.get()
        {
            let _ = input.focus();
        }
    });
    let context = StoredValue::new({
        let path = location.pathname.get_untracked();
        let focused = document()
            .active_element()
            .and_then(|el| el.closest(".api-ticket, .api-list-row").ok().flatten())
            .and_then(|el| {
                el.get_attribute("href").or_else(|| {
                    el.query_selector("a[href^='/t/']")
                        .ok()
                        .flatten()
                        .and_then(|a| a.get_attribute("href"))
                })
            });
        let route = focused.unwrap_or(path);
        let ts = if fixture {
            fixture_tickets(design)
        } else {
            board.tickets.get_untracked().iter().map(compact).collect()
        };
        ts.into_iter().find(|t| {
            route == format!("/t/{}", t.key)
                || (route == "/ticket"
                    && location.search.get_untracked() == format!("?id={}", t.id))
                || (fixture
                    && route == "/design/ticket"
                    && t.id == i64::from(design.selected_ticket.get_untracked()))
        })
    });
    Effect::new(move |_| {
        let text = query.get();
        let current = mode.get();
        retry.track();
        if current != Mode::Search {
            return;
        }
        results.set(vec![]);
        error.set(String::new());
        active.set(0);
        let parsed = match SearchQuery::parse(&text) {
            Ok(q) => q,
            Err(e) => {
                error.set(e);
                loading.set(false);
                return;
            }
        };
        if fixture {
            results.set(fixture_search(design, &parsed, &recent.get_value()));
            return;
        }
        loading.set(true);
        let (abort, registration) = futures_util::future::AbortHandle::new_pair();
        on_cleanup(move || abort.abort());
        spawn_local(async move {
            let _ = futures_util::future::Abortable::new(
                async move {
                    if !text.is_empty() {
                        gloo_timers::future::TimeoutFuture::new(120).await;
                    }
                    let ids = recent
                        .get_value()
                        .iter()
                        .map(i64::to_string)
                        .collect::<Vec<_>>()
                        .join(",");
                    let path = format!(
                        "/api/v1/search?q={}&recent={ids}",
                        js_sys::encode_uri_component(&text)
                    );
                    match api("GET", &path, Value::Null).await.and_then(|v| {
                        serde_json::from_value(v["data"].clone())
                            .map_err(|_| "Could not read search results. Try again.".into())
                    }) {
                        Ok(ts) => {
                            results.set(ts);
                            active.set(0);
                        }
                        Err(e) => error.set(e),
                    }
                    loading.set(false);
                },
                registration,
            )
            .await;
        });
    });
    let rows = Signal::derive(move || {
        let text = query.get();
        let mut rows = vec![];
        let projects = board.projects.get();
        let root = if fixture { "/design" } else { "" };
        match mode.get() {
            Mode::Search => {
                let empty = text.trim().is_empty();
                let tickets = results.get();
                if empty {
                    for id in recent.get_value() {
                        if let Some(t) = tickets.iter().find(|t| t.id == id) {
                            rows.push(Row::ticket("Recently viewed", t.clone()));
                        }
                    }
                    rows.extend(
                        tickets
                            .into_iter()
                            .filter(|t| !recent.get_value().contains(&t.id))
                            .take(6)
                            .map(|t| Row::ticket("In progress & assigned", t)),
                    );
                } else {
                    rows.extend(tickets.into_iter().map(|t| Row::ticket("Tickets", t)));
                }
                if let Some(t) = context.get_value() {
                    let mut row = Row::command(
                        "Current ticket",
                        format!("Actions for {}", t.key),
                        "ticket",
                        "→",
                        Action::Actions(t.clone()),
                    );
                    row.meta = t.title;
                    rows.push(row);
                }
                let project_name = projects
                    .iter()
                    .find(|p| p.id == board.project.get())
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| {
                        if fixture {
                            design.current_project.get()
                        } else {
                            String::new()
                        }
                    });
                let mut create =
                    Row::command("Commands", "New ticket", "plus", "C", Action::Create);
                create.meta = project_name;
                rows.extend([
                    create,
                    Row::command(
                        "Commands",
                        "Go to board",
                        "board",
                        "G B",
                        Action::Navigate(if fixture {
                            "/design/board".into()
                        } else {
                            format!("/board?project={}", board.project.get())
                        }),
                    ),
                    Row::command(
                        "Commands",
                        "Go to list",
                        "list",
                        "G L",
                        Action::Navigate(format!("{root}/list?project={}", board.project.get())),
                    ),
                    Row::command(
                        "Commands",
                        "Go to projects",
                        "project",
                        "",
                        Action::Navigate(format!("{root}/projects")),
                    ),
                    Row::command(
                        "Commands",
                        "Settings",
                        "settings",
                        "",
                        Action::Navigate(format!("{root}/settings/account")),
                    ),
                    Row::command("Commands", "Toggle theme", "appearance", "", Action::Theme),
                ]);
                for p in projects.into_iter().filter(|p| !p.archived) {
                    let mut row = Row::command(
                        "Projects",
                        p.name.clone(),
                        "project",
                        "",
                        Action::Project(p.clone()),
                    );
                    row.meta = p.key_prefix.clone();
                    row.project = Some(p);
                    rows.push(row);
                }
                rows.retain(|r| {
                    r.ticket.is_some()
                        || (SearchQuery::parse(&text).is_ok_and(|q| !q.filtered())
                            && fuzzy_match(&format!("{} {}", r.title, r.meta), &text))
                });
            }
            Mode::Actions => {
                if let Some(t) = target.get() {
                    rows.extend([
                        Row::command(
                            "Ticket actions",
                            "Open ticket",
                            "ticket",
                            "↵",
                            Action::Open(t.clone()),
                        ),
                        Row::command(
                            "Ticket actions",
                            "Change status…",
                            "list",
                            "→",
                            Action::Mode(Mode::Status),
                        ),
                        Row::command(
                            "Ticket actions",
                            "Change priority…",
                            "bolt",
                            "→",
                            Action::Mode(Mode::Priority),
                        ),
                        Row::command(
                            "Ticket actions",
                            "Move to project…",
                            "project",
                            "→",
                            Action::Mode(Mode::Project),
                        ),
                        Row::command("Copy", "Copy link", "link", "", Action::Copy(true)),
                        Row::command("Copy", "Copy key", "code", "", Action::Copy(false)),
                    ]);
                    if let Some(url) = t
                        .external_url
                        .filter(|u| u.starts_with("https://github.com/"))
                    {
                        rows.push(Row::command(
                            "Links",
                            "Open in GitHub",
                            "external",
                            "",
                            Action::GitHub(url),
                        ));
                    }
                }
            }
            Mode::Status => {
                for s in Status::ALL {
                    let mut row =
                        Row::command("Change status", s.name(), "ticket", "", Action::Status(s));
                    row.status = Some(s);
                    row.checked = target.get().is_some_and(|t| t.status == s);
                    rows.push(row);
                }
            }
            Mode::Priority => {
                for p in [
                    Priority::None,
                    Priority::Urgent,
                    Priority::High,
                    Priority::Medium,
                    Priority::Low,
                ] {
                    let mut row =
                        Row::command("Change priority", p.name(), "bolt", "", Action::Priority(p));
                    row.priority = Some(p);
                    row.checked = target.get().is_some_and(|t| t.priority == p);
                    rows.push(row);
                }
            }
            Mode::Project => {
                for p in projects.into_iter().filter(|p| !p.archived) {
                    let mut row = Row::command(
                        "Move to project",
                        p.name.clone(),
                        "project",
                        "",
                        Action::Move(p.id),
                    );
                    row.meta = p.key_prefix.clone();
                    row.checked = target.get().is_some_and(|t| t.project_id == p.id);
                    row.project = Some(p);
                    rows.push(row);
                }
            }
        }
        if mode.get() != Mode::Search {
            rows.retain(|r| fuzzy_match(&r.title, &text));
        }
        rows
    });
    let back = Callback::new(move |_| {
        if busy.get_untracked() {
            return;
        }
        error.set(String::new());
        if matches!(
            mode.get_untracked(),
            Mode::Status | Mode::Priority | Mode::Project
        ) {
            mode.set(Mode::Actions);
            query.set(String::new());
            active.set(0);
        } else {
            mode.set(Mode::Search);
            let (q, index) = saved.get_value();
            query.set(q);
            active.set(index);
        }
        if let Some(input) = input.get_untracked() {
            let _ = input.focus();
        }
    });
    let start_actions = Callback::new(move |t: SearchTicket| {
        saved.set_value((query.get_untracked(), active.get_untracked()));
        target.set(Some(t));
        mode.set(Mode::Actions);
        query.set(String::new());
        active.set(0);
        error.set(String::new());
        if let Some(input) = input.get_untracked() {
            let _ = input.focus();
        }
    });
    let close = Callback::new(move |_| {
        if !busy.get_untracked() {
            design.dialog.set(DialogKind::None);
        }
    });
    let execute = Callback::new(move |action: Action| {
        if busy.get_untracked() {
            return;
        }
        match action {
            Action::Actions(t) => start_actions.run(t),
            Action::Mode(next) => {
                mode.set(next);
                query.set(String::new());
                active.set(0);
                error.set(String::new());
            }
            Action::Open(t) => {
                if !fixture {
                    remember(t.id);
                }
                close.run(());
                navigate(
                    &if fixture {
                        format!("/design/ticket?id={}", t.id)
                    } else {
                        format!("/t/{}", t.key)
                    },
                    Default::default(),
                );
            }
            Action::Navigate(path) => {
                close.run(());
                navigate(&path, Default::default());
            }
            Action::Project(p) => {
                if fixture {
                    design.current_project.set(p.name);
                } else {
                    board.select_project(p.id);
                }
                close.run(());
                navigate(
                    if fixture { "/design/board" } else { "/board" },
                    Default::default(),
                );
            }
            Action::Create => {
                close.run(());
                if fixture {
                    set_timeout(
                        move || design.new_ticket(Status::Todo),
                        std::time::Duration::from_millis(EXIT_MS),
                    );
                } else if board.project.get_untracked() > 0 {
                    let path = location.pathname.get_untracked();
                    if ["/", "/home"].contains(&path.as_str()) {
                        board.refresh();
                    }
                    if !["/", "/home", "/board", "/list", "/ticket"].contains(&path.as_str())
                        && !path.starts_with("/t/")
                    {
                        navigate("/board", Default::default());
                    }
                    set_timeout(
                        move || board.composer.set(Some(Status::Todo)),
                        std::time::Duration::from_millis(EXIT_MS),
                    );
                } else {
                    navigate("/projects", Default::default());
                    design
                        .toast
                        .set("Create a project to add its first ticket.".into());
                }
            }
            Action::Theme => {
                let light = document()
                    .document_element()
                    .and_then(|e| e.get_attribute("data-theme"))
                    .as_deref()
                    == Some("light");
                persist("tickets.theme", if light { "dark" } else { "light" });
                apply_appearance();
                close.run(());
                design
                    .toast
                    .set(if light { "Dark theme" } else { "Light theme" }.into());
            }
            Action::GitHub(url) => {
                let _ = window().open_with_url_and_target_and_features(
                    &url,
                    "_blank",
                    "noopener,noreferrer",
                );
                close.run(());
            }
            Action::Copy(link) => {
                if let Some(t) = target.get_untracked() {
                    let text = if link {
                        format!(
                            "{}/t/{}",
                            window().location().origin().unwrap_or_default(),
                            t.key
                        )
                    } else {
                        t.key
                    };
                    busy.set(true);
                    spawn_local(async move {
                        let result =
                            JsFuture::from(window().navigator().clipboard().write_text(&text))
                                .await;
                        if busy.is_disposed() {
                            return;
                        }
                        busy.set(false);
                        match result {
                            Ok(_) => {
                                design
                                    .toast
                                    .set(if link { "Link copied" } else { "Key copied" }.into());
                                close.run(());
                            }
                            Err(_) => error.set(
                                "Could not copy. Allow clipboard access and try again.".into(),
                            ),
                        }
                    });
                }
            }
            change => {
                let Some(old) = target.get_untracked() else {
                    return;
                };
                let mut optimistic = old.clone();
                let patch = match change {
                    Action::Status(s) => {
                        optimistic.status = s;
                        json!({"status":s})
                    }
                    Action::Priority(p) => {
                        optimistic.priority = p;
                        json!({"priority":p})
                    }
                    Action::Move(id) => json!({"project_id":id}),
                    _ => return,
                };
                if fixture {
                    design.tickets.update(|ts| {
                        if let Some(t) = ts.iter_mut().find(|t| i64::from(t.id) == old.id) {
                            t.status = optimistic.status;
                            t.priority = optimistic.priority;
                            if let Some(id) = patch["project_id"].as_i64()
                                && let Some(p) = board
                                    .projects
                                    .get_untracked()
                                    .into_iter()
                                    .find(|p| p.id == id)
                            {
                                t.project = p.name;
                            }
                        }
                    });
                    design
                        .toast
                        .set(format!("{} updated · Sample data", old.key));
                    close.run(());
                    return;
                }
                target.set(Some(optimistic));
                busy.set(true);
                error.set(String::new());
                let navigate = navigate.clone();
                spawn_local(async move {
                    let result = api("PATCH", &format!("/api/v1/tickets/{}", old.id), patch)
                        .await
                        .and_then(|v| {
                            serde_json::from_value::<BoardTicket>(v).map_err(|_| {
                                "Could not read saved ticket. Reopen to check its state.".into()
                            })
                        });
                    if busy.is_disposed() {
                        return;
                    }
                    busy.set(false);
                    match result {
                        Ok(t) => {
                            board.apply_change_event(t.clone(), false);
                            design.toast.set(format!("{} updated", t.key));
                            close.run(());
                            if old.key != t.key
                                && location.pathname.get_untracked() == format!("/t/{}", old.key)
                            {
                                navigate(&format!("/t/{}", t.key), Default::default());
                            }
                            set_timeout(
                                move || board.catchup_revision.update(|r| *r += 1),
                                std::time::Duration::from_millis(EXIT_MS),
                            );
                        }
                        Err(e) => {
                            target.set(Some(old));
                            error.set(format!(
                                "Could not save. {e} Choose the value again to retry."
                            ));
                        }
                    }
                });
            }
        }
    });
    let actions = Callback::new(move |_| {
        if let Some(t) = rows
            .get_untracked()
            .get(active.get_untracked())
            .and_then(|r| r.ticket.clone())
        {
            start_actions.run(t);
        }
    });
    let on_key = move |event: web_sys::KeyboardEvent| {
        if event.is_composing() {
            return;
        }
        let key = event.key();
        if busy.get_untracked() {
            if ["Escape", "Enter", "k", "K"].contains(&key.as_str()) {
                event.prevent_default();
                event.stop_propagation();
            }
            return;
        }
        if key == "Escape" && mode.get_untracked() != Mode::Search {
            event.prevent_default();
            event.stop_propagation();
            back.run(());
            return;
        }
        if key == "Backspace"
            && query.get_untracked().is_empty()
            && mode.get_untracked() != Mode::Search
        {
            event.prevent_default();
            back.run(());
            return;
        }
        if key == "ArrowRight" && query.get_untracked().is_empty() {
            if let Some(row) = rows.get_untracked().get(active.get_untracked()) {
                if row.ticket.is_some() {
                    event.prevent_default();
                    actions.run(());
                } else if matches!(row.action, Action::Mode(_) | Action::Actions(_)) {
                    event.prevent_default();
                    execute.run(row.action.clone());
                }
            }
            return;
        }
        if key == "Enter" && (event.ctrl_key() || event.meta_key()) {
            event.prevent_default();
            actions.run(());
            return;
        }
        let len = rows.get_untracked().len();
        if len == 0 {
            return;
        }
        if ["ArrowDown", "ArrowUp", "Home", "End"].contains(&key.as_str())
            && (key.starts_with("Arrow") || event.ctrl_key() || event.meta_key())
        {
            event.prevent_default();
            active.update(|i| {
                *i = match key.as_str() {
                    "ArrowUp" => (*i + len - 1) % len,
                    "Home" => 0,
                    "End" => len - 1,
                    _ => (*i + 1) % len,
                }
            });
            if let Some(input) = input.get_untracked() {
                let _ = input.focus();
            }
        } else if key == "Enter"
            && event
                .target()
                .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                .is_some_and(|e| e.tag_name() == "INPUT")
        {
            event.prevent_default();
            if let Some(row) = rows.get_untracked().get(active.get_untracked()) {
                execute.run(row.action.clone());
            }
        }
    };
    Effect::new(move |_| {
        let i = active.get();
        if let Some(el) = document().get_element_by_id(&format!("command-option-{i}")) {
            let parent = el.closest(".command-results").ok().flatten();
            if let Some(parent) = parent {
                let r = el.get_bounding_client_rect();
                let p = parent.get_bounding_client_rect();
                if r.bottom() > p.bottom() {
                    parent.set_scroll_top(parent.scroll_top() + (r.bottom() - p.bottom()) as i32);
                } else if r.top() < p.top() {
                    parent.set_scroll_top(parent.scroll_top() - (p.top() - r.top()) as i32);
                }
            }
        }
    });
    let search_label = Signal::derive(move || match mode.get() {
        Mode::Search => "Search tickets or commands",
        Mode::Actions => "Search ticket actions",
        Mode::Status => "Choose a status",
        Mode::Priority => "Choose a priority",
        Mode::Project => "Choose a project",
    });
    let action_hint = if window()
        .navigator()
        .platform()
        .unwrap_or_default()
        .contains("Mac")
    {
        "⌘ ↵"
    } else {
        "Ctrl ↵"
    };
    let enter_label = Signal::derive(move || {
        rows.get()
            .get(active.get())
            .map(|r| match &r.action {
                Action::Open(_) | Action::Navigate(_) | Action::Project(_) | Action::GitHub(_) => {
                    " Open"
                }
                Action::Actions(_) | Action::Mode(_) => " Choose",
                Action::Status(_) | Action::Priority(_) | Action::Move(_) => " Set",
                Action::Copy(_) => " Copy",
                Action::Create => " Create",
                Action::Theme => " Toggle",
            })
            .unwrap_or("")
    });
    view! {
        <Modal title="Search and commands" open=Signal::derive(move||design.dialog.get()==DialogKind::Command) locked=busy.into() on_close=close>
            <div class="command-palette" aria-busy=move||busy.get().to_string() on:keydown=on_key>
                <Show when=move||mode.get()!=Mode::Search><div class="command-context" id="command-context"><button class="icon-button" aria-label="Back" disabled=busy on:click=move|_|back.run(())><span class="command-back"><Icon name="arrow"/></span></button><span class="mono">{move||target.get().map(|t|t.key)}</span><span class="menu-title">{move||target.get().map(|t|t.title)}</span></div></Show>
                <div class="command-search"><Icon name="search"/><input node_ref=input role="combobox" aria-label=search_label aria-autocomplete="list" aria-haspopup="grid" aria-expanded="true" aria-controls="command-results" aria-describedby=move||if mode.get()==Mode::Search{"command-help"}else{"command-context"} aria-activedescendant=move||(!rows.get().is_empty()).then(||format!("command-option-{}",active.get())) autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other" maxlength="200" placeholder=move||match mode.get(){Mode::Search=>"Search tickets or commands…",Mode::Actions=>"Search actions…",Mode::Status=>"Change status…",Mode::Priority=>"Change priority…",Mode::Project=>"Move to project…"} prop:value=query readonly=busy on:input=move|ev|{query.set(event_target_value(&ev));active.set(0);}/><Show when=move||loading.get()||busy.get()><AsciiSpinner label="Loading"/></Show></div>
                <Show when=move||mode.get()==Mode::Search><div class="command-help" id="command-help"><span>"Filter with "<code>"status:todo"</code>" or "<code>"p:QUA"</code></span><span>{move||SearchQuery::parse(&query.get()).ok().and_then(|q|q.project).unwrap_or_else(||"All projects".into())}</span></div></Show>
                <Show when=move||!error.get().is_empty()><div class="command-error" role="alert"><span>{error}</span><Show when=move||mode.get()==Mode::Search && SearchQuery::parse(&query.get()).is_ok()><button class="button button-ghost" on:click=move|_|retry.update(|r|*r+=1)>"Retry"</button></Show></div></Show>
                <div id="command-results" class="command-results" role="grid" tabindex="-1" aria-colcount="2" aria-label=search_label aria-busy=move||loading.get().to_string()>
                    {move||{
                        let mut previous="";
                        rows.get().into_iter().enumerate().map(|(index,row)|{
                            let heading=(row.group!=previous).then_some(row.group);previous=row.group;
                            let mut name=if let Some(t)=&row.ticket{format!("{} {}, {}",row.meta,row.title,t.status.name())}else{row.title.clone()};
                            if row.checked{name.push_str(", current value");}
                            view!{ {heading.map(|heading|view!{<div class="menu-heading" role="presentation">{heading}</div>})}
                                <div class="command-result" role="row" on:pointermove=move|e|{if e.pointer_type()=="mouse"{active.set(index);}}>
                                <div id=format!("command-option-{index}") class="command-option" role="gridcell" aria-label=name.clone() aria-selected=move||(active.get()==index).to_string()>
                                <button class="menu-item" aria-label=name.clone() tabindex="-1" disabled=busy title=row.title.clone() on:click=move|_|execute.run(row.action.clone())>
                                    {if let Some(s)=row.status{view!{<span title=s.name()><StatusIcon status=s/></span>}.into_any()}else if let Some(p)=row.priority{view!{<PriorityIcon priority=p/>}.into_any()}else if let Some(p)=row.project{view!{<ProjectIcon name=p.name icon=p.icon.unwrap_or_default() color=p.color/>}.into_any()}else{view!{<Icon name=row.icon/>}.into_any()}}
                                    {row.ticket.is_some().then(||view!{<span class="mono command-key">{row.meta.clone()}</span>})}<span class="menu-title">{row.title.clone()}</span>
                                    {row.ticket.is_none().then(||view!{<span class="command-meta">{row.meta}</span>})}
                                    {row.checked.then(||view!{<Icon name="check"/>})}{(!row.hint.is_empty()).then(||view!{<span class="command-hint"><Shortcut>{row.hint}</Shortcut></span>})}
                                </button></div>
                                <div class="command-action-cell" role="gridcell">{row.ticket.map(|t|view!{<button class="command-row-actions" aria-label=format!("Actions for {}",t.key) title="Ticket actions" tabindex=move||if active.get()==index{"0"}else{"-1"} on:focus=move|_|active.set(index) on:click=move|_|start_actions.run(t.clone())><Icon name="more"/></button>})}</div>
                                </div>
                            }
                        }).collect_view()
                    }}
                    <Show when=move||rows.get().is_empty() && !loading.get() && error.get().is_empty()><div class="command-empty" role="status"><strong>"No results"</strong><p>{move||if mode.get()==Mode::Search{"Try a ticket key, fewer words, or a different filter."}else{"Clear your search to see all choices."}}</p></div></Show>
                    <Show when=move||loading.get() && rows.get().is_empty()><p class="command-empty" role="status">"Searching tickets…"</p></Show>
                </div>
                <div class="command-footer"><span><Shortcut>"↑ ↓"</Shortcut>" Navigate"</span><Show when=move||!rows.get().is_empty()><span><Shortcut>"↵"</Shortcut>{enter_label}</span></Show><span><Shortcut>"esc"</Shortcut>{move||if mode.get()==Mode::Search{" Close"}else{" Back"}}</span><Show when=move||mode.get()==Mode::Search && rows.get().get(active.get()).is_some_and(|r|r.ticket.is_some())><button class="command-actions" on:click=move|_|actions.run(())>"Actions"<Shortcut>{action_hint}</Shortcut></button></Show></div>
            </div>
        </Modal>
    }
}
