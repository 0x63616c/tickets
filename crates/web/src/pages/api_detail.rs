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
}
#[derive(Clone, Copy)]
struct DetailState {
    owner: StoredValue<leptos::reactive::owner::WeakOwner>,
    board: BoardState,
    id: RwSignal<i64>,
    selector: StoredValue<String>,
    ticket: RwSignal<Option<BoardTicket>>,
    comments: RwSignal<Vec<CommentRecord>>,
    busy: RwSignal<bool>,
    loading: RwSignal<bool>,
    error: RwSignal<String>,
    revision: RwSignal<u64>,
}
impl DetailState {
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
        let comments =
            all::<CommentRecord>(&format!("tickets/{}/comments", self.id.get_untracked())).await?;
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
        self.ticket.set(Some(ticket));
        self.comments.set(comments);
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
        self.error.set(String::new());
        self.spawn(async move {
            match api(method, &path, body).await {
                Ok(_) => {
                    done();
                    if let Err(e) = self.load(false).await {
                        self.error.set(format!("Saved, but could not reload: {e}"));
                    }
                    self.board.notice.set("Saved.".into());
                }
                Err(e) => {
                    self.error.set(e);
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
        busy: RwSignal::new(false),
        loading: RwSignal::new(true),
        error: RwSignal::new(String::new()),
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
    Effect::new(move |_| {
        if board.catchup_revision.get() > 0 {
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
    view! {
        <header class="topbar"><div class="breadcrumbs"><a href="/projects"><Icon name="project"/>"Projects"</a><span class="breadcrumb-slash">"/"</span><a href="/">{move||state.ticket.get().and_then(|t|board.projects.get().into_iter().find(|p|p.id==t.project_id)).map(|p|view!{<ProjectIcon name=p.name.clone() icon=p.icon.unwrap_or_default() color=p.color/>{p.name}})}</a><span class="breadcrumb-slash">"/"</span><span class="mono">{move||state.ticket.get().map(|t|t.key).unwrap_or_default()}</span></div><div class="topbar-actions"><crate::live::LiveIndicator/><span class="toolbar-divider"/><button class="button button-ghost" disabled=move||state.busy.get() || state.ticket.get().is_none() on:click=move|_|edit.set(true)>"Edit ticket"</button><button class="button button-primary" disabled=move||board.loading.get() || board.project.get()==0 on:click=move|_|board.composer.set(Some(Status::Todo))><Icon name="plus"/>"New ticket"<Shortcut>"C"</Shortcut></button></div></header>
        <Show when=move||state.loading.get() && state.ticket.get().is_none()><LoadingSkeleton layout="detail"/></Show>
        <Show when=move||!state.error.get().is_empty() && !edit.get() && !dependency.get()><div class="board-message" role="alert">{move||state.error.get()}<button class="button" disabled=move||state.busy.get() on:click=move|_|{state.busy.set(true);state.spawn(async move{state.error.set(String::new());if let Err(e)=state.load(false).await{state.error.set(e);}state.busy.set(false);});}>"Reload ticket"</button></div></Show>
        <Show when=move||state.ticket.get().is_some()><div class="detail-layout" aria-busy=move||state.busy.get().to_string()>
            <article class="ticket-detail"><div class="detail-overline"><a href="/" class="back-link"><Icon name="board"/>"Board"</a><span class="breadcrumb-slash">"/"</span><span class="mono">{move||state.ticket.get().map(|t|t.key)}</span></div>
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
                    <div class="pr-link-row"><a class="subtle-link" href=pr.url target="_blank" rel="noopener noreferrer"><Icon name="link"/><span>{format!("{}#{} · {}",pr.repo,pr.number,pr.title)}</span><Icon name="external"/></a><span class="muted">{format!("{} · {}",match pr.state.as_str(){"open"=>"Open","draft"=>"Draft","merged"=>"Merged","closed"=>"Closed",state=>state},if pr.closes{"Closes ticket"}else{"Reference"})}</span></div>
                }).collect_view()}</section>
            })}
            <section class="activity" aria-label="Comments and activity"><div class="section-heading"><h2>"Activity"</h2></div>{move||state.ticket.get().filter(|t|t.external_id.as_deref().is_some_and(|id|id.starts_with("github:"))).map(|t|view!{<div class="import-activity"><Icon name="link"/><span>"Imported from GitHub"</span>{t.external_url.map(|url|view!{<a class="subtle-link" href=url target="_blank" rel="noopener noreferrer">"Source issue"<Icon name="external"/></a>})}</div>})}<For each=move||state.comments.get() key=|c|(c.id,c.body.clone()) children=|comment|view!{<ApiComment comment/>}/><CommentEditor/></section>
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
fn ApiComment(comment: CommentRecord) -> impl IntoView {
    let deleting = RwSignal::new(false);
    let state = expect_context::<DetailState>();
    let editing = RwSignal::new(false);
    let id = comment.id;
    let editable = comment.author == "owner";
    let author = if comment.body.starts_with("<!-- tickets-import:github:") {
        comment
            .body
            .split("**@")
            .nth(1)
            .and_then(|s| s.split_once("** on GitHub"))
            .map(|(login, _)| format!("@{login}"))
            .unwrap_or_else(|| "GitHub".into())
    } else if comment.author != "owner" {
        comment.author.clone()
    } else {
        "You".into()
    };
    let display_body = if author != "You" {
        comment
            .body
            .split_once("** on GitHub\n\n")
            .map(|(_, body)| body.to_owned())
            .unwrap_or_else(|| comment.body.clone())
    } else {
        comment.body.clone()
    };
    view! {<div class="comment"><Avatar label=author.clone() initials={author.trim_start_matches('@').chars().take(1).collect::<String>()}/><div class="comment-content"><div class="comment-heading"><strong>{author}</strong><time>{short_date(&comment.created_at)}</time><Show when=move||editable><button class="button button-ghost comment-action" disabled=move||state.busy.get() on:click=move|_|editing.set(true)>"Edit"</button><BusyButton kind="button" variant="ghost" class="comment-action" busy=Signal::derive(move||deleting.get() && state.busy.get()) disabled=state.busy on_click=Callback::new(move|_|{deleting.set(true);state.write("DELETE",format!("/api/v1/comments/{id}"),Value::Null,||{});})>"Delete"</BusyButton></Show></div><Show when=move||editing.get() fallback={let body=display_body.clone();move||view!{<Markdown source=body.clone() project=state.ticket.get().and_then(|t|t.key.rsplit_once('-').map(|(p,_)|p.to_owned()))/>}}><CommentEditor id=id initial=comment.body.clone() close=Callback::new(move|_|editing.set(false))/></Show></div></div>}
}
#[component]
fn CommentEditor(
    #[prop(optional)] id: Option<i64>,
    #[prop(default=String::new())] initial: String,
    #[prop(optional)] close: Option<Callback<()>>,
) -> impl IntoView {
    let state = expect_context::<DetailState>();
    let body = RwSignal::new(initial);
    view! {<form autocomplete="off" data-1p-ignore="true" data-form-type="other" class="comment-form" class:has-draft=move||id.is_some() || !body.get().is_empty() on:submit=move|e|{e.prevent_default();let path=id.map(|id|format!("/api/v1/comments/{id}")).unwrap_or_else(||format!("/api/v1/tickets/{}/comments",state.id.get_untracked()));state.write(if id.is_some(){"PATCH"}else{"POST"},path,json!({"body":body.get_untracked()}),move||{body.set(String::new());if let Some(close)=close{close.run(());}});}><MarkdownEditor body label="Write a comment"/><div><span>"Markdown supported"</span>{close.map(|close|view!{<button class="button button-ghost" type="button" disabled=move||state.busy.get() on:click=move|_|close.run(())>"Cancel"</button>})}<BusyButton busy=state.busy variant="secondary" disabled=Signal::derive(move||body.get().trim().is_empty())>{if id.is_some(){"Save comment"}else{"Comment"}}<Icon name="arrow"/></BusyButton></div></form>}
}
#[component]
pub(crate) fn MarkdownEditor(body: RwSignal<String>, label: &'static str) -> impl IntoView {
    let mode = RwSignal::new("Write".to_string());
    let preview = Signal::derive(move || mode.get() == "Preview");
    view! {<div class="markdown-editor"><SegmentedControl label="Editor mode" options=vec!["Write","Preview"] value=mode/><Show when=move||preview.get() fallback=move||view!{<label class="field"><span>{label}</span><textarea autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  aria-label=label placeholder=if label=="Write a comment"{"Leave a comment…"}else{"Add details…"} rows="4" prop:value=move||body.get() on:input=move|e|body.set(event_target_value(&e))/></label>}><div class="markdown-preview">{move||view!{<Markdown source=body.get() project={use_context::<DetailState>().and_then(|state|state.ticket.get()).and_then(|t|t.key.rsplit_once('-').map(|(p,_)|p.to_owned()))}/>}}</div></Show></div>}
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
