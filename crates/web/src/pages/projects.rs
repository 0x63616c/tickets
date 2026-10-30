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

pub(crate) fn repository(p: &Project) -> Option<String> {
    let owner = p.github_owner.as_deref()?;
    let repo = p.github_repo.as_deref()?;
    (tickets_core::github::valid_owner(owner) && tickets_core::github::valid_repo(repo))
        .then(|| format!("{owner}/{repo}"))
}

#[component]
fn RepositoryLinks(repo: String) -> impl IntoView {
    view! { <div class="repository-links"><a class="subtle-link" href=format!("https://github.com/{repo}") target="_blank" rel="noreferrer">{repo.clone()}<Icon name="external"/></a><div class="repository-shortcuts"><a class="subtle-link" href=format!("https://github.com/{repo}/pulls") target="_blank" rel="noreferrer">"Open PRs"</a><a class="subtle-link" href=format!("https://github.com/{repo}/actions") target="_blank" rel="noreferrer">"Actions / CI"</a></div></div> }
}

#[component]
pub fn ProjectLinks() -> impl IntoView {
    let board = expect_context::<BoardState>();
    view! { {move || board.projects.get().iter().find(|p| p.id==board.project.get()).map(|p| {
        let repo=repository(p);
        let project=github_link(p);
        view! { <div class="project-links">{repo.map(|repo|view!{<a class="subtle-link" href=format!("https://github.com/{repo}") target="_blank" rel="noreferrer">{repo.clone()}<Icon name="external"/></a>})}{project.map(|link|view!{<a class="subtle-link" href=link.url() target="_blank" rel="noreferrer">{format!("GitHub Project {}",link.number)}<Icon name="external"/></a>})}</div> }
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
    let reveal_root = NodeRef::<leptos::html::Div>::new();
    use_ascii_reveal(
        reveal_root,
        Signal::derive(move || board.loading.get()),
        true,
    );
    view! {
        <header class="topbar"><div class="breadcrumbs"><Icon name="project"/>"Projects"</div><button class="button button-primary" disabled=move||board.loading.get() || !board.signed_in.get() on:click=move|_|editor.set(Some(None))><Icon name="plus"/>"New project"</button></header>
        <div node_ref=reveal_root class="projects-page">
            <div class="projects-heading"><h1>"Projects"</h1><span class="muted" hidden=move||board.loading.get()>{move||crate::count::count_label(board.projects.get().iter().filter(|p|archived.get() || !p.archived).count(), "project")}</span><label class="canceled-toggle"><input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  type="checkbox" prop:checked=archived on:change=move|_|archived.update(|v|*v = !*v)/>"Show archived"</label></div>
            <Show when=move||board.loading.get()><LoadingSkeleton layout="projects"/></Show>
            <Show when=move||!board.error.get().is_empty()><p role="alert">{move||board.error.get()}" "<button class="button" on:click=move|_|board.refresh()>"Retry"</button></p></Show>
            <Show when=move||!board.loading.get() && board.error.get().is_empty()>
                <div class="projects-table repository-table"><div class="projects-table-head"><span>"Name"</span><span>"Tickets"</span><span>"Repository"</span><span>"Last activity"</span></div>
                {move||board.projects.get().into_iter().filter(|p|archived.get() || !p.archived).map(|p| {
                    let id=p.id;
                    let edit=p.clone();
                    let repo=repository(&p);
                    let activity=p.last_activity_at.clone();
                    let project=github_link(&p);
                    view! { <div class="project-row api-project-row">
                        <div><a class="project-title" href="/board" on:click=move|e|{if !board.select_project(id){e.prevent_default();}}><ProjectIcon name=p.name.clone() icon=p.icon.clone().unwrap_or_default() color=p.color.clone()/><div><strong>{p.name}</strong><p class="mono">{p.key_prefix}{if p.archived {" · Archived"}else{""}}</p></div></a></div>
                        <span class="muted project-counts"><span>{format!("{} open",p.open_ticket_count)}</span><span>{format!("{} done",p.done_ticket_count)}</span></span>
                        <span class="muted project-repository">{repo.map(|repo|view!{<RepositoryLinks repo/>}.into_any()).unwrap_or_else(||view!{<span>"Not linked"</span>}.into_any())}{project.map(|link|view!{<a class="subtle-link github-project-link" href=link.url() target="_blank" rel="noreferrer">{format!("GitHub Project {}",link.number)}<Icon name="external"/></a>})}</span>
                        <span class="muted project-activity">{activity.map(|at|view!{<time datetime=at.clone() title=at.clone()>{at.chars().take(10).collect::<String>()}</time>}.into_any()).unwrap_or_else(||view!{<span>"No activity"</span>}.into_any())}</span>
                        <button class="icon-button project-row-action" aria-label=format!("Edit project {}",edit.name) on:click=move|_|editor.set(Some(Some(edit.clone())))><Icon name="more"/></button>
                    </div> }
                }).collect_view()}
                </div>
                <Show when=move||!board.projects.get().iter().any(|p|archived.get() || !p.archived)><EmptyState title="No active projects" description="Create a project to start adding tickets." action="New project" icon="project" on_action=Callback::new(move|_|editor.set(Some(None)))/></Show>
            </Show>
        </div>
        {let shown=present_value(editor.into());move||shown.get().map(|project|view!{<ProjectEditor project open=Signal::derive(move||editor.get().is_some()) on_close=Callback::new(move|_|editor.set(None))/>})}
    }
}

#[component]
pub(crate) fn ProjectEditor(
    project: Option<Project>,
    open: Signal<bool>,
    on_close: Callback<()>,
    #[prop(default = true)] archive_control: bool,
    #[prop(optional)] restore_project: Option<i64>,
) -> impl IntoView {
    on_cleanup(move || {
        if let Some(id) = restore_project {
            request_animation_frame(move || crate::components::focus_project_action(id));
        }
    });
    let board = expect_context::<BoardState>();
    let fixture = leptos_router::hooks::use_location()
        .pathname
        .get_untracked()
        .starts_with("/design");
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
    let repo = RwSignal::new(project.as_ref().and_then(repository).unwrap_or_default());
    let icon = RwSignal::new(
        project
            .as_ref()
            .and_then(|p| p.icon.clone())
            .unwrap_or_default(),
    );
    let archived = RwSignal::new(project.as_ref().is_some_and(|p| p.archived));
    let saved_owner = project.as_ref().and_then(|p| p.github_owner.clone());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    view! { <Modal open title=if id.is_some(){"Edit project"}else{"New project"} on_close locked=Signal::derive(move||busy.get())>
        <form autocomplete="off" data-1p-ignore="true" data-form-type="other" class="project-form" on:submit=move|e|{
            e.prevent_default();
            if busy.get_untracked(){return;}
            error.set(String::new());
            let link=if owner.get_untracked().trim().is_empty(){Ok(None)}else{ProjectLink::parse(&owner.get_untracked(),"",&kind.get_untracked()).map(Some)};
            let link=match link{Ok(link)=>link,Err(e)=>{error.set(e);return;}};
            let repository=repo.get_untracked().trim().to_string();
            let path=repository.strip_prefix("https://github.com/").or_else(||repository.strip_prefix("github.com/")).unwrap_or(&repository).trim_end_matches('/');
            let (repo_owner,repo_name)=if path.is_empty(){(None,None)}else if let Some((owner,name))=path.split_once('/'){
                if !tickets_core::github::valid_owner(owner) || !tickets_core::github::valid_repo(name){error.set("Enter a GitHub repository URL or owner/repository.".into());return;}
                (Some(owner.to_string()),Some(name.to_string()))
            }else if tickets_core::github::valid_repo(path){
                let Some(owner)=link.as_ref().map(|l|l.owner.clone()).or(saved_owner.clone())else{error.set("Enter a GitHub repository URL or owner/repository.".into());return;};
                (Some(owner),Some(path.to_string()))
            }else{error.set("Enter a GitHub repository URL or owner/repository.".into());return;};
            if let (Some(link),Some(owner))=(&link,&repo_owner) && link.owner!=*owner {error.set("The repository and GitHub Project must use the same owner.".into());return;}
            let body=json!({"name":name.get_untracked().trim(),"key_prefix":prefix.get_untracked().trim(),"color":color.get_untracked(),"icon":if icon.get_untracked().is_empty(){None}else{Some(icon.get_untracked())},"archived":archived.get_untracked(),"github_owner":link.as_ref().map(|l|l.owner.clone()).or(repo_owner),"github_project_number":link.as_ref().map(|l|l.number),"github_project_kind":link.as_ref().map(|l|l.kind.as_str()).unwrap_or(&kind.get_untracked()),"github_repo":repo_name});
            if let Err(e)=tickets_core::api::validate_create("projects",&body){error.set(e);return;}
            if fixture {
                let mut saved=project.clone().expect("fixture editing an existing project");
                saved.name=name.get_untracked().trim().into();saved.key_prefix=prefix.get_untracked().trim().into();saved.color=color.get_untracked();saved.icon=body["icon"].as_str().map(str::to_string);saved.archived=archived.get_untracked();saved.github_owner=body["github_owner"].as_str().map(str::to_string);saved.github_repo=body["github_repo"].as_str().map(str::to_string);
                board.projects.update(|ps|{if let Some(p)=ps.iter_mut().find(|p|p.id==saved.id){*p=saved;}});on_close.run(());return;
            }
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
        <ProjectIconPicker value=icon color name/><fieldset class="project-colors"><legend>"Icon color"</legend><div class="project-swatches">{[("#5e6ad2","Accent","--accent"),("#8b82d9","Feature","--label-feature"),("#eb5757","Bug","--label-bug"),("#4ea7fc","Infrastructure","--label-infra"),("#c6a1b9","Design","--label-design"),("#27a644","Green","--success"),("#f0bf00","Yellow","--warning")].into_iter().map(|(value,label,token)|view!{<button type="button" class="project-swatch" aria-label=format!("{label} color") aria-pressed=move||(color.get()==value).to_string() style=format!("--swatch:var({token})") on:click=move|_|color.set(value.into())/>}).collect_view()}</div></fieldset>
        <TextInput id="project-color" label="Color" placeholder="#RRGGBB" value=color/>
        <TextInput id="project-owner" label="GitHub Project" placeholder="github.com/orgs/example/projects/1 or example/1" value=owner/>
        <Show when=move||{let value=owner.get(); !value.trim().is_empty() && !value.contains("github.com/") && !value.contains("://")}><label class="field">"Owner type"<SelectField label="Owner type" value=Signal::derive(move||kind.get()) options=Signal::derive(move||vec![SelectOption::new("users","User"),SelectOption::new("orgs","Organization")]) on_change=Callback::new(move|next:String|kind.set(next))/></label></Show>
        <TextInput id="project-repo" label="GitHub repository (optional)" placeholder="https://github.com/owner/repo" value=repo/>
        <Show when=move||id.is_some() && archive_control><label class="canceled-toggle"><input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other"  type="checkbox" prop:checked=archived on:change=move|_|archived.update(|v|*v = !*v)/>"Archived"</label><p class="muted">"Archived projects leave the sidebar. Tickets are kept and remain editable."</p></Show>
        <Show when=move||!error.get().is_empty()><p role="alert">{error}</p></Show>
        </div><div class="modal-footer"><button class="button button-ghost" type="button" disabled=busy on:click=move|_|on_close.run(())>"Cancel"</button><BusyButton busy>{if id.is_some(){"Save project"}else{"Create project"}}</BusyButton></div>
        </form>
    </Modal> }
}
