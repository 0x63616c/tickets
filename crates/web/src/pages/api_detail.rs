//! Persistent ticket editor using the fixture's shared detail structure and controls.
use super::board::{BoardComposer, BoardState, BoardTicket, all};
use crate::{
    components::*,
    fixtures::{Priority, Status},
    ui::api,
};
use leptos::{ev, prelude::*};
use leptos_router::hooks::{use_params_map, use_query_map};
use serde::Deserialize;
use serde_json::{Value, json};
use web_sys::wasm_bindgen::JsCast;

#[derive(Clone, Deserialize)]
struct CommentRecord {
    id: i64,
    body: String,
    author: String,
    created_at: String,
    #[serde(default)]
    author_kind: String,
    #[serde(default)]
    edited_at: Option<String>,
    #[serde(default)]
    deleted: bool,
    #[serde(default)]
    can_edit: bool,
    #[serde(skip)]
    pending: bool,
    #[serde(skip)]
    reveal: bool,
}
#[derive(Clone, Deserialize)]
struct HistoryPage {
    data: Vec<tickets_core::history::Entry>,
    next_cursor: Option<String>,
}
#[derive(Clone)]
enum Activity {
    Change(tickets_core::history::Entry),
    Comment(CommentRecord),
}
impl Activity {
    fn time(&self) -> &str {
        match self {
            Self::Change(e) => &e.recorded_at,
            Self::Comment(c) => &c.created_at,
        }
    }
    fn id(&self) -> i64 {
        match self {
            Self::Change(e) => e.id,
            Self::Comment(c) => c.id,
        }
    }
    fn key(&self) -> String {
        match self {
            Self::Change(e) => format!("history-{}", e.id),
            Self::Comment(c) => format!(
                "comment-{}-{}-{}-{:?}-{}",
                c.id, c.body, c.deleted, c.edited_at, c.can_edit
            ),
        }
    }
}
#[derive(Clone, Copy)]
struct DetailState {
    owner: StoredValue<leptos::reactive::owner::WeakOwner>,
    board: BoardState,
    id: RwSignal<i64>,
    selector: StoredValue<String>,
    ticket: RwSignal<Option<BoardTicket>>,
    comments: RwSignal<Vec<CommentRecord>>,
    history: RwSignal<Vec<tickets_core::history::Entry>>,
    history_cursor: RwSignal<Option<String>>,
    history_busy: RwSignal<bool>,
    history_error: RwSignal<String>,
    history_failed_older: RwSignal<bool>,
    busy: RwSignal<bool>,
    loading: RwSignal<bool>,
    error: RwSignal<String>,
    comment_error: RwSignal<String>,
    revision: RwSignal<u64>,
}
impl DetailState {
    async fn refresh_history(self, ticket_id: i64) -> Result<HistoryPage, String> {
        let newest = self
            .history
            .with_untracked(|entries| entries.first().map(|e| e.id));
        let mut entries = vec![];
        let mut cursor = None;
        loop {
            let suffix = cursor
                .as_ref()
                .map(|c| format!("?cursor={c}"))
                .unwrap_or_default();
            let page: HistoryPage = serde_json::from_value(
                api(
                    "GET",
                    &format!("/api/v1/tickets/{ticket_id}/history{suffix}"),
                    Value::Null,
                )
                .await?,
            )
            .map_err(|e| e.to_string())?;
            let overlaps = newest.is_some_and(|id| page.data.iter().any(|e| e.id <= id));
            entries.extend(page.data);
            cursor = page.next_cursor;
            // Fetch through bursts larger than one page before merging, so the
            // retained history window never has a silent gap in the middle.
            if newest.is_none() || overlaps || cursor.is_none() {
                return Ok(HistoryPage {
                    data: entries,
                    next_cursor: cursor,
                });
            }
        }
    }
    /// Detail reads and writes must stop when this ticket's keyed page unmounts.
    /// Use the page owner even from effects or short-lived editor children.
    fn spawn(self, task: impl std::future::Future<Output = ()> + 'static) {
        self.owner.with_value(|owner| {
            // A disposed page has nothing left to load or save into.
            let Some(owner) = owner.upgrade() else {
                return;
            };
            let (abort, registration) = futures_util::future::AbortHandle::new_pair();
            owner.with(|| on_cleanup(move || abort.abort()));
            // Do not retain the page Owner in the future: that would keep the
            // keyed page alive and prevent its cleanup from canceling this task.
            wasm_bindgen_futures::spawn_local(async move {
                let _ = futures_util::future::Abortable::new(task, registration).await;
            });
        });
    }
    async fn load(self, background: bool) -> Result<(), String> {
        let revision = self.revision.get_untracked();
        let ticket: BoardTicket = serde_json::from_value(
            api(
                "GET",
                &format!(
                    "/api/v1/tickets/{}",
                    if self.id.get_untracked() > 0 {
                        self.id.get_untracked().to_string()
                    } else {
                        self.selector.get_value()
                    }
                ),
                Value::Null,
            )
            .await?,
        )
        .map_err(|e| e.to_string())?;
        self.id.set(ticket.id);
        let mut comments =
            all::<CommentRecord>(&format!("tickets/{}/comments", self.id.get_untracked())).await?;
        let history = self.refresh_history(ticket.id).await;
        if background
            && (crate::live::editing()
                || self.busy.get_untracked()
                || revision != self.revision.get_untracked())
        {
            self.board.live_revision.update(|r| *r += 1);
            return Ok(());
        }
        let restore_label_focus = document().active_element().is_some_and(|element| {
            element
                .matches(".label-menu .menu-trigger")
                .unwrap_or(false)
        });
        self.board.project.set(ticket.project_id);
        self.board.apply_change_event(ticket.clone(), false);
        let reveal_comments = self.ticket.get_untracked().is_some();
        self.ticket.set(Some(ticket));
        // Capture this before the async write clears `busy` and Leptos mounts
        // keyed comments. Initial comments share the page's mount reveal.
        for comment in &mut comments {
            comment.reveal = reveal_comments;
        }
        self.comments.set(comments);
        match history {
            Ok(page) => {
                let first_load = self.history.with_untracked(Vec::is_empty);
                self.history.update(|entries| {
                    entries.extend(page.data);
                    entries.sort_by_key(|e| std::cmp::Reverse(e.id));
                    entries.dedup_by_key(|e| e.id);
                });
                if first_load {
                    self.history_cursor.set(page.next_cursor);
                }
                self.history_error.set(String::new());
            }
            Err(error) => {
                self.history_failed_older.set(false);
                self.history_error
                    .set(format!("Could not load activity: {error}"));
            }
        }
        self.revision.update(|r| *r += 1);
        if restore_label_focus {
            gloo_timers::future::TimeoutFuture::new(0).await;
            focus_label_trigger();
        }
        Ok(())
    }
    fn write(self, method: &'static str, path: String, body: Value, done: impl FnOnce() + 'static) {
        if self.busy.get_untracked() {
            return;
        }
        self.busy.set(true);
        let comment_write = path.contains("/comments/");
        if comment_write {
            self.comment_error.set(String::new());
        } else {
            self.error.set(String::new());
        }
        self.spawn(async move {
            match api(method, &path, body).await {
                Ok(_) => {
                    done();
                    if let Err(e) = self.load(false).await {
                        self.error.set(format!("Saved, but could not reload: {e}"));
                    }
                    self.board.notice.set("Saved.".into());
                    if comment_write {
                        let selector = if method == "PATCH" {
                            format!(
                                "[data-comment-id='{}'] .comment-action-edit",
                                path.rsplit('/').next().unwrap_or("")
                            )
                        } else {
                            ".activity > .comment-form textarea".into()
                        };
                        focus_thread(selector);
                    }
                }
                Err(e) => {
                    if comment_write {
                        self.comment_error.set(e);
                    } else {
                        self.error.set(e);
                    }
                    self.revision.update(|r| *r += 1);
                }
            }
            self.busy.set(false);
        });
    }
    fn patch(self, body: Value) {
        self.write(
            "PATCH",
            format!("/api/v1/tickets/{}", self.id.get_untracked()),
            body,
            || {},
        );
    }
}
#[component]
pub fn ApiDetail() -> impl IntoView {
    let query = use_query_map();
    let params = use_params_map();
    view! {<For each=move||{params.get().get("key").map(|key|key.to_ascii_uppercase()).or_else(||query.get().get("id")).into_iter().collect::<Vec<_>>()} key=|selector|selector.clone() children=|selector|view!{<DetailPage selector/>}/>}
}
#[component]
fn DetailPage(selector: String) -> impl IntoView {
    let board = expect_context::<BoardState>();
    let state = DetailState {
        owner: StoredValue::new(Owner::current().expect("detail page owner").downgrade()),
        board,
        id: RwSignal::new(0),
        selector: StoredValue::new(selector),
        ticket: RwSignal::new(None),
        comments: RwSignal::new(vec![]),
        history: RwSignal::new(vec![]),
        history_cursor: RwSignal::new(None),
        history_busy: RwSignal::new(false),
        history_error: RwSignal::new(String::new()),
        history_failed_older: RwSignal::new(false),
        busy: RwSignal::new(false),
        loading: RwSignal::new(true),
        error: RwSignal::new(String::new()),
        comment_error: RwSignal::new(String::new()),
        revision: RwSignal::new(0),
    };
    provide_context(state);
    board.refresh();
    state.spawn(async move {
        if let Err(e) = state.load(false).await {
            state.error.set(e);
        }
        state.loading.set(false);
    });
    let properties_expanded = RwSignal::new(false);
    let edit = RwSignal::new(false);
    let pending_live = RwSignal::new(false);
    let observed_live = StoredValue::new(board.catchup_revision.get_untracked());
    Effect::new(move |_| {
        let revision = board.catchup_revision.get();
        if revision != observed_live.get_value() {
            observed_live.set_value(revision);
            pending_live.set(true);
        }
    });
    Effect::new(move |_| {
        if pending_live.get()
            && !state.busy.get()
            && !state.loading.get()
            && !edit.get()
            && !crate::live::editing()
        {
            pending_live.set(false);
            state.loading.set(true);
            state.spawn(async move {
                if let Err(error) = state.load(true).await {
                    state.error.set(error);
                }
                state.loading.set(false);
            });
        }
    });
    let dependency = RwSignal::new(false);
    let handle = window_event_listener(ev::keydown, move |e| {
        let typing = e
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .is_some_and(|t| {
                ["INPUT", "TEXTAREA", "SELECT"].contains(&t.tag_name().as_str())
                    || t.has_attribute("contenteditable")
            });
        if !typing
            && !e.ctrl_key()
            && !e.meta_key()
            && !e.alt_key()
            && e.key() == "c"
            && !state.busy.get_untracked()
            && !board.loading.get_untracked()
            && document().query_selector("dialog").ok().flatten().is_none()
        {
            e.prevent_default();
            board.composer.set(Some(Status::Todo));
        }
    });
    on_cleanup(move || handle.remove());
    let reveal_root = NodeRef::<leptos::html::Div>::new();
    // This root mounts only after the first load. Live reconciliation keeps it
    // mounted; reveal changed keyed comments without replaying the ticket.
    use_ascii_reveal(reveal_root, Signal::derive(|| false), true);
    view! {
        <header class="topbar"><div class="breadcrumbs"><a href="/projects"><Icon name="project"/>"Projects"</a><span class="breadcrumb-slash">"/"</span><a href="/board">{move||state.ticket.get().and_then(|t|board.projects.get().into_iter().find(|p|p.id==t.project_id)).map(|p|view!{<ProjectIcon name=p.name.clone() icon=p.icon.unwrap_or_default() color=p.color/>{p.name}})}</a><span class="breadcrumb-slash">"/"</span><span class="mono">{move||state.ticket.get().map(|t|t.key).unwrap_or_default()}</span></div><div class="topbar-actions"><crate::live::LiveIndicator/><span class="toolbar-divider"/><button class="button button-secondary" disabled=move||state.busy.get() || state.ticket.get().is_none() on:click=move|_|edit.set(true)>"Edit ticket"</button><button class="button button-primary" disabled=move||board.loading.get() || board.project.get()==0 on:click=move|_|board.composer.set(Some(Status::Todo))><Icon name="plus"/>"New ticket"<Shortcut>"C"</Shortcut></button></div></header>
        <Show when=move||state.loading.get() && state.ticket.get().is_none()><LoadingSkeleton layout="detail"/></Show>
        <Show when=move||!state.error.get().is_empty() && !edit.get() && !dependency.get()><div class="board-message" role="alert">{move||state.error.get()}<button class="button" disabled=move||state.busy.get() on:click=move|_|{state.busy.set(true);state.spawn(async move{state.error.set(String::new());if let Err(e)=state.load(false).await{state.error.set(e);}state.busy.set(false);});}>"Reload ticket"</button></div></Show>
        <Show when=move||state.ticket.get().is_some()><div node_ref=reveal_root class="detail-layout" aria-busy=move||state.busy.get().to_string()>
            <article class="ticket-detail"><div class="detail-overline"><a href="/board" class="back-link"><Icon name="board"/>"Board"</a><span class="breadcrumb-slash">"/"</span><span class="mono">{move||state.ticket.get().map(|t|t.key)}</span></div>
            <h1>{move||state.ticket.get().map(|t|t.title)}</h1>
            {move||state.ticket.get().and_then(|t|{let identity=t.external_id?.strip_prefix("github:")?.to_owned();let(repo,number)=identity.rsplit_once(':')?;Some((format!("Imported from GitHub {repo}#{number}"),t.external_url?))}).map(|(label,url)|view!{<p class="muted github-import"><a class="subtle-link" href=url target="_blank" rel="noopener">{label}<Icon name="external"/></a></p>})}
            <aside class="properties" aria-label="Ticket properties" data-expanded=move||properties_expanded.get().to_string()><PropertiesHeading expanded=properties_expanded/><div id="ticket-properties-body">
            <For each=move||vec![state.revision.get()] key=|r|*r children=move|_|view!{<Properties/>}/>
            </div></aside><div class="detail-description">{move||state.ticket.get().map(|t|view!{<Markdown source=display_body(&t) project=t.key.rsplit_once('-').map(|(p,_)|p.to_owned())/>})}</div>
            <section class="dependencies" aria-label="Dependencies"><div class="section-heading"><h2>"Dependencies"</h2><button class="button button-ghost" disabled=move||state.busy.get() on:click=move|_|dependency.set(true)><Icon name="plus"/>"Add dependency"</button></div>
            {move||state.ticket.get().map(|t|{let empty=t.blocks.is_empty() && t.blocked_by.is_empty();view!{ {empty.then(||view!{<p class="muted dependency-empty">"No dependencies"</p>})}
                {t.blocked_by.into_iter().map(|related|view!{<DependencyRow related incoming=true/>}).collect_view()}{t.blocks.into_iter().map(|related|view!{<DependencyRow related incoming=false/>}).collect_view()}
            }})}</section>
            {move||state.ticket.get().filter(|t|!t.pull_requests.is_empty()).map(|t|view!{
                <section class="dependencies" aria-label="Pull requests"><div class="section-heading"><h2>"Pull requests"</h2></div>
                {t.pull_requests.into_iter().map(|pr|view!{
                    <div class="pr-link-row"><a class="subtle-link" href=pr.url target="_blank" rel="noopener noreferrer"><Icon name="link"/><span>{format!("{}#{} · {}",pr.repo,pr.number,pr.title)}</span><Icon name="external"/></a><PrCheckStatus conclusion=pr.ci_status/><PrCheckStatus label="Tickets" conclusion=pr.tickets_check/><span class="muted">{format!("{} · {}",match pr.state.as_str(){"open"=>"Open","draft"=>"Draft","merged"=>"Merged","closed"=>"Closed",state=>state},if pr.closes{"Closes ticket"}else{"Reference"})}</span></div>
                }).collect_view()}</section>
            })}
            <section class="activity" aria-label="Comments and activity"><div class="section-heading"><h2>"Activity"</h2></div>{move||state.ticket.get().filter(|t|t.external_id.as_deref().is_some_and(|id|id.starts_with("github:"))).map(|t|view!{<div class="import-activity"><Icon name="link"/><span>"Imported from GitHub"</span>{t.external_url.map(|url|view!{<a class="subtle-link" href=url target="_blank" rel="noopener noreferrer">"Source issue"<Icon name="external"/></a>})}</div>})}<ActivityFeed/><Show when=move||!state.comment_error.get().is_empty()><p class="comment-error" role="alert">{move||state.comment_error.get()}</p></Show><CommentEditor/></section>
            </article></div></Show>
        <Presence show=edit><EditTicket close=Callback::new(move|_|edit.set(false))/></Presence>
        <Presence show=dependency><DependencyEditor close=Callback::new(move|_|dependency.set(false))/></Presence>
        {let shown=present_value(board.composer.into());move||shown.get().map(|status|view!{<BoardComposer status/>})}
    }
}
#[component]
fn Properties() -> impl IntoView {
    let state = expect_context::<DetailState>();
    let board = expect_context::<BoardState>();
    let t = state.ticket.get_untracked();
    t.map(|t|{let assignee=RwSignal::new(t.assignee.clone().unwrap_or_default());let status=t.ui_status();view!{
        <div class="property"><span>"Status"</span><div><StatusIcon status/><SelectField label="Status" value=Signal::derive(move||status.name().to_string()) options=Signal::derive(move||options(&Status::ALL.map(Status::name))) on_change=Callback::new(move|next:String|state.patch(json!({"status":Status::from_name(&next)}))) disabled=Signal::derive(move||state.busy.get())/></div></div>
        {move||state.ticket.get().map(|ticket|{let keys=ticket.blocked_by.iter().filter_map(|id|board.tickets.get().into_iter().find(|t|t.id==*id && !matches!(t.status,Status::Done|Status::Canceled))).map(|t|t.key).collect::<Vec<_>>();(!keys.is_empty()).then(||view!{<div class="property"><span>"Dependency"</span><div class="row-blocked" title=format!("Blocked by {}",keys.join(", "))><Icon name="blocked"/>"Blocked"</div></div>})})}
        <div class="property"><span>"Priority"</span><div><PriorityIcon priority=t.priority/><SelectField label="Priority" value=Signal::derive(move||t.priority.name().to_string()) options=Signal::derive(move||options(&Priority::ALL.map(Priority::name))) on_change=Callback::new(move|next:String|{if let Some(p)=Priority::ALL.into_iter().find(|p|p.name()==next){state.patch(json!({"priority":p}));}}) disabled=Signal::derive(move||state.busy.get())/></div></div>
        <form autocomplete="off" data-1p-ignore="true" data-form-type="other" class="property assignee-editor" on:submit=move|e|{e.prevent_default();let name=assignee.get_untracked().trim().to_string();state.patch(json!({"assignee":if name.is_empty(){None}else{Some(name)}}));}><span>"Assignee"</span><div>{move||{let a=assignee.get();view!{<Avatar label=if a.is_empty(){"No assignee".into()}else{a.clone()} initials={a.chars().take(1).collect::<String>()}/>}}}<input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  aria-label="Assignee" disabled=move||state.busy.get() prop:value=move||assignee.get() on:input=move|e|assignee.set(event_target_value(&e)) on:change=move|e|{let _=event_target::<web_sys::HtmlInputElement>(&e).form().map(|f|f.request_submit());}/></div></form>
        <div class="property-section"><h2>"Labels"</h2><div class="property-labels">{t.labels.into_iter().map(|l|view!{<LabelChip name=l.name color=l.color/>}).collect_view()}</div><LabelPicker/></div>
        <div class="property-section"><h2>"Project"</h2><div class="property"><div><SelectField label="Project" value=Signal::derive(move||t.project_id.to_string()) options=Signal::derive(move||board.projects.get().into_iter().map(SelectOption::project).collect()) on_change=Callback::new(move|next:String|{if let Ok(id)=next.parse::<i64>(){state.patch(json!({"project_id":id}));}}) disabled=Signal::derive(move||state.busy.get())/></div></div></div>
        {t.external_url.clone().map(|url|view!{<div class="property-section"><h2>"Links"</h2><a class="subtle-link" href=url target="_blank" rel="noopener noreferrer"><Icon name="link"/>{if url.contains("github.com"){"GitHub issue"}else{"External link"}}<Icon name="external"/></a></div>})}<div class="property-section property-dates"><span>"Created "<time datetime=t.created_at.clone() title=t.created_at.clone()>{date_time(&t.created_at)}</time></span><span>"Updated "<time datetime=t.updated_at.clone() title=t.updated_at.clone()>{date_time(&t.updated_at)}</time></span></div>
    }})
}
fn focus_label_trigger() {
    if let Ok(Some(trigger)) = document().query_selector(".label-menu .menu-trigger")
        && let Some(trigger) = trigger.dyn_ref::<web_sys::HtmlElement>()
    {
        let _ = trigger.focus();
    }
}

/// Create and attach in one UI action; refresh labels first to avoid stale duplicates.
#[component]
fn LabelPicker() -> impl IntoView {
    let state = expect_context::<DetailState>();
    let query = RwSignal::new(String::new());
    let choose = Callback::new(move |existing: Option<i64>| {
        if state.busy.get_untracked() {
            return;
        }
        let name = query.get_untracked().trim().to_string();
        if existing.is_none() && name.is_empty() {
            return;
        }
        state.busy.set(true);
        state.error.set(String::new());
        state.spawn(async move {
            let result = async {
                let labels = all::<tickets_core::domain::Label>("labels").await?;
                let exact = labels
                    .iter()
                    .find(|l| l.name.to_lowercase() == name.to_lowercase());
                let id = if let Some(id) = existing {
                    id
                } else if let Some(label) = exact {
                    label.id
                } else {
                    // The four label hues from tokens.css, stable across sessions.
                    let palette = ["#8b82d9", "#eb5757", "#4ea7fc", "#c6a1b9"];
                    let hash = name
                        .to_lowercase()
                        .bytes()
                        .fold(0usize, |h, b| h.wrapping_mul(31).wrapping_add(b as usize));
                    let label: tickets_core::domain::Label = serde_json::from_value(
                        api(
                            "POST",
                            "/api/v1/labels",
                            json!({"name":name,"color":palette[hash % palette.len()]}),
                        )
                        .await?,
                    )
                    .map_err(|e| e.to_string())?;
                    label.id
                };
                let attached = state
                    .ticket
                    .get_untracked()
                    .is_some_and(|t| t.labels.iter().any(|l| l.id == id));
                if existing.is_some() || !attached {
                    api(
                        if attached { "DELETE" } else { "POST" },
                        &format!("/api/v1/tickets/{}/labels/{id}", state.id.get_untracked()),
                        Value::Null,
                    )
                    .await?;
                }
                state.board.labels.set(all("labels").await?);
                // The authoritative read remounts Properties and disposes query.
                query.set(String::new());
                state.load(false).await?;
                Ok::<_, String>(())
            }
            .await;
            if let Err(error) = result {
                state.error.set(error);
            } else {
                state.board.notice.set("Labels saved.".into());
                // Wait for Properties to remount after the authoritative read.
                gloo_timers::future::TimeoutFuture::new(0).await;
                focus_label_trigger();
            }
            state.busy.set(false);
        });
    });
    view! {<Combobox label="Add label" input_id="label-filter"
    options=Signal::derive(move||state.board.labels.get().into_iter().filter(|l|visible_label(&l.name)).map(|l|SelectOption{color:l.color,..SelectOption::new(l.id,l.name)}).collect())
    selected=Signal::derive(move||state.ticket.get().map(|t|t.labels.into_iter().map(|l|l.id.to_string()).collect()).unwrap_or_default())
    busy=state.busy on_toggle=Callback::new(move|id:String|{if let Ok(id)=id.parse(){choose.run(Some(id));}})
    on_create=Callback::new(move|name:String|{query.set(name);choose.run(None);})/>}
}
#[component]
fn DependencyRow(related: i64, incoming: bool) -> impl IntoView {
    let state = expect_context::<DetailState>();
    let board = expect_context::<BoardState>();
    let removing = RwSignal::new(false);
    let (from, to) = if incoming {
        (related, state.id.get_untracked())
    } else {
        (state.id.get_untracked(), related)
    };
    view! {<div class="dependency-row"><span class="dependency-kind"><Icon name=if incoming{"blocked"}else{"arrow"}/>{if incoming{"Blocked by"}else{"Blocks"}}</span>
    {move||board.tickets.get().into_iter().find(|t|t.id==related).map(|t|view!{<StatusIcon status=t.ui_status()/><span class="mono muted">{t.key.clone()}</span><a class="dependency-title" href=format!("/t/{}",t.key)>{t.title}</a>{matches!(t.status,Status::Done|Status::Canceled).then(||view!{<span class="resolved">"Resolved"</span>})}})}
    <BusyButton class="icon-button" variant="ghost" kind="button" aria_label=format!("Remove dependency {related}") busy=Signal::derive(move||removing.get() && state.busy.get()) disabled=state.busy on_click=Callback::new(move|_|{removing.set(true);state.write("DELETE",format!("/api/v1/tickets/{from}/dependencies/{to}"),Value::Null,||{});})><Icon name="close"/></BusyButton></div>}
}
#[component]
fn DependencyEditor(close: Callback<()>) -> impl IntoView {
    let state = expect_context::<DetailState>();
    let board = expect_context::<BoardState>();
    let pending = RwSignal::new(None::<i64>);
    let direction = RwSignal::new("Is blocked by".to_string());
    let query = RwSignal::new(String::new());
    view! {<Modal title="Add dependency" locked=Signal::derive(move||state.busy.get()) on_close=close><div class="modal-body"><div class="dependency-direction"><Select label="Direction" options=vec!["Is blocked by","Blocks"] value=direction/></div><p class="muted">{move||if direction.get()=="Blocks"{"This ticket must be completed before the selected ticket."}else{"The selected ticket must be completed before this ticket."}}</p><TextInput id="dependency-search" label="Search tickets" value=query/>
    <p role="alert">{move||state.error.get()}</p><div class="dependency-options">{move||board.tickets.get().into_iter().filter(|t|t.id!=state.id.get_untracked() && format!("{} {}",t.key,t.title).to_lowercase().contains(&query.get().to_lowercase())).map(|t|{let related=t.id;view!{<BusyButton class="menu-item" variant="ghost" kind="button" busy=Signal::derive(move||pending.get()==Some(related) && state.busy.get()) disabled=state.busy on_click=Callback::new(move|_|{pending.set(Some(related));let(from,to)=if direction.get_untracked()=="Blocks"{(state.id.get_untracked(),related)}else{(related,state.id.get_untracked())};state.write("POST",format!("/api/v1/tickets/{from}/dependencies/{to}"),Value::Null,move||close.run(()));})><StatusIcon status=t.ui_status()/><span class="mono muted">{t.key}</span>{t.title}</BusyButton>}}).collect_view()}</div></div></Modal>}
}
#[component]
fn EditTicket(close: Callback<()>) -> impl IntoView {
    let state = expect_context::<DetailState>();
    let t = state.ticket.get_untracked().expect("loaded ticket");
    let title = RwSignal::new(t.title);
    let body = RwSignal::new(t.body);
    view! {<Modal title="Edit ticket" locked=Signal::derive(move||state.busy.get()) on_close=close><form autocomplete="off" data-1p-ignore="true" data-form-type="other" on:submit=move|e|{e.prevent_default();state.write("PATCH",format!("/api/v1/tickets/{}",state.id.get_untracked()),json!({"title":title.get_untracked(),"body":body.get_untracked()}),move||close.run(()));} ><div class="modal-body"><TextInput id="detail-title" label="Title" value=title/><MarkdownEditor body label="Description"/><p role="alert">{move||state.error.get()}</p></div><div class="modal-footer"><BusyButton busy=state.busy disabled=Signal::derive(move||title.get().trim().is_empty())>"Save ticket"</BusyButton></div></form></Modal>}
}
#[component]
fn ActivityFeed() -> impl IntoView {
    let state = expect_context::<DetailState>();
    let load_older = move |_| {
        if state.history_busy.get_untracked() {
            return;
        }
        let Some(cursor) = state.history_cursor.get_untracked() else {
            return;
        };
        let revision = state.revision.get_untracked();
        state.history_busy.set(true);
        state.history_error.set(String::new());
        state.spawn(async move {
            let result = api(
                "GET",
                &format!(
                    "/api/v1/tickets/{}/history?cursor={cursor}",
                    state.id.get_untracked()
                ),
                Value::Null,
            )
            .await
            .and_then(|v| serde_json::from_value::<HistoryPage>(v).map_err(|e| e.to_string()));
            if revision == state.revision.get_untracked() {
                match result {
                    Ok(page) => {
                        state.history.update(|entries| entries.extend(page.data));
                        state.history_cursor.set(page.next_cursor);
                    }
                    Err(error) => {
                        state.history_failed_older.set(true);
                        state
                            .history_error
                            .set(format!("Could not load older activity: {error}"));
                    }
                }
            }
            state.history_busy.set(false);
        });
    };
    view! {
        <div class="activity-feed" aria-label="Ticket history">
            <For each=move||{
                let mut entries=state.history.get().into_iter().filter(|e|e.field!="comment_created").map(Activity::Change).chain(state.comments.get().into_iter().map(Activity::Comment)).collect::<Vec<_>>();
                entries.sort_by(|a,b|b.time().cmp(a.time()).then_with(||b.id().cmp(&a.id())));
                entries
            } key=Activity::key children=|entry|match entry {Activity::Change(entry)=>view!{<HistoryEvent entry/>}.into_any(),Activity::Comment(comment)=>view!{<ApiComment comment/>}.into_any()}/>
        </div>
        <Show when=move||!state.history_error.get().is_empty()><div class="history-error" role="alert">{move||state.history_error.get()}<button class="button button-ghost" disabled=move||state.busy.get() || state.history_busy.get() on:click=move|_|{if state.history_failed_older.get_untracked(){load_older(())}else{state.spawn(async move{if let Err(e)=state.load(false).await{state.history_error.set(e);}})}}>"Retry activity"</button></div></Show>
        <Show when=move||state.history_cursor.get().is_some()><BusyButton variant="ghost" class="history-more" busy=state.history_busy disabled=state.busy on_click=Callback::new(load_older)>"Load older activity"</BusyButton></Show>
    }
}

#[component]
fn ApiComment(comment: CommentRecord) -> impl IntoView {
    let deleting = RwSignal::new(false);
    let state = expect_context::<DetailState>();
    let editing = RwSignal::new(false);
    let id = comment.id;
    let confirming = RwSignal::new(false);
    let editable = comment.can_edit && !comment.deleted && !comment.pending;
    let author = if comment.body.starts_with("<!-- tickets-import:github:") {
        comment
            .body
            .split("**@")
            .nth(1)
            .and_then(|s| s.split_once("** on GitHub"))
            .map(|(login, _)| format!("@{login}"))
            .unwrap_or_else(|| comment.author.clone())
    } else {
        comment.author.clone()
    };
    let display_body = if comment.body.starts_with("<!-- tickets-import:github:") {
        comment
            .body
            .split_once("** on GitHub\n\n")
            .map(|(_, body)| body.to_owned())
            .unwrap_or_else(|| comment.body.clone())
    } else {
        comment.body.clone()
    };
    let is_bot = comment.author_kind == "bot";
    let deleted = comment.deleted;
    let pending = comment.pending;
    let edited = comment.edited_at.clone();
    let initial_body = StoredValue::new(comment.body.clone());
    let display_body = StoredValue::new(display_body);
    let comment_root = NodeRef::<leptos::html::Div>::new();
    // Unchanged comments retain their keyed nodes and never replay.
    use_ascii_reveal(comment_root, Signal::derive(|| false), comment.reveal);
    view! {<div node_ref=comment_root class="comment" class:comment-deleted=deleted class:comment-pending=pending data-comment-id=id on:keydown=move|e|{if e.key()=="Escape" && confirming.get_untracked() && !state.busy.get_untracked(){e.prevent_default();e.stop_propagation();confirming.set(false);focus_thread(format!("[data-comment-id='{id}'] .comment-action-delete"));}} aria-busy=pending.to_string()><Avatar label=author.clone() initials={author.trim_start_matches('@').chars().take(1).collect::<String>().to_uppercase()}/><div class="comment-content"><div class="comment-heading"><strong>{author}</strong>{is_bot.then(||view!{<span class="comment-bot">"Bot"</span>})}<ActivityTime iso=comment.created_at/>{edited.map(|time|view!{<ActivityTime iso=time edited=true/>})}{pending.then(||view!{<span role="status" class="muted">"Posting…"</span>})}<Show when=move||editable && !editing.get() && !confirming.get()><button type="button" class="button button-ghost comment-action comment-action-edit" disabled=move||state.busy.get() on:click=move|_|{confirming.set(false);editing.set(true);focus_thread(format!("[data-comment-id='{id}'] textarea"));}>"Edit"</button><button type="button" class="button button-ghost comment-action comment-action-delete" disabled=move||state.busy.get() on:click=move|_|{confirming.set(true);focus_thread(format!("[data-comment-id='{id}'] .comment-confirm button"));}>"Delete"</button></Show></div>
    <Show when=move||confirming.get()><div class="comment-confirm" role="group" aria-label="Confirm comment deletion"><span>"Delete this comment?"</span><BusyButton kind="button" status_label="Deleting comment" variant="ghost" busy=Signal::derive(move||deleting.get() && state.busy.get()) disabled=state.busy on_click=Callback::new(move|_|{deleting.set(true);state.write("DELETE",format!("/api/v1/comments/{id}"),Value::Null,||{});})>"Confirm delete"</BusyButton><button type="button" class="button button-ghost" disabled=move||state.busy.get() on:click=move|_|{confirming.set(false);focus_thread(format!("[data-comment-id='{id}'] .comment-action-delete"));}>"Cancel"</button></div></Show>
    {if deleted {view!{<p class="muted">"Comment deleted"</p>}.into_any()} else {view!{<Show when=move||editing.get() fallback=move||view!{<Markdown source=display_body.get_value() project=state.ticket.get().and_then(|t|t.key.rsplit_once('-').map(|(p,_)|p.to_owned()))/>}><CommentEditor id=id initial=initial_body.get_value() close=Callback::new(move|_|{editing.set(false);focus_thread(format!("[data-comment-id='{id}'] .comment-action-edit"));})/></Show>}.into_any()}}</div></div>}
}
#[component]
fn CommentEditor(
    #[prop(optional)] id: Option<i64>,
    #[prop(default=String::new())] initial: String,
    #[prop(optional)] close: Option<Callback<()>>,
) -> impl IntoView {
    let state = expect_context::<DetailState>();
    let body = RwSignal::new(initial);
    let form = NodeRef::<leptos::html::Form>::new();
    let account = expect_context::<crate::pages::settings::AccountState>();
    let submit = move || {
        let text = body.get_untracked();
        if state.busy.get_untracked() || text.trim().is_empty() {
            return;
        }
        let path = id
            .map(|id| format!("/api/v1/comments/{id}"))
            .unwrap_or_else(|| format!("/api/v1/tickets/{}/comments", state.id.get_untracked()));
        if id.is_some() {
            state.write("PATCH", path, json!({"body":text}), move || {
                body.set(String::new());
                if let Some(close) = close {
                    close.run(());
                }
            });
        } else {
            let snapshot = state.comments.get_untracked();
            state.busy.set(true);
            state.comment_error.set(String::new());
            state.comments.update(|comments| {
                comments.push(CommentRecord {
                    id: -1,
                    body: text.clone(),
                    author: account.account.get_untracked().label(),
                    author_kind: "user".into(),
                    created_at: js_sys::Date::new_0()
                        .to_iso_string()
                        .as_string()
                        .unwrap_or_default(),
                    edited_at: None,
                    deleted: false,
                    can_edit: false,
                    pending: true,
                    reveal: true,
                })
            });
            state.spawn(async move {
                match api("POST", &path, json!({"body":text})).await {
                    Ok(value) => {
                        body.set(String::new());
                        if let Ok(saved) = serde_json::from_value::<CommentRecord>(value) {
                            state.comments.update(|comments| {
                                comments.retain(|c| !c.pending);
                                comments.push(CommentRecord {
                                    reveal: true,
                                    ..saved
                                });
                            });
                        }
                        if let Err(error) = state.load(false).await {
                            state
                                .comment_error
                                .set(format!("Comment posted, but could not reload: {error}"));
                        }
                    }
                    Err(error) => {
                        state.comments.set(snapshot);
                        state.comment_error.set(error);
                    }
                }
                state.busy.set(false);
                focus_thread(".activity > .comment-form textarea".into());
            });
        }
    };
    view! {<form node_ref=form autocomplete="off" data-1p-ignore="true" data-form-type="other" class="comment-form" class:has-draft=move||id.is_some() || !body.get().is_empty() on:keydown=move|e|{
        if e.key()=="Enter" && (e.meta_key() || e.ctrl_key()) && !e.is_composing() {e.prevent_default();e.stop_propagation();if let Some(form)=form.get(){let _=form.request_submit();}}
        if e.key()=="Escape" && !state.busy.get_untracked() && let Some(close)=close {e.prevent_default();e.stop_propagation();close.run(());}
    } on:submit=move|e|{e.prevent_default();submit();}><MarkdownEditor body label="Write a comment" disabled=state.busy/><div><span>{if id.is_some(){"Markdown · ⌘/Ctrl+Enter to save"}else{"Markdown · ⌘/Ctrl+Enter to send"}}</span>{close.map(|close|view!{<button class="button button-ghost" type="button" disabled=move||state.busy.get() on:click=move|_|close.run(())>"Cancel"</button>})}<BusyButton busy=state.busy status_label="Saving comment" variant="secondary" disabled=Signal::derive(move||body.get().trim().is_empty())>{if id.is_some(){"Save comment"}else{"Comment"}}<Icon name="arrow"/></BusyButton></div></form>}
}
#[component]
pub(crate) fn MarkdownEditor(
    body: RwSignal<String>,
    label: &'static str,
    #[prop(optional, into)] disabled: Signal<bool>,
) -> impl IntoView {
    let mode = RwSignal::new("Write".to_string());
    let preview = Signal::derive(move || mode.get() == "Preview");
    view! {<div class="markdown-editor"><SegmentedControl label="Editor mode" options=vec!["Write","Preview"] value=mode/><Show when=move||preview.get() fallback=move||view!{<label class="field"><span>{label}</span><textarea autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  aria-label=label placeholder=if label=="Write a comment"{"Leave a comment…"}else{"Add details…"} rows="4" disabled=move||disabled.get() prop:value=move||body.get() on:input=move|e|body.set(event_target_value(&e))/></label>}><div class="markdown-preview">{move||view!{<Markdown source=body.get() project={use_context::<DetailState>().and_then(|state|state.ticket.get()).and_then(|t|t.key.rsplit_once('-').map(|(p,_)|p.to_owned()))}/>}}</div></Show></div>}
}

/// `2026-10-28T…` → `Oct 28`; falls back to the raw value.
fn short_date(iso: &str) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let month = iso
        .get(5..7)
        .and_then(|m| m.parse::<usize>().ok())
        .filter(|m| (1..=12).contains(m));
    let day = iso.get(8..10).and_then(|d| d.parse::<u32>().ok());
    match (month, day) {
        (Some(m), Some(d)) => format!("{} {d}", MONTHS[m - 1]),
        _ => iso.to_owned(),
    }
}

/// Redundant import metadata is hidden only when the matching model label exists.
fn display_body(ticket: &BoardTicket) -> String {
    let mut fenced = false;
    ticket
        .body
        .lines()
        .filter(|line| {
            if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
                fenced = !fenced;
                return true;
            }
            if fenced {
                return true;
            }
            let plain = line.trim().replace('*', "");
            !plain.strip_prefix("Model:").is_some_and(|value| {
                ticket.labels.iter().any(|label| {
                    label
                        .name
                        .eq_ignore_ascii_case(&format!("model:{}", value.trim()))
                })
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn date_time(iso: &str) -> String {
    iso.get(11..16)
        .map(|time| format!("{} at {time} UTC", short_date(iso)))
        .unwrap_or_else(|| iso.to_owned())
}

#[component]
fn PropertiesHeading(expanded: RwSignal<bool>) -> impl IntoView {
    view! {<h2 class="properties-heading">"Properties"</h2><button class="button button-ghost properties-toggle" type="button" aria-controls="ticket-properties-body" aria-expanded=move||expanded.get().to_string() on:click=move|_|{
        expanded.update(|v| *v = !*v);
    }>"Properties"<Icon name="chevron"/></button>}
}

// Focus only document nodes after async reconciliation; child signals may be disposed.
fn focus_thread(selector: String) {
    request_animation_frame(move || {
        if let Ok(Some(element)) = document().query_selector(&selector)
            && let Some(element) = element.dyn_ref::<web_sys::HtmlElement>()
        {
            let _ = element.focus();
        }
    });
}
