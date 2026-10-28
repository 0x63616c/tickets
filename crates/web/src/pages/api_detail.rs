//! Persistent ticket editor using the fixture's shared detail structure and controls.
use super::board::{BoardComposer, BoardState, BoardTicket, all};
use crate::{
    components::*,
    fixtures::{Priority, Status},
    ui::api,
};
use leptos::{ev, prelude::*};
use leptos_router::hooks::use_query_map;
use serde::Deserialize;
use serde_json::{Value, json};
use wasm_bindgen_futures::spawn_local;
use web_sys::wasm_bindgen::JsCast;

#[derive(Clone, Deserialize)]
struct CommentRecord {
    id: i64,
    body: String,
    created_at: String,
}
#[derive(Clone, Copy)]
struct DetailState {
    board: BoardState,
    id: i64,
    ticket: RwSignal<Option<BoardTicket>>,
    comments: RwSignal<Vec<CommentRecord>>,
    busy: RwSignal<bool>,
    loading: RwSignal<bool>,
    error: RwSignal<String>,
    revision: RwSignal<u64>,
}
impl DetailState {
    async fn load(self) -> Result<(), String> {
        let ticket: BoardTicket = serde_json::from_value(
            api("GET", &format!("/api/v1/tickets/{}", self.id), Value::Null).await?,
        )
        .map_err(|e| e.to_string())?;
        let comments = all::<CommentRecord>(&format!("tickets/{}/comments", self.id)).await?;
        self.board.project.set(ticket.project_id);
        self.board.apply_change_event(ticket.clone(), false);
        self.ticket.set(Some(ticket));
        self.comments.set(comments);
        self.revision.update(|r| *r += 1);
        Ok(())
    }
    fn write(self, method: &'static str, path: String, body: Value, done: Callback<()>) {
        if self.busy.get_untracked() {
            return;
        }
        self.busy.set(true);
        self.error.set(String::new());
        spawn_local(async move {
            match api(method, &path, body).await {
                Ok(_) => {
                    done.run(());
                    if let Err(e) = self.load().await {
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
            format!("/api/v1/tickets/{}", self.id),
            body,
            Callback::new(|_| {}),
        );
    }
}
#[component]
pub fn ApiDetail() -> impl IntoView {
    let query = use_query_map();
    view! {<For each=move||{query.get().get("id").and_then(|id|id.parse::<i64>().ok()).into_iter().collect::<Vec<_>>()} key=|id|*id children=|id|view!{<DetailPage id/>}/>}
}
#[component]
fn DetailPage(id: i64) -> impl IntoView {
    let board = expect_context::<BoardState>();
    let state = DetailState {
        board,
        id,
        ticket: RwSignal::new(None),
        comments: RwSignal::new(vec![]),
        busy: RwSignal::new(false),
        loading: RwSignal::new(true),
        error: RwSignal::new(String::new()),
        revision: RwSignal::new(0),
    };
    provide_context(state);
    board.refresh();
    spawn_local(async move {
        if let Err(e) = state.load().await {
            state.error.set(e);
        }
        state.loading.set(false);
    });
    let edit = RwSignal::new(false);
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
        <header class="topbar"><div class="breadcrumbs"><a href="/"><Icon name="project"/>"Projects"</a><span class="breadcrumb-slash">"/"</span><a href="/">{move||state.ticket.get().and_then(|t|board.projects.get().into_iter().find(|p|p.id==t.project_id)).map(|p|p.name).unwrap_or_default()}</a><span class="breadcrumb-slash">"/"</span><span class="mono">{move||state.ticket.get().map(|t|t.key).unwrap_or_default()}</span></div><div class="topbar-actions"><button class="button button-ghost" disabled=move||state.busy.get() || state.ticket.get().is_none() on:click=move|_|edit.set(true)>"Edit ticket"</button><button class="button button-primary" disabled=move||board.loading.get() || board.project.get()==0 on:click=move|_|board.composer.set(Some(Status::Todo))><Icon name="plus"/>"New ticket"<Shortcut>"C"</Shortcut></button></div></header>
        <Show when=move||state.loading.get()><p class="board-message" role="status">"Loading ticket…"</p></Show>
        <Show when=move||!state.error.get().is_empty() && !edit.get() && !dependency.get()><div class="board-message" role="alert">{move||state.error.get()}<Show when=move||state.error.get().contains("sign in")><a href="/login">"Sign in"</a></Show><button class="button" disabled=move||state.busy.get() on:click=move|_|{state.busy.set(true);spawn_local(async move{state.error.set(String::new());if let Err(e)=state.load().await{state.error.set(e);}state.busy.set(false);});}>"Reload ticket"</button></div></Show>
        <Show when=move||state.ticket.get().is_some()><div class="detail-layout" aria-busy=move||state.busy.get().to_string()>
            <article class="ticket-detail"><div class="detail-overline"><a href="/" class="back-link"><Icon name="board"/>"Board"</a><span class="breadcrumb-slash">"/"</span><span class="mono">{move||state.ticket.get().map(|t|t.key)}</span></div>
            <h1>{move||state.ticket.get().map(|t|t.title)}</h1><div class="detail-description">{move||state.ticket.get().map(|t|view!{<Markdown source=t.body/>})}</div>
            <section class="dependencies" aria-label="Dependencies"><div class="section-heading"><h2>"Dependencies"</h2><button class="button button-ghost" disabled=move||state.busy.get() on:click=move|_|dependency.set(true)><Icon name="plus"/>"Add dependency"</button></div>
            {move||state.ticket.get().map(|t|{let empty=t.blocks.is_empty() && t.blocked_by.is_empty();view!{ {empty.then(||view!{<p class="muted dependency-empty">"No dependencies. This ticket can be worked on independently."</p>})}
                {t.blocked_by.into_iter().map(|related|view!{<DependencyRow related incoming=true/>}).collect_view()}{t.blocks.into_iter().map(|related|view!{<DependencyRow related incoming=false/>}).collect_view()}
            }})}</section>
            <section class="activity" aria-label="Comments and activity"><div class="section-heading"><h2>"Activity"</h2></div><For each=move||state.comments.get() key=|c|(c.id,c.body.clone()) children=|comment|view!{<ApiComment comment/>}/><CommentEditor/></section>
            </article><aside class="properties" aria-label="Ticket properties"><h2>"Properties"</h2>
            <For each=move||vec![state.revision.get()] key=|r|*r children=move|_|view!{<Properties/>}/>
            </aside></div></Show>
        {move||edit.get().then(||view!{<EditTicket close=Callback::new(move|_|edit.set(false))/>})}
        {move||dependency.get().then(||view!{<DependencyEditor close=Callback::new(move|_|dependency.set(false))/>})}
        {move||board.composer.get().map(|status|view!{<BoardComposer status/>})}
    }
}
#[component]
fn Properties() -> impl IntoView {
    let state = expect_context::<DetailState>();
    let board = expect_context::<BoardState>();
    let t = state.ticket.get_untracked();
    t.map(|t|{let assignee=RwSignal::new(t.assignee.unwrap_or_default());view!{
        <div class="property"><span>"Status"</span><div><StatusIcon status=t.status/><select aria-label="Status" disabled=move||state.busy.get() prop:value=t.status.name() on:change=move|e|state.patch(json!({"status":Status::from_name(&event_target_value(&e))}))>{Status::ALL.into_iter().map(|s|view!{<option selected=s==t.status>{s.name()}</option>}).collect_view()}</select></div></div>
        <div class="property"><span>"Priority"</span><div><PriorityIcon priority=t.priority/><select aria-label="Priority" disabled=move||state.busy.get() prop:value=t.priority.name() on:change=move|e|{if let Some(p)=Priority::ALL.into_iter().find(|p|p.name()==event_target_value(&e)){state.patch(json!({"priority":p}));}}>{Priority::ALL.into_iter().map(|p|view!{<option selected=p==t.priority>{p.name()}</option>}).collect_view()}</select></div></div>
        <form class="property assignee-editor" on:submit=move|e|{e.prevent_default();let name=assignee.get_untracked().trim().to_string();state.patch(json!({"assignee":if name.is_empty(){None}else{Some(name)}}));}><span>"Assignee"</span><div>{move||{let a=assignee.get();(!a.trim().is_empty()).then(||view!{<Avatar label=a.clone() initials={a.chars().take(1).collect::<String>()}/>})}}<input aria-label="Assignee" placeholder="No assignee" disabled=move||state.busy.get() prop:value=move||assignee.get() on:input=move|e|assignee.set(event_target_value(&e)) on:change=move|e|{let _=event_target::<web_sys::HtmlInputElement>(&e).form().map(|f|f.request_submit());}/></div></form>
        <div class="property-section"><h2>"Labels"</h2><div class="property-labels">{t.labels.into_iter().map(|l|view!{<LabelChip name=l.name color=l.color/>}).collect_view()}</div><Menu label="Add label">{move||board.labels.get().into_iter().map(|l|{let selected=state.ticket.get().is_some_and(|t|t.labels.iter().any(|v|v.id==l.id));view!{<button class="menu-item" aria-pressed=selected disabled=move||state.busy.get() on:click=move|_|state.write(if selected{"DELETE"}else{"POST"},format!("/api/v1/tickets/{}/labels/{}",state.id,l.id),Value::Null,Callback::new(|_|{}))><LabelChip name=l.name color=l.color/>{selected.then(||view!{<Icon name="check"/>})}</button>}}).collect_view()}</Menu></div>
        <div class="property-section"><h2>"Project"</h2><div class="property"><div><select aria-label="Project" disabled=move||state.busy.get() prop:value=t.project_id.to_string() on:change=move|e|{if let Ok(id)=event_target_value(&e).parse::<i64>(){state.patch(json!({"project_id":id}));}}>{move||board.projects.get().into_iter().map(|p|view!{<option value=p.id.to_string() selected=p.id==t.project_id>{p.name}</option>}).collect_view()}</select></div></div></div>
        {t.external_url.clone().map(|url|view!{<div class="property-section"><h2>"Links"</h2><a class="subtle-link" href=url target="_blank" rel="noopener noreferrer"><Icon name="link"/>{if url.contains("github.com"){"GitHub issue"}else{"External link"}}<Icon name="external"/></a></div>})}<div class="property-section property-dates"><span>{format!("Created {}",short_date(&t.created_at))}</span><span>{format!("Updated {}",short_date(&t.updated_at))}</span></div>
    }})
}
#[component]
fn DependencyRow(related: i64, incoming: bool) -> impl IntoView {
    let state = expect_context::<DetailState>();
    let board = expect_context::<BoardState>();
    let (from, to) = if incoming {
        (related, state.id)
    } else {
        (state.id, related)
    };
    view! {<div class="dependency-row"><span class="dependency-kind"><Icon name=if incoming{"blocked"}else{"arrow"}/>{if incoming{"Blocked by"}else{"Blocks"}}</span>
    {move||board.tickets.get().into_iter().find(|t|t.id==related).map(|t|view!{<StatusIcon status=t.status/><span class="mono muted">{t.key}</span><a class="dependency-title" href=format!("/ticket?id={related}")>{t.title}</a>{matches!(t.status,Status::Done|Status::Canceled).then(||view!{<span class="resolved">"Resolved"</span>})}})}
    <button class="icon-button" aria-label=format!("Remove dependency {related}") disabled=move||state.busy.get() on:click=move|_|state.write("DELETE",format!("/api/v1/tickets/{from}/dependencies/{to}"),Value::Null,Callback::new(|_|{}))><Icon name="close"/></button></div>}
}
#[component]
fn DependencyEditor(close: Callback<()>) -> impl IntoView {
    let state = expect_context::<DetailState>();
    let board = expect_context::<BoardState>();
    let direction = RwSignal::new("Is blocked by".to_string());
    let query = RwSignal::new(String::new());
    view! {<Modal title="Add dependency" locked=Signal::derive(move||state.busy.get()) on_close=close><div class="modal-body"><div class="dependency-direction"><Select label="Direction" options=vec!["Is blocked by","Blocks"] value=direction/></div><p class="muted">{move||if direction.get()=="Blocks"{"This ticket must be completed before the selected ticket."}else{"The selected ticket must be completed before this ticket."}}</p><TextInput id="dependency-search" label="Search tickets" value=query/>
    <p role="alert">{move||state.error.get()}</p><div class="dependency-options">{move||board.tickets.get().into_iter().filter(|t|t.id!=state.id && format!("{} {}",t.key,t.title).to_lowercase().contains(&query.get().to_lowercase())).map(|t|{let related=t.id;view!{<button class="menu-item" disabled=move||state.busy.get() on:click=move|_|{let(from,to)=if direction.get_untracked()=="Blocks"{(state.id,related)}else{(related,state.id)};state.write("POST",format!("/api/v1/tickets/{from}/dependencies/{to}"),Value::Null,close);} ><StatusIcon status=t.status/><span class="mono muted">{t.key}</span>{t.title}</button>}}).collect_view()}</div></div></Modal>}
}
#[component]
fn EditTicket(close: Callback<()>) -> impl IntoView {
    let state = expect_context::<DetailState>();
    let t = state.ticket.get_untracked().expect("loaded ticket");
    let title = RwSignal::new(t.title);
    let body = RwSignal::new(t.body);
    view! {<Modal title="Edit ticket" locked=Signal::derive(move||state.busy.get()) on_close=close><form on:submit=move|e|{e.prevent_default();state.write("PATCH",format!("/api/v1/tickets/{}",state.id),json!({"title":title.get_untracked(),"body":body.get_untracked()}),close);} ><div class="modal-body"><TextInput id="detail-title" label="Title" value=title/><MarkdownEditor body label="Description"/><p role="alert">{move||state.error.get()}</p></div><div class="modal-footer"><button class="button button-primary" type="submit" disabled=move||state.busy.get() || title.get().trim().is_empty()>"Save ticket"</button></div></form></Modal>}
}
#[component]
fn ApiComment(comment: CommentRecord) -> impl IntoView {
    let state = expect_context::<DetailState>();
    let editing = RwSignal::new(false);
    let id = comment.id;
    view! {<div class="comment"><Avatar/><div class="comment-content"><div class="comment-heading"><strong>"You"</strong><time>{short_date(&comment.created_at)}</time><button class="button button-ghost comment-action" disabled=move||state.busy.get() on:click=move|_|editing.set(true)>"Edit"</button><button class="button button-ghost comment-action" disabled=move||state.busy.get() on:click=move|_|state.write("DELETE",format!("/api/v1/comments/{id}"),Value::Null,Callback::new(|_|{}))>"Delete"</button></div><Show when=move||editing.get() fallback={let body=comment.body.clone();move||view!{<Markdown source=body.clone()/>}}><CommentEditor id=id initial=comment.body.clone() close=Callback::new(move|_|editing.set(false))/></Show></div></div>}
}
#[component]
fn CommentEditor(
    #[prop(optional)] id: Option<i64>,
    #[prop(default=String::new())] initial: String,
    #[prop(optional)] close: Option<Callback<()>>,
) -> impl IntoView {
    let state = expect_context::<DetailState>();
    let body = RwSignal::new(initial);
    view! {<form class="comment-form" on:submit=move|e|{e.prevent_default();let path=id.map(|id|format!("/api/v1/comments/{id}")).unwrap_or_else(||format!("/api/v1/tickets/{}/comments",state.id));state.write(if id.is_some(){"PATCH"}else{"POST"},path,json!({"body":body.get_untracked()}),Callback::new(move|_|{body.set(String::new());if let Some(close)=close{close.run(());}}));}><MarkdownEditor body label="Write a comment"/><div><span>"Markdown supported"</span>{close.map(|close|view!{<button class="button button-ghost" type="button" disabled=move||state.busy.get() on:click=move|_|close.run(())>"Cancel"</button>})}<button class="button button-secondary" type="submit" disabled=move||state.busy.get() || body.get().trim().is_empty()>{if id.is_some(){"Save comment"}else{"Comment"}}<Icon name="arrow"/></button></div></form>}
}
#[component]
pub(crate) fn MarkdownEditor(body: RwSignal<String>, label: &'static str) -> impl IntoView {
    let preview = RwSignal::new(false);
    view! {<div class="markdown-editor"><div class="editor-tabs" role="group" aria-label=format!("{label} mode")><button class="button button-ghost" type="button" aria-pressed=move||!preview.get() on:click=move|_|preview.set(false)>"Write"</button><button class="button button-ghost" type="button" aria-pressed=move||preview.get() on:click=move|_|preview.set(true)>"Preview"</button></div><Show when=move||preview.get() fallback=move||view!{<label class="field"><span>{label}</span><textarea aria-label=label rows="4" prop:value=move||body.get() on:input=move|e|body.set(event_target_value(&e))/></label>}><div class="markdown-preview">{move||view!{<Markdown source=body.get()/>}}</div></Show></div>}
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
