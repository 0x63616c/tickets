//! Shared ticket creation surface; callers own persistence and fixture data.
use super::*;
use leptos::{html, prelude::*};
use tickets_core::domain::{Label, Project};

#[derive(Clone)]
pub struct TicketDraft {
    pub title: String,
    pub body: String,
    pub project_id: i64,
    pub status: Status,
    pub priority: crate::fixtures::Priority,
    pub labels: Vec<String>,
}

#[component]
pub fn TicketComposer(
    project_id: i64,
    status: Status,
    projects: Signal<Vec<Project>>,
    labels: Signal<Vec<Label>>,
    open: Signal<bool>,
    busy: Signal<bool>,
    error: Signal<String>,
    #[prop(default = Signal::derive(|| false))] created: Signal<bool>,
    on_submit: Callback<TicketDraft>,
    on_close: Callback<()>,
) -> impl IntoView {
    let title = RwSignal::new(String::new());
    let body = RwSignal::new(String::new());
    let project = RwSignal::new(project_id.to_string());
    let status = RwSignal::new(status.name().to_string());
    let priority = RwSignal::new("No priority".to_string());
    let selected = RwSignal::new(Vec::<String>::new());
    let mode = RwSignal::new("Write".to_string());
    let discard = RwSignal::new(false);
    let keep = NodeRef::<html::Button>::new();
    let shortcut = RwSignal::new(false);
    let preview = NodeRef::<html::Div>::new();
    let editor = NodeRef::<html::Textarea>::new();
    let retry = RwSignal::new(false);
    Effect::new(move || {
        if open.get() && !busy.get() {
            retry.set(created.get());
        }
    });
    let modifier = if window()
        .navigator()
        .user_agent()
        .unwrap_or_default()
        .contains("Mac")
    {
        "⌘"
    } else {
        "Ctrl"
    };
    let close = Callback::new(move |_| {
        if busy.get_untracked() {
            return;
        }
        if discard.get_untracked() {
            discard.set(false);
            mode.set("Write".into());
            request_animation_frame(move || {
                if let Some(e) = editor.get_untracked() {
                    let _ = e.focus();
                }
            });
        } else if !created.get_untracked()
            && (!body.get_untracked().is_empty() || !title.get_untracked().is_empty())
        {
            discard.set(true);
            request_animation_frame(move || {
                if let Some(button) = keep.get_untracked() {
                    let _ = button.focus();
                }
            });
        } else {
            on_close.run(());
        }
    });
    let submit = Callback::new(move |_| {
        if busy.get_untracked()
            || discard.get_untracked()
            || title.get_untracked().trim().is_empty()
        {
            return;
        }
        on_submit.run(TicketDraft {
            title: title.get_untracked().trim().into(),
            body: body.get_untracked(),
            project_id: project.get_untracked().parse().unwrap_or(project_id),
            status: Status::from_name(&status.get_untracked()),
            priority: crate::fixtures::Priority::ALL
                .into_iter()
                .find(|p| p.name() == priority.get_untracked())
                .unwrap_or(crate::fixtures::Priority::None),
            labels: selected.get_untracked(),
        });
    });
    view! {
        <Modal title="New ticket" open locked=busy on_close=close>
            <form class="ticket-composer" autocomplete="off" data-1p-ignore="true" data-form-type="other"
                on:submit=move |e| { e.prevent_default(); submit.run(()); }
                on:keydown=move |e:web_sys::KeyboardEvent| {
                    if !(e.meta_key() || e.ctrl_key()) || e.is_composing() { return; }
                    if e.key() == "Enter" { e.prevent_default(); e.stop_propagation(); submit.run(()); }
                    if e.shift_key() && e.key().eq_ignore_ascii_case("p") && !busy.get_untracked() && !discard.get_untracked() {
                        e.prevent_default(); e.stop_propagation(); shortcut.set(true); mode.update(|m| *m = if m == "Write" { "Preview" } else { "Write" }.into());
                        request_animation_frame(move||{if mode.get_untracked()=="Preview" {if let Some(e)=preview.get_untracked(){let _=e.focus();}} else if let Some(e)=editor.get_untracked(){let _=e.focus();}});
                    }
                }>
                <fieldset class="composer-fields modal-body" disabled=move || busy.get() || created.get()>
                    <TextInput id="board-title" label="Title" placeholder="What needs to be done?" value=title/>
                    <div class="composer-editor">
                        <div class="composer-editor-heading"><label for="composer-body">"Description"</label><span on:pointerdown=move|_|shortcut.set(false) title="Toggle preview (⌘⇧P / Ctrl+Shift+P)"><SegmentedControl label="Description mode" options=vec!["Write", "Preview"] value=mode instant=shortcut.into()/></span></div>
                        <div class="composer-editor-content">
                            <textarea node_ref=editor id="composer-body" aria-label="Description" hidden=move||mode.get()!="Write" autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other" placeholder="Add details…" prop:value=body on:input=move|e|body.set(event_target_value(&e))/>
                            <div node_ref=preview class="markdown-preview" role="region" aria-label="Description preview" tabindex="0" hidden=move||mode.get()!="Preview">
                                <Show when=move||!body.get().trim().is_empty() fallback=||view!{<p class="preview-placeholder">"Nothing to preview yet."</p>}>
                                    {move||view!{<Markdown source=body.get()/>}}
                                </Show>
                            </div>
                        </div>
                        <span class="composer-hint">{format!("Markdown supported · {modifier}⇧P to preview")}</span>
                    </div>
                    <div class="composer-properties">
                        <label class="field composer-project"><span>"Project"</span><SelectField label="New ticket project" value=project options=Signal::derive(move||projects.get().into_iter().map(SelectOption::project).collect()) on_change=Callback::new(move|v|project.set(v))/></label>
                        <label class="field"><span>"Status"</span><SelectField label="New ticket status" value=status options=Signal::derive(||Status::ALL.into_iter().map(SelectOption::status).collect()) on_change=Callback::new(move|v|status.set(v))/></label>
                        <label class="field"><span>"Priority"</span><SelectField label="New ticket priority" value=priority options=Signal::derive(||crate::fixtures::Priority::ALL.into_iter().map(SelectOption::priority).collect()) on_change=Callback::new(move|v|priority.set(v))/></label>
                    </div>
                    <div class="composer-labels"><Combobox label="Labels" input_id="composer-label-filter" options=Signal::derive(move||labels.get().into_iter().map(|l|SelectOption{color:l.color,..SelectOption::new(l.id,l.name)}).collect()) selected on_toggle=Callback::new(move|v:String|selected.update(|s|{if s.contains(&v){s.retain(|l|l!=&v);}else{s.push(v);}}))/>
                        {move||labels.get().into_iter().filter(|l|selected.get().contains(&l.id.to_string())).map(|l|view!{<LabelChip name=l.name color=l.color/>}).collect_view()}
                    </div>
                </fieldset>
                <Show when=move||!error.get().is_empty()><p class="composer-error" role="alert">{move||error.get()}</p></Show>
                <div class="modal-footer">
                    <Show when=move||discard.get() fallback=move||view!{
                        <button class="button button-ghost" type="button" disabled=busy on:click=move|_|close.run(())>{move||if created.get(){"Close"}else{"Cancel"}}</button>
                        <BusyButton busy disabled=Signal::derive(move||title.get().trim().is_empty())><span>{move||if retry.get(){"Retry labels"}else{"Create ticket"}}</span><Shortcut>{format!("{modifier} ↵")}</Shortcut></BusyButton>
                    }>
                        <span class="discard-prompt" role="alert">"Discard this draft?"</span>
                        <button node_ref=keep class="button" type="button" on:click=move|_|{discard.set(false);mode.set("Write".into());request_animation_frame(move||{if let Some(e)=editor.get_untracked(){let _=e.focus();}});}>"Keep editing"</button>
                        <button class="button button-danger" type="button" on:click=move|_|on_close.run(())>"Discard draft"</button>
                    </Show>
                </div>
            </form>
        </Modal>
    }
}
