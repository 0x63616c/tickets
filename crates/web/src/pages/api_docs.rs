//! Native OpenAPI reader. Requests and credentials stay in the current session.
use crate::components::{Icon, LoadingIndicator};
use futures_util::future::{AbortHandle, Abortable};
use leptos::prelude::*;
use serde_json::Value;
use std::collections::BTreeMap;

fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_owned()
}
fn deref<'a>(value: &'a Value, spec: &'a Value) -> &'a Value {
    value
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|r| r.strip_prefix('#'))
        .and_then(|p| spec.pointer(p))
        .unwrap_or(value)
}
fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}
/// Compact table labels; full reusable schemas remain available below.
fn schema_label(schema: &Value) -> String {
    if let Some(reference) = schema["$ref"].as_str() {
        return reference.rsplit('/').next().unwrap_or(reference).to_owned();
    }
    let mut parts = vec![schema["type"].as_str().unwrap_or("value").to_owned()];
    if let Some(values) = schema["enum"].as_array() {
        parts.push(
            values
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| v.to_string())
                })
                .collect::<Vec<_>>()
                .join(" | "),
        );
    }
    for (key, label) in [
        ("minimum", "min"),
        ("maximum", "max"),
        ("maxLength", "max length"),
        ("default", "default"),
        ("pattern", "pattern"),
        ("const", "value"),
    ] {
        if let Some(value) = schema.get(key) {
            parts.push(format!(
                "{label} {}",
                value
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| value.to_string())
            ));
        }
    }
    parts.join(" · ")
}
fn auth(operation: &Value) -> String {
    let schemes = operation["security"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|v| v.as_object().into_iter().flat_map(|o| o.keys()))
        .map(|s| match s.as_str() {
            "bearerAuth" => "Bearer token",
            "sessionCookie" => "Session cookie",
            "githubSignature" => "GitHub signature",
            "setupState" => "Setup state",
            _ => s,
        })
        .collect::<Vec<_>>();
    if schemes.is_empty() {
        "Public".into()
    } else {
        schemes.join(" or ")
    }
}

#[component]
pub fn ApiDocs() -> impl IntoView {
    let spec = RwSignal::new(None::<Value>);
    let error = RwSignal::new(String::new());
    let revision = RwSignal::new(0_u32);
    let query = RwSignal::new(String::new());
    Effect::new(move |_| {
        revision.track();
        error.set(String::new());
        let (abort, registration) = AbortHandle::new_pair();
        on_cleanup(move || abort.abort());
        wasm_bindgen_futures::spawn_local(async move {
            let _ = Abortable::new(async move {
                match crate::ui::api("GET", "/api/openapi.json", Value::Null).await {
                    Ok(value) if value["openapi"] == "3.1.0" && value["paths"].is_object() => spec.set(Some(value)),
                    Ok(_) => error.set("The API reference returned an invalid document. Retry to load it again.".into()),
                    Err(message) => error.set(format!("Could not load the API reference. {message}")),
                }
            }, registration).await;
        });
    });
    view! {
        <style>{include_str!("../../style/api-docs.css")}</style>
        <header class="topbar"><h1 class="home-title">"API reference"</h1><a class="button button-secondary" href="/api/openapi.json" target="_blank" rel="noreferrer"><Icon name="code"/>"OpenAPI JSON"</a></header>
        <div class="api-docs">
            <div class="api-docs-intro"><h2>"Build with Tickets"</h2><p>"Explore every endpoint, its inputs and responses. Requests use your current workspace session."</p><p class="api-docs-note">"Bearer tokens and session cookies have workspace access. Cookie writes require a matching Origin. Invalid bearer credentials never fall back to a cookie."</p></div>
            <label class="api-docs-search"><Icon name="search"/><span class="sr-only">"Filter endpoints"</span><input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other" type="search" placeholder="Filter by tag, method or path…" prop:value=query on:input=move |ev| query.set(event_target_value(&ev))/></label>
            <Show when=move || !error.get().is_empty()><div class="home-error" role="alert"><span>{move || error.get()}</span><button class="button button-secondary" on:click=move |_| revision.update(|r| *r+=1)>"Retry"</button></div></Show>
            <Show when=move || spec.get().is_none() && error.get().is_empty()><LoadingIndicator label="Loading API reference"/></Show>
            {move || spec.get().map(|document| view!{<Reference document query/>})}
        </div>
    }
}

#[derive(Clone, PartialEq)]
struct Endpoint {
    method: String,
    path: String,
    operation: Value,
}
#[component]
fn Reference(document: Value, query: RwSignal<String>) -> impl IntoView {
    let mut groups = BTreeMap::<String, Vec<Endpoint>>::new();
    for (path, item) in document["paths"].as_object().into_iter().flatten() {
        for (method, operation) in item.as_object().into_iter().flatten() {
            groups
                .entry(operation["tags"][0].as_str().unwrap_or("Other").to_owned())
                .or_default()
                .push(Endpoint {
                    method: method.to_uppercase(),
                    path: path.clone(),
                    operation: operation.clone(),
                });
        }
    }
    let total: usize = groups.values().map(Vec::len).sum();
    let spec = StoredValue::new(document);
    let all = StoredValue::new(groups);
    let matches = Memo::new(move |_| {
        let q = query.get().trim().to_lowercase();
        all.with_value(|groups| {
            groups
                .iter()
                .map(|(tag, endpoints)| {
                    (
                        tag.clone(),
                        endpoints
                            .iter()
                            .filter(|e| {
                                format!(
                                    "{} {} {} {} {}",
                                    tag,
                                    e.method,
                                    e.path,
                                    text(&e.operation, "summary"),
                                    text(&e.operation, "description")
                                )
                                .to_lowercase()
                                .contains(&q)
                            })
                            .cloned()
                            .collect::<Vec<_>>(),
                    )
                })
                .filter(|(_, endpoints)| !endpoints.is_empty())
                .collect::<Vec<_>>()
        })
    });
    view! {
        <div class="api-docs-meta" role="status">{move || format!("{} of {total} endpoints · OpenAPI 3.1",matches.get().iter().map(|(_,e)|e.len()).sum::<usize>())}</div>
        <Show when=move || matches.get().is_empty()><p class="api-docs-empty">"No endpoints match. Try a method such as GET or a resource such as tickets."</p></Show>
        <For each=move || matches.get() key=|(tag,_)|tag.clone() children=move |(tag,_)|{
            let heading=tag.clone();
            let rows=Memo::new(move |_| matches.get().into_iter().find(|(t,_)| t==&tag).map(|(_,e)|e).unwrap_or_default());
            view!{<section class="api-docs-group"><h2>{heading}</h2><For each=move || rows.get() key=|e|format!("{} {}",e.method,e.path) children=move |endpoint|view!{<Operation endpoint spec/>}/></section>}
        }/>
        <details class="api-docs-components"><summary>"Reusable schemas"</summary>{spec.with_value(|s|s["components"]["schemas"].as_object().into_iter().flatten().map(|(name,schema)| view!{<details class="api-docs-schema"><summary>{name.clone()}</summary><pre>{pretty(schema)}</pre></details>}).collect_view())}</details>
    }
}

#[component]
fn Operation(endpoint: Endpoint, spec: StoredValue<Value>) -> impl IntoView {
    let Endpoint {
        method,
        path,
        operation,
    } = endpoint;
    let params = operation["parameters"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let auth_label = auth(&operation);
    let try_read = method == "GET"
        && !path.contains("/live/")
        && !path.contains("/avatar")
        && !path.contains("/credentials")
        && !path.contains("/manifest-callback");
    let try_path = path.clone();
    let try_params = params.clone();
    let method_class = format!("api-docs-method api-docs-method-{}", method.to_lowercase());
    view! {
        <details class="api-docs-operation" id=text(&operation,"operationId")>
            <summary><span class=method_class>{method}</span><code class="api-docs-path">{path}</code><span class="api-docs-summary">{text(&operation,"summary")}</span><Icon name="chevron"/></summary>
            <div class="api-docs-detail"><p>{text(&operation,"description")}</p><p class="api-docs-auth"><Icon name="lock"/><strong>"Auth"</strong>{auth_label}</p>
                {(!params.is_empty()).then(||view!{<div class="api-docs-table-wrap"><table><caption>"Parameters"</caption><thead><tr><th>"Name"</th><th>"Location"</th><th>"Schema"</th><th>"Description"</th></tr></thead><tbody>{params.into_iter().map(|p|view!{<tr><th scope="row"><code>{text(&p,"name")}</code>{(p["required"]==true).then(||view!{<span class="api-docs-required">"Required"</span>})}</th><td>{text(&p,"in")}</td><td><code>{schema_label(&p["schema"])}</code></td><td>{text(&p,"description")}</td></tr>}).collect_view()}</tbody></table></div>})}
                {operation.get("requestBody").map(|body|view!{<SchemaContent title="Request body" value=body.clone() spec/>})}
                <details class="api-docs-schema"><summary>"Responses"</summary>{operation["responses"].as_object().into_iter().flatten().map(|(status,response)|{
                    let response=spec.with_value(|s|deref(response,s).clone());
                    view!{<SchemaContent title=format!("{status} · {}",text(&response,"description")) value=response spec/>}
                }).collect_view()}</details>
                {try_read.then(||view!{<TryRead path=try_path params=try_params/>})}
            </div>
        </details>
    }
}
#[component]
fn SchemaContent(
    #[prop(into)] title: String,
    value: Value,
    spec: StoredValue<Value>,
) -> impl IntoView {
    let value = spec.with_value(|s| {
        let mut value = deref(&value, s).clone();
        if let Some(content) = value.get_mut("content").and_then(Value::as_object_mut) {
            for media in content.values_mut() {
                if let Some(schema) = media.get_mut("schema") {
                    *schema = deref(schema, s).clone();
                }
            }
        }
        value
    });
    view! {<details class="api-docs-schema"><summary>{title}</summary><pre>{pretty(&value)}</pre></details>}
}

#[component]
fn TryRead(path: String, params: Vec<Value>) -> impl IntoView {
    let fields = params
        .into_iter()
        .filter(|p| p["in"] == "path" || p["in"] == "query")
        .map(|p| {
            (
                text(&p, "name"),
                text(&p, "in"),
                p["required"] == true,
                RwSignal::new(String::new()),
            )
        })
        .collect::<Vec<_>>();
    let inputs = fields.clone();
    let busy = RwSignal::new(false);
    let result = RwSignal::new(String::new());
    let active = StoredValue::new_local(None::<AbortHandle>);
    on_cleanup(move || {
        active.with_value(|handle| {
            if let Some(handle) = handle {
                handle.abort();
            }
        })
    });
    view! {<details class="api-docs-schema api-docs-try"><summary>"Try this GET request"</summary><p>"Uses your session cookie. Optional fields are omitted when blank."</p>
        <form on:submit=move |ev| {
            ev.prevent_default();
            if busy.get_untracked(){return;}
            let mut url=path.clone();
            let mut query=Vec::new();
            for (name,location,required,input) in &fields {
                let value=input.get_untracked();
                if value.is_empty() && *required {result.set(format!("Enter {name} before sending."));return;}
                let encoded=js_sys::encode_uri_component(&value).as_string().unwrap_or_default();
                if location=="path" {url=url.replace(&format!("{{{name}}}"),&encoded);} else if !value.is_empty(){query.push(format!("{name}={encoded}"));}
            }
            if !query.is_empty(){url.push('?');url.push_str(&query.join("&"));}
            busy.set(true); result.set(String::new());
            let (abort, registration) = AbortHandle::new_pair();
            active.set_value(Some(abort));
            wasm_bindgen_futures::spawn_local(async move {
                let request=async move {
                    let response=gloo_net::http::Request::get(&url).send().await;
                    let output=match response {
                        Ok(response)=>{
                            let status=response.status();
                            let body=response.text().await.unwrap_or_else(|_|"Could not read response body.".into());
                            let body=serde_json::from_str::<Value>(&body).map(|v|pretty(&v)).unwrap_or(body);
                            format!("HTTP {status}\n{body}")
                        },
                        Err(_)=>"The request could not reach Tickets. Check your connection and try again.".into(),
                    };
                    if !result.is_disposed(){result.set(output);busy.set(false);}
                };
                let _ = Abortable::new(request, registration).await;
            });
        }>
        {inputs.into_iter().map(|(name,location,required,input)|view!{<label class="api-docs-input field"><span>{format!("{name} · {location}{}",if required{" · required"}else{""})}</span><input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other" required=required prop:value=input on:input=move |ev|input.set(event_target_value(&ev))/></label>}).collect_view()}
        <button class="button button-secondary" type="submit" disabled=busy>{move ||if busy.get(){"Sending…"}else{"Send GET request"}}</button></form>
        <Show when=move ||!result.get().is_empty()><pre aria-live="polite">{move ||result.get()}</pre></Show>
    </details>}
}
