//! Compact, read-only activity rows shared by the real detail and fixture preview.
use super::*;
use serde_json::{Value, json};
use tickets_core::history::{ActorKind, Entry};

fn value_label(field: &str, value: &Value) -> String {
    if value.is_null() {
        return if field == "assignee" {
            "Unassigned"
        } else {
            "None"
        }
        .into();
    }
    match field {
        "status" => match value.as_str().unwrap_or("") {
            "in_progress" => "In progress",
            "in_review" => "In review",
            "todo" => "Todo",
            "backlog" => "Backlog",
            "done" => "Done",
            "canceled" => "Canceled",
            _ => "Unknown",
        }
        .into(),
        "priority" => match value.as_i64().unwrap_or(0) {
            1 => "Urgent",
            2 => "High",
            3 => "Medium",
            4 => "Low",
            _ => "No priority",
        }
        .into(),
        "project" => value["name"].as_str().unwrap_or("Project").into(),
        field if field.starts_with("label_") => value["name"].as_str().unwrap_or("Label").into(),
        field if field.starts_with("blocker_") || field.starts_with("blocks_") => value["key"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("Ticket {}", value["id"])),
        _ => value
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| value.to_string()),
    }
}
fn relative_time(iso: &str) -> String {
    let seconds = ((js_sys::Date::now() - js_sys::Date::parse(iso)) / 1000.0).max(0.0);
    if seconds < 60.0 {
        "just now".into()
    } else if seconds < 3600.0 {
        format!("{}m ago", (seconds / 60.0) as u64)
    } else if seconds < 86400.0 {
        format!("{}h ago", (seconds / 3600.0) as u64)
    } else if seconds < 604800.0 {
        format!("{}d ago", (seconds / 86400.0) as u64)
    } else {
        iso.get(..10).unwrap_or(iso).into()
    }
}
#[component]
pub fn ActivityTime(iso: String, #[prop(optional)] edited: bool) -> impl IntoView {
    let label = RwSignal::new(if edited {
        "edited".into()
    } else {
        relative_time(&iso)
    });
    let timestamp = iso.clone();
    let interval = gloo_timers::callback::Interval::new(60_000, move || {
        if !edited {
            label.set(relative_time(&timestamp));
        }
    });
    let _interval = StoredValue::new_local(interval);
    let exact = iso.get(..19).unwrap_or(&iso).replace('T', " ") + " UTC";
    let exact = if edited {
        format!("Edited {exact}")
    } else {
        exact
    };
    let show_exact = RwSignal::new(false);
    let title = exact.clone();
    let accessible = format!("{exact}. Toggle exact time");
    view! {<button type="button" class="activity-time" class:comment-edited=edited title=title aria-label=accessible aria-pressed=move||show_exact.get().to_string() on:click=move|_|show_exact.update(|v| *v = !*v)><time datetime=iso>{move||if show_exact.get(){exact.clone()}else{label.get()}}</time></button>}
}
#[component]
pub fn HistoryEvent(entry: Entry) -> impl IntoView {
    let field = entry.field.clone();
    let before = value_label(&field, &entry.old_value);
    let after = value_label(&field, &entry.new_value);
    let action = match field.as_str() {
        "created" if entry.actor_id == "migration" => "started tracking this ticket".into(),
        "created" => "created this ticket".into(),
        "deleted" => "deleted this ticket".into(),
        "comment_created" => "posted a comment".into(),
        "comment_edited" => "edited a comment".into(),
        "comment_deleted" => "deleted a comment".into(),
        "description" => "changed the description".into(),
        "label_added" => format!("added label {after}"),
        "label_removed" => format!("removed label {before}"),
        "blocker_added" => format!("added blocker {after}"),
        "blocker_removed" => format!("removed blocker {before}"),
        "blocks_added" => format!("marked this as blocking {after}"),
        "blocks_removed" => format!("stopped blocking {before}"),
        "claim" => "claimed this ticket".into(),
        "heartbeat" => "renewed the claim".into(),
        "expiry" => "recorded claim expiry".into(),
        "release" => "released the claim".into(),
        _ => format!("changed {field}"),
    };
    let show_values = matches!(
        field.as_str(),
        "status" | "priority" | "assignee" | "project" | "type" | "scope"
    );
    let expandable = matches!(field.as_str(), "description" | "title");
    let old_text = entry.old_value.as_str().unwrap_or("").to_owned();
    let new_text = entry.new_value.as_str().unwrap_or("").to_owned();
    let icon = if entry.actor_kind == ActorKind::Webhook {
        "github"
    } else {
        match field.as_str() {
            "label_added" | "label_removed" => "tag",
            "blocker_added" | "blocker_removed" | "blocks_added" | "blocks_removed" => "link",
            "assignee" => "user",
            "created" => "plus",
            _ => "clock",
        }
    };
    let actor_title = match entry.actor_kind {
        ActorKind::User => format!("User #{}", entry.actor_id),
        ActorKind::ApiToken => format!("API token #{}", entry.actor_id),
        ActorKind::Webhook => "GitHub webhook".into(),
        ActorKind::System => "System".into(),
    };
    view! {<div class="history-event" data-history-id=entry.id data-field=field>
        <span class="history-glyph" aria-hidden="true"><Icon name=icon/></span>
        <div class="history-content"><div class="history-line"><span><strong title=actor_title>{entry.actor_label}</strong>" "{action}{show_values.then(||view!{<span class="history-values">" "<span>{before}</span><span aria-label="to">" → "</span><strong>{after}</strong></span>})}</span><ActivityTime iso=entry.recorded_at/></div>
        {expandable.then(||view!{<details class="history-diff"><summary>"View change"</summary><div><span class="muted">"Before"</span><pre>{if old_text.is_empty(){"(empty)".into()}else{old_text}}</pre><span class="muted">"After"</span><pre>{if new_text.is_empty(){"(empty)".into()}else{new_text}}</pre></div></details>})}
        </div>
    </div>}
}
/// Explicit sample events, never inferred from ticket timestamps.
pub fn history_fixture() -> Vec<Entry> {
    [
        (
            5,
            "user",
            "1",
            "Sample user",
            "priority",
            json!(2),
            json!(1),
        ),
        (
            4,
            "api_token",
            "2",
            "API token #2",
            "status",
            json!("todo"),
            json!("in_progress"),
        ),
        (
            3,
            "user",
            "1",
            "Sample user",
            "description",
            json!("Keep changes visible."),
            json!("Keep every **saved change** visible, including API edits."),
        ),
        (
            2,
            "user",
            "1",
            "Sample user",
            "label_added",
            Value::Null,
            json!({"id":1,"name":"Feature"}),
        ),
        (
            1,
            "user",
            "1",
            "Sample user",
            "created",
            Value::Null,
            json!({"title":"Record ticket changes"}),
        ),
    ]
    .into_iter()
    .map(
        |(id, kind, actor_id, label, field, old_value, new_value)| Entry {
            id,
            subject_id: 1,
            ticket_id: 1,
            actor_kind: serde_json::from_value(json!(kind)).unwrap(),
            actor_id: actor_id.into(),
            actor_label: label.into(),
            field: field.into(),
            old_value,
            new_value,
            recorded_at: format!("2026-10-30T10:{:02}:00.000Z", id * 5),
        },
    )
    .collect()
}
