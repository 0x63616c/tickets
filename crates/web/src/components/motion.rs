//! Presence retains the rendered value until its exit finishes. Reopening cancels exit.
use leptos::prelude::*;
use wasm_bindgen::prelude::*;

// Make Cargo track the local bridge when only JavaScript changes.
const _: &str = include_str!("dom.js");
const _: &str = include_str!("dom_drag_c97c58dae072cc25.js");

#[wasm_bindgen(module = "/src/components/dom.js")]
extern "C" {
    pub fn position_popup(trigger: &web_sys::Element, panel: &web_sys::Element);
    pub fn watch_popup(trigger: &web_sys::Element, panel: &web_sys::Element) -> js_sys::Function;
    pub fn watch_tooltip(dismiss: &js_sys::Function) -> js_sys::Function;
    pub fn hide_popup(panel: &web_sys::Element);
}

pub const EXIT_MS: u64 = 140;

#[derive(Clone, Copy)]
pub struct Presence {
    pub mounted: RwSignal<bool>,
    pub open: Signal<bool>,
}
impl Presence {
    pub fn state(self) -> &'static str {
        if self.open.get() { "open" } else { "closed" }
    }
}

pub fn use_presence(open: Signal<bool>, exit_ms: u64) -> Presence {
    let mounted = RwSignal::new(open.get_untracked());
    let timer = StoredValue::new(None::<TimeoutHandle>);
    Effect::new(move |_| {
        if let Some(handle) = timer.get_value() {
            handle.clear();
        }
        if open.get() {
            mounted.set(true);
        } else if mounted.get_untracked() {
            timer.set_value(
                set_timeout_with_handle(
                    move || {
                        mounted.set(false);
                    },
                    std::time::Duration::from_millis(exit_ms),
                )
                .ok(),
            );
        }
    });
    on_cleanup(move || {
        if let Some(handle) = timer.get_value() {
            handle.clear();
        }
    });
    Presence { mounted, open }
}

/// Keeps an Option's last Some value during exit; use the source to drive data-state.
pub fn present_value<T: Clone + Send + Sync + 'static>(
    source: Signal<Option<T>>,
) -> Signal<Option<T>> {
    let kept = RwSignal::new(source.get_untracked());
    let presence = use_presence(Signal::derive(move || source.get().is_some()), EXIT_MS);
    Effect::new(move |_| {
        if let Some(value) = source.get() {
            kept.set(Some(value));
        } else if !presence.mounted.get() {
            kept.set(None);
        }
    });
    kept.into()
}

/// Wrap conditional content, including dialogs. Children receive the presence context.
#[component]
pub fn Presence(#[prop(into)] show: Signal<bool>, children: ChildrenFn) -> impl IntoView {
    let presence = use_presence(show, EXIT_MS);
    view! { <Show when=move || presence.mounted.get()>{let children=children.clone(); move || {
        provide_context(presence);
        children()
    }}</Show> }
}

// Keep the legacy popup snippet unchanged; motion coordinates with board drag.
#[wasm_bindgen(module = "/src/components/dom_drag_c97c58dae072cc25.js")]
extern "C" {
    pub fn install_motion() -> js_sys::Function;
}
