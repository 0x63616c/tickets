//! Shared top-layer surfaces. Controlled values stay authoritative during failed writes.
use super::*;
use leptos::{ev, html, prelude::*};
use std::sync::atomic::{AtomicUsize, Ordering};
use web_sys::wasm_bindgen::JsCast;
static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
fn id(prefix: &str) -> String {
    format!("{prefix}-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed))
}

#[derive(Clone, Debug, PartialEq)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
    pub disabled: bool,
    pub icon: Option<String>,
    pub color: String,
    pub status: Option<Status>,
    pub priority: Option<crate::fixtures::Priority>,
}
impl SelectOption {
    pub fn new(value: impl ToString, label: impl ToString) -> Self {
        Self {
            value: value.to_string(),
            label: label.to_string(),
            disabled: false,
            icon: None,
            color: String::new(),
            status: None,
            priority: None,
        }
    }
    pub fn status(status: Status) -> Self {
        Self {
            status: Some(status),
            ..Self::new(status.name(), status.name())
        }
    }
    pub fn priority(priority: crate::fixtures::Priority) -> Self {
        Self {
            priority: Some(priority),
            ..Self::new(priority.name(), priority.name())
        }
    }
    pub fn project(p: tickets_core::domain::Project) -> Self {
        Self {
            value: p.id.to_string(),
            label: p.name,
            disabled: false,
            icon: Some(p.icon.unwrap_or_default()),
            color: p.color,
            status: None,
            priority: None,
        }
    }
}
pub fn options(names: &[&str]) -> Vec<SelectOption> {
    names.iter().map(|s| SelectOption::new(s, s)).collect()
}

#[derive(Clone, Copy)]
struct Popup {
    open: RwSignal<bool>,
    presence: super::motion::Presence,
    trigger: NodeRef<html::Button>,
    panel: NodeRef<html::Div>,
}
impl Popup {
    fn new() -> Self {
        let open = RwSignal::new(false);
        let this = Self {
            open,
            presence: use_presence(open.into(), EXIT_MS),
            trigger: NodeRef::new(),
            panel: NodeRef::new(),
        };
        Effect::new(move |_| {
            let visible = this.presence.mounted.get();
            if let (Some(trigger), Some(panel)) = (this.trigger.get(), this.panel.get()) {
                if visible {
                    position_popup(&trigger, &panel);
                } else {
                    hide_popup(&panel);
                }
            }
        });
        let outside = window_event_listener(ev::pointerdown, move |e| {
            if !open.get_untracked() {
                return;
            }
            if let Some(target) = e.target().and_then(|t| t.dyn_into::<web_sys::Node>().ok()) {
                let inside = this
                    .trigger
                    .get_untracked()
                    .is_some_and(|n| n.contains(Some(&target)))
                    || this
                        .panel
                        .get_untracked()
                        .is_some_and(|n| n.contains(Some(&target)));
                if !inside {
                    open.set(false);
                }
            }
        });
        Effect::new(move |_| {
            if this.presence.mounted.get()
                && let (Some(trigger), Some(panel)) = (this.trigger.get(), this.panel.get())
            {
                let cleanup = watch_popup(&trigger, &panel);
                on_cleanup(move || {
                    let _ = cleanup.call0(&wasm_bindgen::JsValue::NULL);
                });
            }
        });
        on_cleanup(move || outside.remove());
        this
    }
    fn position(self) {
        if self.open.get_untracked()
            && let (Some(t), Some(p)) = (self.trigger.get_untracked(), self.panel.get_untracked())
        {
            position_popup(&t, &p);
        }
    }
    fn close(self, restore: bool) {
        self.open.set(false);
        if restore && let Some(t) = self.trigger.get_untracked() {
            let _ = t.focus();
        }
    }
    fn focus(self, edge: Option<bool>) {
        request_animation_frame(move || {
            if self.open.try_get_untracked() != Some(true) {
                return;
            }
            self.position();
            if let Some(panel) = self.panel.get_untracked()
                && let Ok(items) = panel.query_selector_all(
                    "[role=option]:not(:disabled), [role^=menuitem]:not(:disabled), input",
                )
            {
                let selected = panel.query_selector("[aria-selected=true]").ok().flatten();
                let node = panel
                    .query_selector("input")
                    .ok()
                    .flatten()
                    .or_else(|| if edge.is_none() { selected } else { None })
                    .or_else(|| {
                        items
                            .item(if edge == Some(true) {
                                items.length().saturating_sub(1)
                            } else {
                                0
                            })
                            .and_then(|n| n.dyn_into().ok())
                    });
                if let Some(node) = node.and_then(|n| n.dyn_into::<web_sys::HtmlElement>().ok()) {
                    let _ = node.focus();
                }
            }
        });
    }
}

/// Roving focus/typeahead shared by listboxes and menus. Inputs retain editing keys.
fn popup_key(
    e: web_sys::KeyboardEvent,
    popup: Popup,
    search: RwSignal<String>,
    stamp: RwSignal<f64>,
) {
    let key = e.key();
    if key == "Escape" {
        e.prevent_default();
        e.stop_propagation();
        popup.close(true);
        return;
    }
    if key == "Tab" {
        popup.close(true);
        return;
    }
    let Some(panel) = popup.panel.get_untracked() else {
        return;
    };
    let Ok(items) =
        panel.query_selector_all("[role=option]:not(:disabled), [role^=menuitem]:not(:disabled)")
    else {
        return;
    };
    let nodes = (0..items.length())
        .filter_map(|i| items.item(i)?.dyn_into::<web_sys::HtmlElement>().ok())
        .collect::<Vec<_>>();
    if nodes.is_empty() {
        return;
    }
    let active = document().active_element();
    let current = nodes
        .iter()
        .position(|n| active.as_ref().is_some_and(|a| n.is_same_node(Some(a))));
    let input = e
        .target()
        .is_some_and(|t| t.dyn_ref::<web_sys::HtmlInputElement>().is_some());
    if !input
        && matches!(key.as_str(), "Enter" | " ")
        && let Some(i) = current
    {
        e.prevent_default();
        e.stop_propagation();
        nodes[i].click();
        return;
    }
    if key == "ArrowUp"
        && current == Some(0)
        && let Ok(Some(input)) = panel.query_selector("input")
        && let Some(input) = input.dyn_ref::<web_sys::HtmlElement>()
    {
        e.prevent_default();
        e.stop_propagation();
        let _ = input.focus();
        return;
    }
    let index = match key.as_str() {
        "ArrowDown" => Some(current.map(|i| (i + 1) % nodes.len()).unwrap_or(0)),
        "ArrowUp" => Some(
            current
                .map(|i| (i + nodes.len() - 1) % nodes.len())
                .unwrap_or(nodes.len() - 1),
        ),
        "Home" if !input => Some(0),
        "End" if !input => Some(nodes.len() - 1),
        _ if !input
            && key.chars().count() == 1
            && !e.ctrl_key()
            && !e.meta_key()
            && !e.alt_key() =>
        {
            let now = js_sys::Date::now();
            if now - stamp.get_untracked() > 700. {
                search.set(String::new());
            }
            stamp.set(now);
            search.update(|s| s.push_str(&key.to_lowercase()));
            let query = search.get_untracked();
            let query = if query.chars().all(|c| query.starts_with(c)) {
                key.to_lowercase()
            } else {
                query
            };
            (1..=nodes.len())
                .map(|n| (current.unwrap_or(nodes.len() - 1) + n) % nodes.len())
                .find(|i| {
                    nodes[*i]
                        .text_content()
                        .unwrap_or_default()
                        .trim()
                        .to_lowercase()
                        .starts_with(&query)
                })
        }
        _ => None,
    };
    if let Some(index) = index {
        e.prevent_default();
        e.stop_propagation();
        let _ = nodes[index].focus();
        nodes[index].scroll_into_view_with_bool(false);
    }
}

#[component]
pub fn SelectField(
    #[prop(into)] label: String,
    #[prop(into)] options: Signal<Vec<SelectOption>>,
    #[prop(into)] value: Signal<String>,
    on_change: Callback<String>,
    #[prop(default=Signal::derive(||false),into)] disabled: Signal<bool>,
    #[prop(optional, into)] described_by: Option<String>,
) -> impl IntoView {
    let popup = Popup::new();
    let panel_id = id("select");
    let aria_id = panel_id.clone();
    let search = RwSignal::new(String::new());
    let stamp = RwSignal::new(0.);
    let fallback = label.clone();
    let list_label = label.clone();
    view! { <span class="select-root">
        <button type="button" node_ref=popup.trigger class="select-trigger" role="combobox" aria-label=label aria-describedby=described_by aria-haspopup="listbox" aria-controls=aria_id aria-expanded=move||popup.open.get().to_string() disabled=disabled prop:value=move||value.get()
            on:click=move|e|{e.stop_propagation();popup.open.update(|v|*v= !*v);if popup.open.get_untracked(){popup.focus(None);}}
            on:keydown=move|e:web_sys::KeyboardEvent|{if ["Enter"," "].contains(&e.key().as_str()){e.prevent_default();e.stop_propagation();popup.open.update(|v|*v= !*v);if popup.open.get_untracked(){popup.focus(None);}}else if ["ArrowDown","ArrowUp","Home","End"].contains(&e.key().as_str()){e.prevent_default();e.stop_propagation();popup.open.set(true);popup.focus(match e.key().as_str(){"Home"=>Some(false),"End"|"ArrowUp"=>Some(true),_=>None});}else if e.key().chars().count()==1 && e.key()!=" " && !e.ctrl_key() && !e.meta_key(){e.prevent_default();e.stop_propagation();popup.open.set(true);let key=e.key().to_lowercase();request_animation_frame(move||{if popup.open.try_get_untracked()!=Some(true){return;}
                if let Some(panel)=popup.panel.get_untracked() && let Ok(items)=panel.query_selector_all("[role=option]:not(:disabled)"){for i in 0..items.length(){if let Some(node)=items.item(i).and_then(|n|n.dyn_into::<web_sys::HtmlElement>().ok()) && node.text_content().unwrap_or_default().trim().to_lowercase().starts_with(&key){let _=node.focus();break;}}}});}}>
            <span class="select-value">{move||{let selected=options.get().into_iter().find(|o|o.value==value.get());match selected {Some(o)=>view!{<>{o.status.map(|status|view!{<StatusIcon status/>})}{o.priority.map(|priority|view!{<PriorityIcon priority/>})}{o.icon.map(|icon|view!{<ProjectIcon name=o.label.clone() icon color=o.color/>})}<span>{o.label}</span></>}.into_any(),None=>view!{<span>{fallback.clone()}</span>}.into_any()}}}</span><Icon name="down"/>
        </button>
        <div node_ref=popup.panel id=panel_id class="popup select-popup" popover="manual" role="listbox" aria-label=list_label data-state=move||popup.presence.state() hidden=move||!popup.presence.mounted.get() inert=move||!popup.open.get()
            on:keydown=move|e|popup_key(e,popup,search,stamp)>
            {move||options.get().into_iter().map(|option|{let v=option.value.clone();let selected=option.value.clone();view!{
                <button type="button" class="menu-item select-option" role="option" tabindex="-1" data-value=v.clone() aria-selected=move||(value.get()==selected).to_string() disabled=option.disabled on:click=move|e|{e.stop_propagation();popup.close(true);on_change.run(v.clone());}>
                {option.status.map(|status|view!{<StatusIcon status/>})}{option.priority.map(|priority|view!{<PriorityIcon priority/>})}{option.icon.map(|icon|view!{<ProjectIcon name=option.label.clone() icon color=option.color/>})}<span>{option.label}</span><span class="option-check" aria-hidden="true"><Icon name="check"/></span></button>
            }}).collect_view()}
        </div>
    </span> }
}

#[component]
pub fn Select(
    label: &'static str,
    options: Vec<&'static str>,
    value: RwSignal<String>,
    #[prop(optional)] on_change: Option<Callback<String>>,
) -> impl IntoView {
    let choices = super::popup::options(&options);
    view! {<SelectField label options=Signal::derive(move||choices.clone()) value on_change=Callback::new(move|next:String|{value.set(next.clone());if let Some(f)=on_change{f.run(next);}})/>}
}

#[derive(Clone, Copy)]
struct MenuClose(Callback<()>);
#[component]
pub fn DropdownMenu(
    #[prop(default = "label-menu")] class: &'static str,
    #[prop(default = "button button-ghost menu-trigger")] trigger_class: &'static str,
    #[prop(default = "plus")] icon: &'static str,
    #[prop(default = false)] disclosure: bool,
    label: &'static str,
    children: Children,
    #[prop(optional)] trigger_content: Option<Children>,
    #[prop(optional)] on_input_enter: Option<Callback<()>>,
) -> impl IntoView {
    let popup = Popup::new();
    let panel_id = id("menu");
    let aria_id = panel_id.clone();
    provide_context(MenuClose(Callback::new(move |_| popup.close(true))));
    let search = RwSignal::new(String::new());
    let stamp = RwSignal::new(0.);
    view! {<div class=class><button type="button" class=trigger_class node_ref=popup.trigger aria-label=label aria-haspopup="menu" aria-expanded=move||popup.open.get().to_string() aria-controls=aria_id on:click=move|_|{popup.open.update(|v|*v= !*v);if popup.open.get_untracked(){popup.focus(None);}} on:keydown=move|e:web_sys::KeyboardEvent|{if ["Enter"," "].contains(&e.key().as_str()){e.prevent_default();popup.open.update(|v|*v= !*v);if popup.open.get_untracked(){popup.focus(None);}}else if ["ArrowDown","ArrowUp"].contains(&e.key().as_str()){e.prevent_default();popup.open.set(true);popup.focus(Some(e.key()=="ArrowUp"));}}>{match trigger_content {Some(content)=>content().into_any(),None=>view!{<><Icon name=icon/><span>{label}</span></>}.into_any()}}{disclosure.then(||view!{<Icon name="down"/>})}</button>
    <div class="popup menu" node_ref=popup.panel id=panel_id role="menu" aria-label=label popover="manual" hidden=move||!popup.presence.mounted.get() inert=move||!popup.open.get() data-state=move||popup.presence.state() on:keydown=move|e:web_sys::KeyboardEvent|{if e.key()=="Enter" && e.target().is_some_and(|t|t.dyn_ref::<web_sys::HtmlInputElement>().is_some()) && let Some(cb)=on_input_enter {e.prevent_default();cb.run(());}else{popup_key(e,popup,search,stamp);}}>{children()}</div></div>}
}
#[component]
pub fn MenuItem(
    children: Children,
    on_select: Callback<()>,
    #[prop(optional)] checked: Option<Signal<bool>>,
    #[prop(optional)] shortcut: Option<&'static str>,
) -> impl IntoView {
    let close = expect_context::<MenuClose>();
    view! {<button type="button" class="menu-item" role=if checked.is_some(){"menuitemcheckbox"}else{"menuitem"} aria-checked=move||checked.map(|s|s.get().to_string()) tabindex="-1" on:click=move|_|{on_select.run(());close.0.run(());}>
    {children()}{shortcut.map(|s|view!{<Shortcut>{s}</Shortcut>})}{move||checked.is_some_and(|s|s.get()).then(||view!{<Icon name="check"/>})}</button>}
}
#[component]
pub fn MenuSeparator() -> impl IntoView {
    view! {<div class="menu-separator" role="separator"/>}
}

/// Searchable multi-select. Callers own values, saving and optional label creation.
#[component]
pub fn Combobox(
    #[prop(into)] label: String,
    #[prop(optional, into)] input_id: Option<String>,
    #[prop(into)] options: Signal<Vec<SelectOption>>,
    #[prop(into)] selected: Signal<Vec<String>>,
    on_toggle: Callback<String>,
    #[prop(optional)] on_create: Option<Callback<String>>,
    #[prop(default=Signal::derive(||false),into)] busy: Signal<bool>,
) -> impl IntoView {
    let popup = Popup::new();
    let query = RwSignal::new(String::new());
    let list = id("combobox");
    let controls = list.clone();
    let dialog_id = format!("{list}-dialog");
    let dialog_controls = dialog_id.clone();
    let input_id = input_id.unwrap_or_else(|| format!("{list}-input"));
    let search = RwSignal::new(String::new());
    let stamp = RwSignal::new(0.);
    let label2 = label.clone();
    let reveal_root = NodeRef::<leptos::html::Div>::new();
    use_ascii_reveal(reveal_root, Signal::derive(move || busy.get()), false);
    view! {<div class="combobox-root label-menu"><button node_ref=popup.trigger type="button" class="button button-ghost menu-trigger" aria-haspopup="dialog" aria-controls=dialog_controls aria-expanded=move||popup.open.get().to_string() on:click=move|_|{popup.open.update(|v|*v= !*v);if popup.open.get_untracked(){popup.focus(None);}} on:keydown=move|e:web_sys::KeyboardEvent|{if ["ArrowDown","ArrowUp"].contains(&e.key().as_str()){e.prevent_default();e.stop_propagation();popup.open.set(true);popup.focus(None);}}><Icon name="plus"/>{label}</button>
    <div node_ref=popup.panel id=dialog_id class="popup combobox-popup" popover="manual" role="dialog" aria-label=label2 data-state=move||popup.presence.state() hidden=move||!popup.presence.mounted.get() inert=move||!popup.open.get() on:keydown=move|e:web_sys::KeyboardEvent|{
        if busy.get_untracked() && e.key()!="Escape" && e.key()!="Tab" {return;}
        if e.key()=="Enter" && e.target().is_some_and(|t|t.dyn_ref::<web_sys::HtmlInputElement>().is_some()) { e.prevent_default();let q=query.get_untracked().trim().to_string();if !q.is_empty(){if let Some(create)=on_create{create.run(q);}else if let Some(o)=options.get_untracked().iter().find(|o|o.label.eq_ignore_ascii_case(&q)){on_toggle.run(o.value.clone());}} }else{popup_key(e,popup,search,stamp);}
    }><div class="combobox-search"><Show when=move||busy.get() fallback=||view!{<Icon name="search"/>}><LoadingIndicator label="Saving labels"/></Show><input id=input_id role="combobox" aria-label="Filter or create label" placeholder="Filter labels…" aria-autocomplete="list" aria-expanded="true" aria-controls=controls autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other" prop:value=query on:input=move|e|query.set(event_target_value(&e))/></div>
    <div node_ref=reveal_root id=list role="listbox" aria-label="Labels" aria-multiselectable="true" aria-busy=move||busy.get().to_string()>{move||{let q=query.get().trim().to_lowercase();let choices=options.get();let filtered=choices.iter().filter(|o|o.label.to_lowercase().contains(&q)).cloned().collect::<Vec<_>>();let exact=choices.iter().any(|o|o.label.to_lowercase()==q);view!{
        {filtered.iter().cloned().map(|o|{let v=o.value.clone();let checked=o.value.clone();view!{<button class="menu-item" type="button" role="option" tabindex="-1" data-value=o.value aria-selected=move||selected.get().contains(&checked).to_string() disabled=move||busy.get() || o.disabled on:click=move|_|on_toggle.run(v.clone())><LabelChip name=o.label color=o.color/><span class="option-check"><Icon name="check"/></span></button>}}).collect_view()}
        {(filtered.is_empty() && (q.is_empty() || on_create.is_none())).then(||view!{<p class="menu-empty">"No labels found."</p>})}
        {(!q.is_empty() && !exact).then(||on_create.map(|create|view!{<button type="button" role="option" aria-selected="false" tabindex="-1" class="menu-item" data-create="true" disabled=busy on:click=move|_|create.run(query.get_untracked().trim().to_string())><Icon name="plus"/>{format!("Create label {}",query.get())}</button>}))}
    }}}</div></div></div>}
}
