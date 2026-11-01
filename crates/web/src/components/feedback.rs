use super::*;
use leptos::{html, prelude::*};
use std::sync::atomic::{AtomicU64, Ordering};
use wasm_bindgen::{JsCast, closure::Closure};

#[component]
pub fn BusyButton(
    children: Children,
    #[prop(into)] busy: Signal<bool>,
    #[prop(default=Signal::derive(||false),into)] disabled: Signal<bool>,
    #[prop(default = "primary")] variant: &'static str,
    #[prop(default = "")] class: &'static str,
    #[prop(optional, into)] aria_label: Option<String>,
    #[prop(default = "Working")] status_label: &'static str,
    #[prop(default = "submit")] kind: &'static str,
    #[prop(optional)] on_click: Option<Callback<()>>,
) -> impl IntoView {
    let content = NodeRef::<html::Span>::new();
    use_ascii_reveal(content, busy, false);
    view! {<button class=format!("button button-{variant} busy-button {class}") type=kind aria-label=aria_label aria-busy=move||busy.get().to_string() disabled=move||busy.get()||disabled.get() on:click=move|_|{if let Some(f)=on_click{f.run(());}}><span node_ref=content class="busy-content" class:busy-hidden=move||busy.get()>{children()}</span><Show when=move||busy.get()><span class="busy-indicator"><LoadingIndicator label=status_label/></span></Show></button>}
}
#[component]
pub fn ProjectIcon(
    #[prop(into)] name: String,
    #[prop(default=String::new(),into)] icon: String,
    #[prop(default=crate::colors::value("--project-default").into(),into)] color: String,
    #[prop(default = "")] size: &'static str,
) -> impl IntoView {
    let color = if tickets_core::domain::validate_color(&color).is_ok() {
        color
    } else {
        crate::colors::value("--project-default").into()
    };
    let initial = name
        .chars()
        .next()
        .unwrap_or('P')
        .to_uppercase()
        .to_string();
    let glyph = tickets_core::domain::PROJECT_GLYPHS
        .iter()
        .copied()
        .find(|g| *g == icon);
    view! {<span class=format!("project-mark {size}") style=format!("--project-color:{color};color:{color}") aria-hidden="true">{match glyph{Some(name)=>view!{<Icon name/>}.into_any(),None=>view!{<span>{if icon.is_empty(){initial}else{icon}}</span>}.into_any()}}</span>}
}
#[component]
pub fn ProjectIconPicker(
    value: RwSignal<String>,
    color: RwSignal<String>,
    name: RwSignal<String>,
) -> impl IntoView {
    let choices = Signal::derive(move || {
        std::iter::once(SelectOption::new("", "Initial"))
            .chain(
                tickets_core::domain::PROJECT_GLYPHS
                    .iter()
                    .map(|g| SelectOption {
                        icon: Some(g.to_string()),
                        color: color.get(),
                        ..SelectOption::new(g, g)
                    }),
            )
            .collect::<Vec<_>>()
    });
    view! {<div class="project-icon-picker"><label class="field"><span>"Project icon"</span><SelectField label="Project icon" options=choices value on_change=Callback::new(move|v|value.set(v))/></label>{move||view!{<ProjectIcon name=name.get() icon=value.get() color=color.get() size="large"/>}}<label class="field"><span>"Emoji (optional)"</span><input id="project-emoji" aria-label="Emoji (optional)" autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other" placeholder="Or paste an emoji" prop:value=move||{let icon=value.get();if tickets_core::domain::PROJECT_GLYPHS.contains(&icon.as_str()){String::new()}else{icon}} on:input=move|e|value.set(event_target_value(&e).trim().to_string())/></label></div>}
}

static LAST_TOOLTIP: AtomicU64 = AtomicU64::new(0);
#[component]
pub fn Tooltip(
    #[prop(into)] text: String,
    #[prop(optional)] shortcut: &'static str,
    // Navigation hints dismiss on interaction and open only for visible focus.
    // Informational tooltips also open on pointer/programmatic focus.
    #[prop(optional)] navigation_hint: bool,
    children: Children,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let active = RwSignal::new(false);
    let presence = use_presence(open.into(), EXIT_MS);
    let trigger = NodeRef::<html::Span>::new();
    let panel = NodeRef::<html::Div>::new();
    let timer = StoredValue::new(None::<TimeoutHandle>);
    let instant = RwSignal::new(false);
    let tooltip_id = format!(
        "tooltip-{}-{}",
        js_sys::Date::now() as u64,
        js_sys::Math::random()
    );

    let show = Callback::new(move |keyboard: bool| {
        active.set(navigation_hint);
        if let Some(t) = timer.get_value() {
            t.clear();
        }
        let recent = document()
            .query_selector(".tooltip[data-state=open]")
            .ok()
            .flatten()
            .is_some()
            || (js_sys::Date::now() as u64).saturating_sub(LAST_TOOLTIP.load(Ordering::Relaxed))
                < 700;
        instant.set(keyboard || recent);
        timer.set_value(
            set_timeout_with_handle(
                move || {
                    open.set(true);
                    LAST_TOOLTIP.store(js_sys::Date::now() as u64, Ordering::Relaxed);
                },
                std::time::Duration::from_millis(if keyboard || recent { 0 } else { 500 }),
            )
            .ok(),
        );
    });
    let hide = Callback::new(move |_| {
        active.set(false);
        if let Some(t) = timer.get_value() {
            t.clear();
        }
        if open.get_untracked() {
            LAST_TOOLTIP.store(js_sys::Date::now() as u64, Ordering::Relaxed);
        }
        open.set(false);
        // Dismiss immediately so a fading hint cannot remain over a moved row.
        if let Some(p) = panel.get_untracked() {
            hide_popup(&p);
        }
    });
    Effect::new(move |_| {
        if let (Some(t), Some(p)) = (trigger.get(), panel.get()) {
            if let Some(control) = t.first_element_child() {
                if open.get() {
                    let _ = control.set_attribute("aria-describedby", &p.id());
                } else {
                    let _ = control.remove_attribute("aria-describedby");
                }
            }
            if presence.mounted.get() && open.get() {
                position_popup(&t, &p);
            } else {
                hide_popup(&p);
            }
        }
    });
    let dismiss = StoredValue::new_local(Closure::wrap(
        Box::new(move || hide.run(())) as Box<dyn Fn()>
    ));
    Effect::new(move |_| {
        // Listen globally only while hovered/focused, including the pending delay.
        if active.get() {
            let cleanup =
                dismiss.with_value(|dismiss| watch_tooltip(dismiss.as_ref().unchecked_ref()));
            on_cleanup(move || {
                let _ = cleanup.call0(&wasm_bindgen::JsValue::NULL);
            });
        } else if !navigation_hint
            && presence.mounted.get()
            && let (Some(t), Some(p)) = (trigger.get(), panel.get())
        {
            let cleanup = watch_popup(&t, &p);
            on_cleanup(move || {
                let _ = cleanup.call0(&wasm_bindgen::JsValue::NULL);
            });
        }
    });
    on_cleanup(move || {
        if let Some(t) = timer.get_value() {
            t.clear();
        }
    });
    view! {<span class="tooltip-trigger" node_ref=trigger on:pointerenter=move|e:web_sys::PointerEvent|{if e.pointer_type()!="touch" && open.try_get_untracked().is_some(){show.run(false);}} on:pointerleave=move|_|{if open.try_get_untracked().is_some(){hide.run(());}} on:focusin=move|e:web_sys::FocusEvent|{if open.try_get_untracked().is_some() && (!navigation_hint || e.target().and_then(|t|t.dyn_into::<web_sys::Element>().ok()).is_some_and(|t|t.matches(":focus-visible").unwrap_or(false))){show.run(true);}} on:focusout=move|_|{if open.try_get_untracked().is_some(){hide.run(());}} on:keydown=move|e:web_sys::KeyboardEvent|{if e.key()=="Escape"{hide.run(());}}>{children()}<div node_ref=panel id=tooltip_id class="popup tooltip" data-instant=move||instant.get().to_string() role="tooltip" popover="manual" data-state=move||presence.state() hidden=move||!open.get()>{text}{(!shortcut.is_empty()).then(||view!{<span>" "<Shortcut>{shortcut}</Shortcut></span>})}</div></span>}
}

/// Equal-width segments, roving tab stop, immediate keyboard selection, transform indicator.
#[component]
pub fn SegmentedControl(
    label: &'static str,
    options: Vec<&'static str>,
    value: RwSignal<String>,
    #[prop(default=Signal::derive(||false))] instant: Signal<bool>,
) -> impl IntoView {
    let choices = StoredValue::new(options.clone());
    let count = options.len();
    let keyboard = RwSignal::new(false);
    view! {<div class="segmented-control" role="group" aria-label=label style=format!("--segments:{count}") data-keyboard=move||(keyboard.get() || instant.get()).to_string()>
    <span class="segment-indicator" aria-hidden="true" style=move||format!("transform:translateX({}%)",choices.with_value(|opts|opts.iter().position(|o|*o==value.get()).unwrap_or(0))*100)/>
    {options.into_iter().map(|option|view!{<button type="button" aria-pressed=move||(value.get()==option).to_string() tabindex=move||if value.get()==option{0}else{-1} on:click=move|_|{keyboard.set(false);value.set(option.into());} on:keydown=move|e:web_sys::KeyboardEvent|{
        let key=e.key();if !["ArrowLeft","ArrowRight","Home","End"].contains(&key.as_str()){return;}e.prevent_default();keyboard.set(true);let opts=choices.get_value();let i=opts.iter().position(|o|*o==value.get_untracked()).unwrap_or(0);let next=match key.as_str(){"Home"=>0,"End"=>count-1,"ArrowLeft"=>(i+count-1)%count,_=>(i+1)%count};value.set(opts[next].into());
        use web_sys::wasm_bindgen::JsCast;if let Some(target)=e.target().and_then(|n|n.dyn_into::<web_sys::Element>().ok()).and_then(|n|n.parent_element()).and_then(|n|n.query_selector_all("button").ok()).and_then(|n|n.item(next as u32)).and_then(|n|n.dyn_into::<web_sys::HtmlElement>().ok()){let _=target.focus();}
    }>{option}</button>}).collect_view()}</div>}
}

/// Text carries the state alongside the existing icon family; color is supplemental.
#[component]
pub fn PrCheckStatus(
    #[prop(optional_no_strip)] conclusion: Option<String>,
    #[prop(default = "CI")] label: &'static str,
) -> impl IntoView {
    let (tone, icon, state) = match conclusion.as_deref() {
        Some("success") => ("passed", "check", "Passed"),
        Some("failure" | "timed_out" | "action_required" | "startup_failure") => {
            ("failed", "close", "Failed")
        }
        Some("cancelled") => ("unknown", "more", "Canceled"),
        Some("neutral") => ("unknown", "more", "Neutral"),
        Some("skipped") => ("unknown", "more", "Skipped"),
        Some("stale") => ("unknown", "more", "Stale"),
        Some("queued" | "in_progress" | "pending") => ("pending", "clock", "Pending"),
        _ => ("unknown", "more", "Not reported"),
    };
    view! {<span class=format!("pr-check-status {tone}")><span aria-hidden="true"><Icon name=icon/></span>{format!("{label}: {state}")}</span>}
}

/// The same compact rows are used by live ticket properties and fixture previews.
#[component]
pub(crate) fn LinkedPullRequests(requests: Vec<crate::pages::board::PullRequest>) -> impl IntoView {
    (!requests.is_empty()).then(|| view! {
        <section class="property-section linked-pull-requests" aria-label="Pull requests">
            <h2>"Pull requests"</h2>
            <ul class="pr-list">{requests.into_iter().map(|pr| {
                let state = match pr.state.as_str() {
                    "open" => "Open", "draft" => "Draft", "merged" => "Merged", _ => "Closed",
                };
                let ci = match pr.ci_status.as_deref() {
                    Some("success") => "Passing",
                    Some("failure" | "error" | "timed_out" | "action_required" | "startup_failure") => "Failing",
                    Some("queued" | "in_progress" | "pending") => "Pending",
                    _ => "None",
                };
                view! {
                    <li class="pr-sidebar-row" title=format!("{} · Tickets check: {}", if pr.closes {"Closes ticket"} else {"Reference"}, pr.tickets_check.as_deref().unwrap_or("Not reported"))>
                        <a class="pr-sidebar-link" href=pr.url target="_blank" rel="noopener noreferrer"
                            aria-label=format!("{}#{}: {} (opens in new tab)", pr.repo, pr.number, pr.title)>
                            <span class="pr-repository"><Icon name="github"/><span title=format!("{}#{}", pr.repo, pr.number)>{format!("{}#{}", pr.repo, pr.number)}</span><Icon name="external"/></span>
                            <span class="pr-title" title=pr.title.clone()>{pr.title.clone()}</span>
                        </a>
                        <div class="pr-sidebar-status"><span class=format!("label-chip pr-state pr-state-{}", pr.state)><span>{state}</span></span><span class="pr-ci" title=pr.ci_status.unwrap_or_else(||"No CI reported for this head".into())>{format!("CI: {ci}")}</span></div>
                    </li>
                }
            }).collect_view()}</ul>
        </section>
    })
}
