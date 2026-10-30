//! Whole-surface navigation leaves nested controls and links in charge of their events.
use leptos::prelude::*;
use tickets_core::domain::Label;
use web_sys::wasm_bindgen::JsCast;

pub(crate) fn ticket_interactive_target(target: Option<web_sys::EventTarget>) -> bool {
    target.and_then(|t| t.dyn_into::<web_sys::Element>().ok()).is_some_and(|el| {
        el.closest("a, button, input, select, textarea, [role=button], [role=combobox], [role=option], [role=menuitem], [contenteditable=true]")
            .ok().flatten().is_some()
    })
}

#[component]
pub fn CardLabels(labels: Vec<Label>) -> impl IntoView {
    let labels = labels
        .into_iter()
        .filter(|l| super::visible_label(&l.name))
        .collect::<Vec<_>>();
    let full = labels
        .iter()
        .map(|l| l.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let short = labels
        .iter()
        .take(3)
        .map(|l| {
            l.name
                .split_once(':')
                .map_or(l.name.as_str(), |(_, short)| short)
        })
        .collect::<Vec<_>>()
        .join(", ");
    view! {<span class="card-label-summary" title=full.clone() aria-label=full.clone()>
        <span class="card-label-dots" aria-hidden="true">{labels.into_iter().map(|l|view!{<span class="card-label-dot" style=format!("background:{}",l.color)/>}).collect_view()}</span>
        <span class="card-label-names">{short}</span>
    </span>}
}
