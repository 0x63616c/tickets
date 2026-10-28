//! API-backed projects, using the fixture table and shared dialog/field controls.
use super::board::BoardState;
use crate::{components::*, ui::api};
use leptos::prelude::*;
use serde_json::json;
use tickets_core::{domain::Project, github::ProjectLink};
use wasm_bindgen_futures::spawn_local;

pub(crate) fn github_link(p: &Project) -> Option<ProjectLink> {
    ProjectLink::parse(
        p.github_owner.as_deref()?,
        &p.github_project_number?.to_string(),
        &p.github_project_kind,
    )
    .ok()
}

#[component]
pub fn ProjectLinks() -> impl IntoView {
    let board = expect_context::<BoardState>();
    view! { {move || board.projects.get().iter().find(|p| p.id==board.project.get()).and_then(|p| github_link(p).map(|link| (link, p.github_repo.clone()))).map(|(link, repo)| {
        let repository=repo.filter(|repo|tickets_core::github::valid_repo(repo)).map(|repo| {let label=format!("{}/{}",link.owner,repo);view!{<a class="subtle-link" href=format!("https://github.com/{label}") target="_blank" rel="noreferrer">{label.clone()}<Icon name="external"/></a>}});
        view! { <div class="project-links"><a class="subtle-link" href=link.url() target="_blank" rel="noreferrer"><Icon name="link"/>{format!("{} / Project {}",link.owner,link.number)}<Icon name="external"/></a>{repository}</div> }
    })} }
}

#[component]
pub fn ApiProjects() -> impl IntoView {
    let board = expect_context::<BoardState>();
    let location = leptos_router::hooks::use_location();
    let editor = RwSignal::new(None::<Option<Project>>);
    let navigate = leptos_router::hooks::use_navigate();
    Effect::new(move |_| {
        if location.search.get() == "new=true" {
            editor.set(Some(None));
            navigate(
                "/projects",
                leptos_router::NavigateOptions {
                    replace: true,
                    ..Default::default()
                },
            );
        }
    });
    let archived = RwSignal::new(false);
    board.refresh();
    view! {
        <header class="topbar"><div class="breadcrumbs"><Icon name="project"/>"Projects"</div><button class="button button-primary" disabled=move||board.loading.get() || !board.signed_in.get() on:click=move|_|editor.set(Some(None))><Icon name="plus"/>"New project"</button></header>
        <div class="projects-page">
            <div class="projects-heading"><h1>"Projects"</h1><span class="muted">{move||board.projects.get().iter().filter(|p|archived.get() || !p.archived).count()}" projects"</span><label class="canceled-toggle"><input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  type="checkbox" prop:checked=archived on:change=move|_|archived.update(|v|*v = !*v)/>"Show archived"</label></div>
            <Show when=move||board.loading.get()><p role="status">"Loading projects…"</p></Show>
            <Show when=move||!board.error.get().is_empty()><p role="alert">{move||board.error.get()}" "<button class="button" on:click=move|_|board.refresh()>"Retry"</button></p></Show>
            <Show when=move||!board.loading.get() && board.error.get().is_empty()>
                <div class="projects-table"><div class="projects-table-head"><span>"Name"</span><span>"Tickets"</span><span>"GitHub"</span></div>
                {move||board.projects.get().into_iter().filter(|p|archived.get() || !p.archived).map(|p| {
                    let id=p.id;
                    let edit=p.clone();
                    let link=github_link(&p);
                    view! { <div class="project-row api-project-row">
                        <div><a class="project-title" href="/" on:click=move|e|{if !board.select_project(id){e.prevent_default();}}><span class="project-mark" style=format!("color:{}",p.color)>{p.name.chars().next().unwrap_or('P').to_uppercase().to_string()}</span><div><strong>{p.name}</strong><p class="mono">{p.key_prefix}{if p.archived {" · Archived"}else{""}}</p></div></a></div>
                        <span class="muted">{move||board.tickets.get().iter().filter(|t|t.project_id==id).count()}</span>
                        <span class="muted">{link.map(|link|view!{<a class="subtle-link" href=link.url() target="_blank" rel="noreferrer">{format!("{} / {}",link.owner,link.number)}<Icon name="external"/></a>}.into_any()).unwrap_or_else(||view!{<span>"Not linked"</span>}.into_any())}</span>
                        <button class="icon-button project-row-action" aria-label=format!("Edit project {}",edit.name) on:click=move|_|editor.set(Some(Some(edit.clone())))><Icon name="more"/></button>
                    </div> }
                }).collect_view()}
                </div>
                <Show when=move||!board.projects.get().iter().any(|p|archived.get() || !p.archived)><EmptyState title="No active projects" description="Create a project to start adding tickets." action="New project" icon="project" on_action=Callback::new(move|_|editor.set(Some(None)))/></Show>
            </Show>
        </div>
        {move||editor.get().map(|project|view!{<ProjectEditor project on_close=Callback::new(move|_|editor.set(None))/>})}
    }
}

#[component]
fn ProjectEditor(project: Option<Project>, on_close: Callback<()>) -> impl IntoView {
    let board = expect_context::<BoardState>();
    let id = project.as_ref().map(|p| p.id);
    let name = RwSignal::new(project.as_ref().map(|p| p.name.clone()).unwrap_or_default());
    let prefix = RwSignal::new(
        project
            .as_ref()
            .map(|p| p.key_prefix.clone())
            .unwrap_or_default(),
    );
    let color = RwSignal::new(
        project
            .as_ref()
            .map(|p| p.color.clone())
            .unwrap_or("#5e6ad2".into()),
    );
    let owner = RwSignal::new(
        project
            .as_ref()
            .and_then(github_link)
            .map(|link| link.url())
            .unwrap_or_default(),
    );
    let kind = RwSignal::new(
        project
            .as_ref()
            .map(|p| p.github_project_kind.clone())
            .unwrap_or("users".into()),
    );
    let repo = RwSignal::new(
        project
            .as_ref()
            .and_then(|p| p.github_repo.clone())
            .unwrap_or_default(),
    );
    let archived = RwSignal::new(project.as_ref().is_some_and(|p| p.archived));
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    view! { <Modal title=if id.is_some(){"Edit project"}else{"New project"} on_close locked=Signal::derive(move||busy.get())>
        <form autocomplete="off" data-1p-ignore="true" data-form-type="other" class="project-form" on:submit=move|e|{
            e.prevent_default();
            if busy.get_untracked(){return;}
            error.set(String::new());
            let link=if owner.get_untracked().trim().is_empty(){Ok(None)}else{ProjectLink::parse(&owner.get_untracked(),"",&kind.get_untracked()).map(Some)};
            let link=match link{Ok(link)=>link,Err(e)=>{error.set(e);return;}};
            let repository=repo.get_untracked().trim().to_string();
            if !repository.is_empty() && (!tickets_core::github::valid_repo(&repository) || link.is_none()) {error.set("Enter a repository name and link its GitHub Project first.".into());return;}
            let body=json!({"name":name.get_untracked().trim(),"key_prefix":prefix.get_untracked().trim(),"color":color.get_untracked(),"archived":archived.get_untracked(),"github_owner":link.as_ref().map(|l|&l.owner),"github_project_number":link.as_ref().map(|l|l.number),"github_project_kind":link.as_ref().map(|l|l.kind.as_str()).unwrap_or("users"),"github_repo":if repository.is_empty(){None}else{Some(repository)}});
            if let Err(e)=tickets_core::api::validate_create("projects",&body){error.set(e);return;}
            busy.set(true);
            spawn_local(async move{
                let path=id.map(|id|format!("/api/v1/projects/{id}")).unwrap_or("/api/v1/projects".into());
                match api(if id.is_some(){"PATCH"}else{"POST"},&path,body).await{
                    Ok(value)=>match serde_json::from_value::<Project>(value){
                        Ok(saved)=>{board.projects.update(|ps|{ps.retain(|p|p.id!=saved.id);ps.push(saved);ps.sort_by_key(|p|p.id);});board.notice.set(if id.is_some(){"Project saved."}else{"Project created. Open it to add a ticket."}.into());board.refresh();on_close.run(());},
                        Err(e)=>error.set(format!("Invalid project response: {e}"))
                    },Err(e)=>error.set(e)
                }
                busy.set(false);
            });
        }>
        <div class="modal-body"><TextInput id="project-title" label="Name" value=name/>
        <TextInput id="project-prefix" label="Key prefix" placeholder="TKT" value=prefix/>
        <Show when=move||id.is_some()><p class="muted">"Changing the prefix updates existing ticket keys."</p></Show>
        <fieldset class="project-colors"><legend>"Icon color"</legend><div class="project-swatches">{[("#5e6ad2","Accent","--accent"),("#8b82d9","Feature","--label-feature"),("#eb5757","Bug","--label-bug"),("#4ea7fc","Infrastructure","--label-infra"),("#c6a1b9","Design","--label-design"),("#27a644","Green","--success"),("#f0bf00","Yellow","--warning")].into_iter().map(|(value,label,token)|view!{<button type="button" class="project-swatch" aria-label=format!("{label} color") aria-pressed=move||(color.get()==value).to_string() style=format!("--swatch:var({token})") on:click=move|_|color.set(value.into())/>}).collect_view()}</div></fieldset>
        <TextInput id="project-owner" label="GitHub Project" placeholder="github.com/orgs/example/projects/1 or example/1" value=owner/>
        <Show when=move||{let value=owner.get(); !value.trim().is_empty() && !value.contains("github.com/") && !value.contains("://")}><label class="field" for="project-kind">"Owner type"<select autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  id="project-kind" prop:value=kind on:change=move|e|kind.set(event_target_value(&e))><option value="users">"User"</option><option value="orgs">"Organization"</option></select></label></Show>
        <TextInput id="project-repo" label="Repository name (optional)" placeholder="repo" value=repo/>
        <Show when=move||id.is_some()><label class="canceled-toggle"><input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  type="checkbox" prop:checked=archived on:change=move|_|archived.update(|v|*v = !*v)/>"Archived"</label><p class="muted">"Archived projects leave the sidebar. Tickets are kept and remain editable."</p></Show>
        <Show when=move||!error.get().is_empty()><p role="alert">{error}</p></Show>
        </div><div class="modal-footer"><button class="button button-ghost" type="button" disabled=busy on:click=move|_|on_close.run(())>"Cancel"</button><button class="button button-primary" type="submit" disabled=busy>{move||if busy.get(){"Saving…"}else if id.is_some(){"Save project"}else{"Create project"}}</button></div>
        </form>
    </Modal> }
}
