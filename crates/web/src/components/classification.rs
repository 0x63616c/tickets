//! Shared compact classification for ticket surfaces and editors.
use super::*;
use leptos::prelude::*;

#[component]
pub fn TypeBadge(kind: Option<String>, scope: Option<String>) -> impl IntoView {
    kind.or_else(||scope.as_ref().map(|_|"No type".into())).map(|kind| {
        let text = match scope { Some(scope) => format!("{kind} · {scope}"), None => kind };
        view! {<span class="ticket-type" title=text.clone() aria-label=format!("Type: {text}")>{text.clone()}</span>}
    })
}

#[component]
pub fn TicketClassification(kind: RwSignal<String>, scope: RwSignal<String>) -> impl IntoView {
    view! {<div class="ticket-classification">
        <label class="field"><span>"Type"</span><SelectField label="Ticket type" value=kind
            options=Signal::derive(||std::iter::once(SelectOption::new("", "No type"))
                .chain(tickets_core::ticket_type::TYPES.iter().map(|t|SelectOption::new(*t,*t))).collect())
            on_change=Callback::new(move|value|kind.set(value))/></label>
        <label class="field"><span>"Scope (optional)"</span><input aria-label="Ticket scope" placeholder="e.g. ui" autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"
            prop:value=move||scope.get() on:input=move|e|scope.set(event_target_value(&e))/></label>
        <span class="composer-hint">"Keep the title plain. PR titles use chore when type is unset."</span>
    </div>}
}
