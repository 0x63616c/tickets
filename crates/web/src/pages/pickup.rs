use crate::{components::*, ui::api};
use leptos::prelude::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use tickets_core::pickup::Rules;
use wasm_bindgen_futures::spawn_local;

#[derive(Clone, serde::Deserialize)]
struct Saved {
    rules: Rules,
    source: String,
}
#[component]
pub fn PickupSettings(#[prop(default = false)] fixture: bool) -> impl IntoView {
    let board = expect_context::<crate::pages::board::BoardState>();
    let scope = RwSignal::new(0_i64);
    let locked = RwSignal::new(false);
    let unsaved = RwSignal::new(false);
    let fixtures = RwSignal::new(BTreeMap::from([(0, Rules::default())]));
    let options = Signal::derive(move || {
        let mut options = vec![SelectOption::new("0", "Workspace default")];
        if fixture {
            options.push(SelectOption::new("1", "Tickets"));
        } else {
            options.extend(board.projects.get().into_iter().map(SelectOption::project));
        }
        options
    });
    view! {<section class="settings-section pickup-settings"><h1>"Pickup rules"</h1><p class="settings-description">"Choose which Todo tickets agents pick up next."</p>
        <label class="field pickup-scope"><span>"Apply to"</span><SelectField label="Rule scope" described_by="pickup-scope-hint" options value=Signal::derive(move||scope.get().to_string()) disabled=locked on_change=Callback::new(move|v:String|scope.set(v.parse().unwrap_or(0)))/></label>
        <p id="pickup-scope-hint" class="field-hint" role="status">{move||if unsaved.get(){"Save or discard changes below to switch scope."}else{""}}</p>
        <For each=move||vec![scope.get()] key=|scope|*scope children=move|scope|view!{<PickupEditor scope fixture fixtures locked unsaved/>}/>
    </section>}
}
#[component]
fn PickupEditor(
    scope: i64,
    fixture: bool,
    fixtures: RwSignal<BTreeMap<i64, Rules>>,
    locked: RwSignal<bool>,
    unsaved: RwSignal<bool>,
) -> impl IntoView {
    let saved = RwSignal::new(None::<Saved>);
    let draft = RwSignal::new(Rules::default());
    let cap = RwSignal::new(String::new());
    let lease = RwSignal::new(String::new());
    let labels = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let label_error = RwSignal::new(String::new());
    let message = RwSignal::new(String::new());
    let revision = RwSignal::new(0_u32);
    let dirty = Signal::derive(move || {
        saved.get().is_some_and(|s| {
            draft.get() != s.rules
                || cap.get() != s.rules.concurrency_cap.to_string()
                || lease.get() != s.rules.lease_seconds.to_string()
                || labels.get() != s.rules.excluded_labels.join(", ")
        })
    });
    Effect::new(move |_| {
        locked.set(busy.get() || dirty.get());
        unsaved.set(dirty.get());
    });
    let apply = Callback::new(move |s: Saved| {
        cap.set(s.rules.concurrency_cap.to_string());
        lease.set(s.rules.lease_seconds.to_string());
        labels.set(s.rules.excluded_labels.join(", "));
        draft.set(s.rules.clone());
        saved.set(Some(s));
    });
    let url = if scope == 0 {
        "/api/v1/pickup-rules".to_string()
    } else {
        format!("/api/v1/pickup-rules?project={scope}")
    };
    let url = StoredValue::new(url);
    Effect::new(move |_| {
        revision.get();
        busy.set(true);
        error.set(String::new());
        spawn_local(async move {
            let result = if fixture {
                let map = fixtures.get_untracked();
                Ok(Saved {
                    rules: map.get(&scope).unwrap_or(&map[&0]).clone(),
                    source: if scope != 0 && map.contains_key(&scope) {
                        "project"
                    } else {
                        "workspace"
                    }
                    .into(),
                })
            } else {
                api("GET", &url.get_value(), Value::Null)
                    .await
                    .and_then(|v| {
                        serde_json::from_value(v).map_err(|_| "Could not read pickup rules.".into())
                    })
            };
            if saved.is_disposed() {
                return;
            }
            match result {
                Ok(s) => apply.run(s),
                Err(e) => error.set(e),
            }
            busy.set(false);
        });
    });
    let save = move |method: &'static str| {
        if busy.get_untracked() {
            return;
        }
        let mut rules = draft.get_untracked();
        if method == "PUT" {
            rules.concurrency_cap = cap.get_untracked().parse().unwrap_or(0);
            rules.lease_seconds = lease.get_untracked().parse::<u32>().unwrap_or(0);
            rules.excluded_labels = labels
                .get_untracked()
                .split(',')
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
                .collect();
            if let Err(e) = rules.validate_labels() {
                label_error.set(e);
                return;
            }
            label_error.set(String::new());
            if let Err(e) = rules.validate() {
                error.set(e);
                return;
            }
        }
        busy.set(true);
        error.set(String::new());
        message.set(String::new());
        spawn_local(async move {
            let result = if fixture {
                fixtures.update(|map| {
                    if method == "DELETE" {
                        map.remove(&scope);
                    } else {
                        map.insert(scope, rules);
                    }
                });
                let map = fixtures.get_untracked();
                Ok(Saved {
                    rules: map.get(&scope).unwrap_or(&map[&0]).clone(),
                    source: if scope != 0 && map.contains_key(&scope) {
                        "project"
                    } else {
                        "workspace"
                    }
                    .into(),
                })
            } else {
                api(
                    method,
                    &url.get_value(),
                    if method == "DELETE" {
                        Value::Null
                    } else {
                        json!(rules)
                    },
                )
                .await
                .and_then(|v| {
                    serde_json::from_value(v).map_err(|_| "Could not read saved rules.".into())
                })
            };
            if saved.is_disposed() {
                return;
            }
            match result {
                Ok(s) => {
                    apply.run(s);
                    message.set(
                        if method == "DELETE" {
                            "Using workspace rules."
                        } else {
                            "Rules saved."
                        }
                        .into(),
                    );
                }
                Err(e) => error.set(e),
            }
            busy.set(false);
        });
    };
    let disabled = Signal::derive(move || busy.get() || saved.get().is_none());
    view! {<div class="pickup-editor">
        <p class="field-hint pickup-inheritance">{move||if scope==0 {"Default for projects without an override."}else if saved.get().is_some_and(|s|s.source=="project"){ "This project has its own rules." }else{"Using workspace rules. Save to create a project override."}}</p>
        <Show when=move||saved.get().is_none() && busy.get()><LoadingIndicator label="Loading pickup rules"/></Show>
        <form on:submit=move|e|{e.prevent_default();save("PUT");}>
            <fieldset disabled=disabled><legend>"Eligibility"</legend>
                <div class="pickup-fixed"><span>"Ticket status"</span><span class="icon-label"><StatusIcon status=crate::fixtures::Status::Todo/>"Todo only"</span></div>
                <p class="field-hint">"Backlog, archived projects and claimed tickets are skipped."</p>
                <RuleToggle label="Skip blocked tickets" hint="Wait until every prerequisite is Done or Canceled." checked=Signal::derive(move||draft.get().skip_blocked) on_change=Callback::new(move|v|draft.update(|r|r.skip_blocked=v))/>
                <RuleToggle label="Skip tickets with open PRs" hint="Includes draft pull requests." checked=Signal::derive(move||draft.get().skip_open_pr) on_change=Callback::new(move|v|draft.update(|r|r.skip_open_pr=v))/>
                <label class="field pickup-labels" for="pickup-labels"><span>"Skip labels"</span><input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other" id="pickup-labels" prop:value=labels on:input=move|e|{labels.set(event_target_value(&e));label_error.set(String::new());} placeholder="Add comma-separated label names" aria-describedby="pickup-labels-hint pickup-labels-error" aria-invalid=move||(!label_error.get().is_empty()).to_string()/></label>
                <p class="field-hint" id="pickup-labels-hint">"Separate names with commas. Labels stay in your workspace."</p>
                <p id="pickup-labels-error" class="settings-error" role="alert">{label_error}</p>
            </fieldset>
            <fieldset disabled=disabled><legend>"Pick order"</legend>
                <RuleToggle label="Unblock other tickets first" hint="Prefer tickets that block open work." checked=Signal::derive(move||draft.get().blockers_first) on_change=Callback::new(move|v|draft.update(|r|r.blockers_first=v))/>
                <RuleToggle label="Then highest priority" hint="Urgent → High → Medium → Low → None" checked=Signal::derive(move||draft.get().priority_first) on_change=Callback::new(move|v|draft.update(|r|r.priority_first=v))/>
                <p class="field-hint">"Oldest created first breaks ties."</p>
            </fieldset>
            <fieldset disabled=disabled><legend>"Capacity & leases"</legend>
                <div class="pickup-numbers"><label class="field" for="pickup-cap"><span>"Concurrent tickets per project"</span><input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other" id="pickup-cap" type="number" min="1" max="100" step="1" required prop:value=cap on:input=move|e|cap.set(event_target_value(&e)) aria-describedby="pickup-cap-hint"/></label>
                <label class="field" for="pickup-lease"><span>"Lease duration (seconds)"</span><input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other" id="pickup-lease" type="number" min="60" max="86400" step="1" required prop:value=lease on:input=move|e|lease.set(event_target_value(&e)) aria-describedby="pickup-lease-hint"/></label></div>
                <p class="field-hint" id="pickup-cap-hint">"Counts In Progress tickets and active claims, once per ticket."</p><p class="field-hint" id="pickup-lease-hint">"Agents renew while working. Expired In Progress claims return to Todo."</p>
            </fieldset>
            <p class="settings-error" role="alert">{error}</p>
            <Show when=move||saved.get().is_none() && !busy.get()><button class="button button-secondary" type="button" on:click=move|_|revision.update(|v|*v+=1)>"Retry loading rules"</button></Show>
            <div class="settings-save"><BusyButton busy disabled=Signal::derive(move||disabled.get() || (!dirty.get() && (scope==0 || saved.get().is_some_and(|s|s.source=="project"))))>{if scope==0 {"Save rules"}else{"Save project rules"}}</BusyButton>
            <Show when=move||dirty.get()><button type="button" class="button button-ghost" disabled=busy on:click=move|_|{if let Some(s)=saved.get_untracked(){apply.run(s);error.set(String::new());label_error.set(String::new());message.set(String::new());}}>"Discard changes"</button></Show>
            <Show when=move||scope!=0 && saved.get().is_some_and(|s|s.source=="project")><button type="button" class="button button-secondary" disabled=Signal::derive(move||busy.get() || dirty.get()) on:click=move|_|save("DELETE")>"Use workspace rules"</button></Show></div>
            <p class="pickup-result" role="status">{move||if dirty.get(){"Unsaved changes. Save or discard to switch scope.".to_string()}else{message.get()}}</p>
        </form>
    </div>}
}
#[component]
fn RuleToggle(
    label: &'static str,
    hint: &'static str,
    checked: Signal<bool>,
    on_change: Callback<bool>,
) -> impl IntoView {
    view! {<label class="pickup-toggle"><span><strong>{label}</strong><span>{hint}</span></span><input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other" type="checkbox" prop:checked=checked on:change=move|e|on_change.run(event_target_checked(&e))/></label>}
}
