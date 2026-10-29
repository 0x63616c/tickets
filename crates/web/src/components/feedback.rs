use super::*;
use leptos::{html, prelude::*};
use std::sync::atomic::{AtomicU64, Ordering};

// cli-spinners, MIT, Sindre Sorhus. Exact vendored frames; see THIRD_PARTY.md.
pub const DOTS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
pub const LINE: &[&str] = &["-", "\\", "|", "/"];
pub const PULSE: &[&str] = &["⠁", "⠂", "⠄", "⡀", "⢀", "⠠", "⠐", "⠈"];
#[derive(Clone, Copy, Default)]
pub enum SpinnerStyle {
    #[default]
    Dots,
    Line,
    Pulse,
}
#[component]
pub fn AsciiSpinner(
    #[prop(default = "Loading")] label: &'static str,
    #[prop(optional)] style: SpinnerStyle,
    #[prop(default = 150)] delay: u64,
) -> impl IntoView {
    let visible = RwSignal::new(false);
    let frame = RwSignal::new(0usize);
    let (frames, ms) = match style {
        SpinnerStyle::Dots => (DOTS, 80),
        SpinnerStyle::Line => (LINE, 130),
        SpinnerStyle::Pulse => (PULSE, 100),
    };
    let timeout = set_timeout_with_handle(
        move || visible.set(true),
        std::time::Duration::from_millis(delay),
    )
    .ok();
    let interval = set_interval_with_handle(
        move || {
            if document().hidden()
                || window()
                    .match_media("(prefers-reduced-motion: reduce)")
                    .ok()
                    .flatten()
                    .is_some_and(|q| q.matches())
            {
                return;
            }
            frame.update(|n| *n = (*n + 1) % frames.len());
        },
        std::time::Duration::from_millis(ms),
    )
    .ok();
    on_cleanup(move || {
        if let Some(t) = timeout {
            t.clear();
        }
        if let Some(i) = interval {
            i.clear();
        }
    });
    view! {<span class="ascii-spinner" role="status" aria-live="polite" aria-atomic="true" class:spinner-visible=move||visible.get()><span aria-hidden="true" class="spinner-frames">{move||frames[frame.get()]}</span><span aria-hidden="true" class="spinner-static">{frames[0]}</span><span class="sr-only">{move||visible.get().then_some(label)}</span></span>}
}
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
    view! {<button class=format!("button button-{variant} busy-button {class}") type=kind aria-label=aria_label aria-busy=move||busy.get().to_string() disabled=move||busy.get()||disabled.get() on:click=move|_|{if let Some(f)=on_click{f.run(());}}><span class="busy-content" class:busy-hidden=move||busy.get()>{children()}</span><Show when=move||busy.get()><span class="busy-indicator"><AsciiSpinner label=status_label/></span></Show></button>}
}
#[component]
pub fn Skeleton(#[prop(default = "line")] shape: &'static str) -> impl IntoView {
    view! {<span class=format!("skeleton skeleton-{shape}") aria-hidden="true"/>}
}
#[component]
pub fn LoadingSkeleton(#[prop(default = "board")] layout: &'static str) -> impl IntoView {
    view! {<div class=format!("loading-skeleton loading-{layout}") aria-busy="true"><div class="loading-caption"><AsciiSpinner label="Loading content"/><span>"Loading…"</span></div><div class="skeleton-layout" aria-hidden="true">{match layout {
        "board"=>(0..4).map(|_|view!{<div class="skeleton-column"><Skeleton shape="heading"/>{(0..3).map(|_|view!{<div class="skeleton-card"><Skeleton shape="meta"/><Skeleton/><Skeleton shape="short"/><Skeleton shape="meta"/></div>}).collect_view()}</div>}).collect_view().into_any(),
        "detail"=>view!{<div class="skeleton-detail"><div><Skeleton shape="heading"/><Skeleton/><Skeleton/><Skeleton shape="short"/><div class="skeleton-section"><Skeleton shape="heading"/><Skeleton/><Skeleton/></div></div><aside>{(0..6).map(|_|view!{<Skeleton/>}).collect_view()}</aside></div>}.into_any(),
        _=>(0..7).map(|_|view!{<div class="skeleton-row"><Skeleton shape="square"/><Skeleton shape="meta"/><Skeleton/><Skeleton shape="short"/></div>}).collect_view().into_any(),
    }}</div></div>}
}

#[component]
pub fn ProjectIcon(
    #[prop(into)] name: String,
    #[prop(default=String::new(),into)] icon: String,
    #[prop(default="#5e6ad2".into(),into)] color: String,
    #[prop(default = "")] size: &'static str,
) -> impl IntoView {
    let color = if tickets_core::domain::validate_color(&color).is_ok() {
        color
    } else {
        "#5e6ad2".into()
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
pub fn Tooltip(#[prop(into)] text: String, children: Children) -> impl IntoView {
    let open = RwSignal::new(false);
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
        if let Some(t) = timer.get_value() {
            t.clear();
        }
        let recent =
            (js_sys::Date::now() as u64).saturating_sub(LAST_TOOLTIP.load(Ordering::Relaxed)) < 700;
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
        if let Some(t) = timer.get_value() {
            t.clear();
        }
        if open.get_untracked() {
            LAST_TOOLTIP.store(js_sys::Date::now() as u64, Ordering::Relaxed);
        }
        open.set(false);
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
            if presence.mounted.get() {
                position_popup(&t, &p);
            } else {
                hide_popup(&p);
            }
        }
    });
    Effect::new(move |_| {
        if presence.mounted.get()
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
    view! {<span class="tooltip-trigger" node_ref=trigger on:mouseenter=move|_|{if open.try_get_untracked().is_some(){show.run(false);}} on:mouseleave=move|_|{if open.try_get_untracked().is_some(){hide.run(());}} on:focusin=move|_|{if open.try_get_untracked().is_some(){show.run(true);}} on:focusout=move|_|{if open.try_get_untracked().is_some(){hide.run(());}} on:keydown=move|e:web_sys::KeyboardEvent|{if e.key()=="Escape"{hide.run(());}}>{children()}<div node_ref=panel id=tooltip_id class="popup tooltip" data-instant=move||instant.get().to_string() role="tooltip" popover="manual" data-state=move||presence.state() hidden=move||!presence.mounted.get()>{text}</div></span>}
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
