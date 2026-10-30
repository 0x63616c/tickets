use crate::{
    components::*,
    fixtures::{Priority, Status},
};
use leptos::prelude::*;
use leptos_router::components::A;

#[component]
pub fn Gallery() -> impl IntoView {
    let state = expect_context::<DesignState>();
    let title = RwSignal::new(String::new());
    let selected = RwSignal::new("In progress".to_string());
    view! {
        <TopBar title="Design system"/>
        <div class="gallery"><div class="gallery-heading"><div><h1>"Design system"</h1><p>"The building blocks of Tickets."</p></div><span class="version-label">"Dark / v1"</span></div>
            <nav class="reference-links" aria-label="Design references"><A href="/design/board"><Icon name="board"/>"Board"<Icon name="arrow"/></A><A href="/design/list"><Icon name="list"/>"List"<Icon name="arrow"/></A><A href="/design/ticket"><Icon name="ticket"/>"Ticket detail"<Icon name="arrow"/></A><A href="/design/empty"><Icon name="project"/>"Empty states"<Icon name="arrow"/></A></nav>
            <section class="specimen"><div class="specimen-label"><h2>"Actions"</h2><p>"One clear primary action."</p></div><div class="specimen-content"><Button variant="primary" on_click=Callback::new(move |_| state.new_ticket(Status::Todo))><Icon name="plus"/>"New ticket"</Button><Button on_click=Callback::new(move |_| state.dialog.set(DialogKind::Project))>"New project"</Button><Button variant="ghost" on_click=Callback::new(move |_| state.toast.set("Changes discarded (preview)".into()))>"Cancel"</Button><Button disabled=true>"Save changes"</Button><IconButton label="Search" icon="search" on_click=Callback::new(move |_| state.dialog.set(DialogKind::Command))/></div></section>
            <section class="specimen"><div class="specimen-label"><h2>"Inputs & menus"</h2><p>"Shared keyboard and focus behavior."</p></div><div class="specimen-content form-specimen"><TextInput id="sample-title" label="Ticket title" placeholder="What needs to be done?" value=title/><label class="field">"Status"<Select label="Example status" options=Status::ALL.map(Status::name).to_vec() value=selected/></label></div></section>
            <section class="specimen"><div class="specimen-label"><h2>"Status"</h2><p>"Shape and color, together."</p></div><div class="specimen-content">{Status::ALL.into_iter().map(|status| view! { <span class="icon-label"><StatusIcon status=status/>{status.name()}</span> }).collect_view()}</div></section>
            <section class="specimen"><div class="specimen-label"><h2>"Priority"</h2><p>"A consistent 16px stroke set."</p></div><div class="specimen-content">{Priority::ALL.into_iter().map(|priority| view! { <span class="icon-label"><PriorityIcon priority=priority/>{priority.name()}</span> }).collect_view()}</div></section>
            <section class="specimen"><div class="specimen-label"><h2>"Labels & identity"</h2><p>"Context without extra weight."</p></div><div class="specimen-content"><LabelChip name="Feature"/><LabelChip name="Bug"/><LabelChip name="Infrastructure"/><LabelChip name="Design"/><span class="toolbar-divider"/><Avatar/><Avatar label="Agent" initials="A"/></div></section>
            <section class="specimen"><div class="specimen-label"><h2>"Live updates"</h2><p>"Always know if you're up to date."</p></div><div class="specimen-content"><ConnectionIndicator state=ConnectionState::Connected/><ConnectionIndicator state=ConnectionState::Reconnecting/><ConnectionIndicator state=ConnectionState::Offline/></div></section>
            <section class="specimen"><div class="specimen-label"><h2>"Overlays & feedback"</h2><p>"Keyboard accessible by default."</p></div><div class="specimen-content"><Button on_click=Callback::new(move |_| state.dialog.set(DialogKind::Command))><Icon name="search"/>"Command palette"<Shortcut>"⌘ K"</Shortcut></Button><Button on_click=Callback::new(move |_| state.new_ticket(Status::Todo))>"Open modal"</Button><Button variant="ghost" on_click=Callback::new(move |_| state.toast.set("Ticket moved to In progress".into()))>"Show toast"</Button><span class="shortcut-sample"><Shortcut>"esc"</Shortcut>" to close"</span></div></section>
            <FoundationGallery/><section class="specimen tokens-specimen"><div class="specimen-label"><h2>"Foundations"</h2><p>"Inter Variable · 13px · 4px grid"</p></div><div class="specimen-content"><div class="color-sample"><span style="background:var(--bg)"/>"Canvas"<code>"#000000"</code></div><div class="color-sample"><span style="background:var(--surface)"/>"Surface"<code>"#070708"</code></div><div class="color-sample"><span style="background:var(--border)"/>"Border"<code>"#141416"</code></div><div class="color-sample"><span style="background:var(--text)"/>"Text"<code>"#f7f8f8"</code></div><div class="color-sample"><span style="background:var(--accent)"/>"Accent"<code>"#5e6ad2"</code></div></div></section>
        </div>
    }
}

pub const COMPONENTS: &[&str] = &[
    "pr-check-status",
    "pickup-rules",
    "history",
    "palette",
    "sidebar",
    "user-menu",
    "select",
    "menu",
    "combobox",
    "modal",
    "loader",
    "skeletons",
    "reveal",
    "icons",
    "busy-button",
    "tabs",
    "tooltip",
];
#[component]
pub fn ComponentPreview() -> impl IntoView {
    let params = leptos_router::hooks::use_params_map();
    let query = leptos_router::hooks::use_query_map();
    let name = params.get_untracked().get("name").unwrap_or_default();
    let variant = query.get_untracked().get("variant").unwrap_or_default();
    let name = COMPONENTS
        .iter()
        .copied()
        .find(|n| *n == name)
        .unwrap_or("select");
    view! {<main class="component-stage" data-component=name><h1>"Sample data · "{name}</h1><ComponentSample name variant/></main>}
}
#[component]
fn FoundationGallery() -> impl IntoView {
    view! {<section class="specimen"><div class="specimen-label"><h2>"Interaction foundations"</h2><p>"Isolated previews, keyboard behavior and motion."</p></div><div class="specimen-content">{COMPONENTS.iter().map(|name|view!{<a class="button" rel="external" href=format!("/design/c/{name}")>{*name}</a>}).collect_view()}</div></section>
    <section class="specimen"><div class="specimen-label"><h2>"Loading & identity"</h2><p>"ascii-reveal · skeleton sweep and text decode."</p></div><div class="specimen-content"><LoadingIndicator/><BusyButton busy=Signal::derive(||true)>"Save project"</BusyButton><ProjectIcon name="Tickets" icon="ticket"/><ProjectIcon name="Quantum" icon="box" color="#4ea7fc"/><ProjectIcon name="Apps" icon="🚀"/><ProjectIcon name="Website" color="#27a644"/></div></section>}
}
#[component]
fn ComponentSample(name: &'static str, variant: String) -> impl IntoView {
    let selected = RwSignal::new("In progress".to_string());
    let labels = RwSignal::new(vec!["Feature".to_string()]);
    let checked = RwSignal::new(true);
    let show = RwSignal::new(false);
    let tabs = RwSignal::new("Write".to_string());
    match name {
        "pr-check-status" => view!{<div class="sample-controls"><PrCheckStatus conclusion=Some("success".into())/><PrCheckStatus conclusion=Some("failure".into())/><PrCheckStatus conclusion=Some("neutral".into())/><PrCheckStatus/><PrCheckStatus label="Tickets" conclusion=Some("success".into())/></div>}.into_any(),
        "pickup-rules"=>view!{<super::pickup::PickupSettings fixture=true/>}.into_any(),
        "history"=>view!{<div class="history-preview"><h2>"Activity"</h2><div class="activity-feed">{history_fixture().into_iter().map(|entry|view!{<HistoryEvent entry/>}).collect_view()}</div></div>}.into_any(),
        "palette"=>{let state=expect_context::<DesignState>();state.dialog.set(DialogKind::Command);view!{<CommandPalette/>}.into_any()},
        "sidebar" | "user-menu"=>view!{<Sidebar/>}.into_any(),
        "select"=>view!{<label class="field"><span>"Status"</span><Select label="Example status" options=Status::ALL.map(Status::name).to_vec() value=selected/></label>}.into_any(),
        "menu"=>view!{<DropdownMenu label="Ticket actions"><MenuItem on_select=Callback::new(move|_|{}) shortcut="E"><Icon name="ticket"/>"Edit ticket"</MenuItem><MenuItem on_select=Callback::new(move|_|{}) shortcut="⌘ C"><Icon name="link"/>"Copy link"</MenuItem><MenuSeparator/><MenuItem checked=Signal::derive(move||checked.get()) on_select=Callback::new(move|_|checked.update(|v|*v= !*v))>"Show completed"</MenuItem></DropdownMenu>}.into_any(),
        "combobox"=>view!{<Combobox label="Add label" input_id="label-filter" options=Signal::derive(||options(&["Feature","Bug","Infrastructure","Design"])) selected=labels on_toggle=Callback::new(move|v:String|labels.update(|ls|{if ls.contains(&v){ls.retain(|l|*l!=v);}else{ls.push(v);}}))/>}.into_any(),
        "modal"=>view!{<button class="button" on:click=move|_|show.set(true)>"New ticket"</button><Presence show><TicketComposer project_id=1 status=Status::Todo projects=Signal::derive(||["Tickets","Apps","Quantum"].into_iter().enumerate().map(|(i,name)|tickets_core::domain::Project{ open_ticket_count:0,done_ticket_count:0,last_activity_at:None, id:i as i64 + 1, name:name.into(), key_prefix:"TKT".into(), icon:Some(["ticket","grid","box"][i].into()),color:"#5e6ad2".into(),archived:false,github_owner:None,github_repo:None,github_project_number:None,github_project_kind:"users".into(),next_ticket_number:1 }).collect()) labels=Signal::derive(||["Feature","Bug","Design"].into_iter().enumerate().map(|(i,name)|tickets_core::domain::Label{id:i as i64+1,name:name.into(),color:["#8b82d9","#eb5757","#c6a1b9"][i].into()}).collect()) open=show.into() busy=Signal::derive(||false) error=Signal::derive(String::new) on_submit=Callback::new(move|_:TicketDraft|show.set(false)) on_close=Callback::new(move|_|show.set(false))/></Presence>}.into_any(),
        "loader"=>view!{<div class="sample-controls"><div class="loading-caption"><LoadingIndicator/>"Loading tickets…"</div></div>}.into_any(),
        "reveal"=>view!{<RevealPreview/>}.into_any(),
        "skeletons"=>{let layout=match variant.as_str(){"list"=>"list","detail"=>"detail","projects"=>"projects",_=>"board"};view!{<LoadingSkeleton layout/>}.into_any()},
        "icons"=>view!{<div class="sample-controls"><ProjectIcon name="Tickets" icon="ticket" size="large"/><ProjectIcon name="Quantum" icon="box" color="#4ea7fc" size="large"/><ProjectIcon name="Apps" icon="🚀" size="large"/><ProjectIcon name="Website" color="#27a644" size="large"/></div>}.into_any(),
        "busy-button" if variant=="cycle"=>view!{<BusyPreview/>}.into_any(),
        "busy-button"=>view!{<div class="sample-controls"><BusyButton busy=Signal::derive(||true)>"Create ticket"</BusyButton><BusyButton variant="secondary" busy=Signal::derive(||true)>"Save changes"</BusyButton><Button variant="primary">"Create ticket"</Button></div>}.into_any(),
        "tabs"=>view!{<SegmentedControl label="Description mode" options=vec!["Write","Preview"] value=tabs/>}.into_any(),
        _=>view!{<IconButton label="Copy ticket link" icon="link" on_click=Callback::new(|_|{})/>}.into_any(),
    }
}

/// Fixture delay only: data always arrives without an artificial delay in real routes.
#[component]
fn RevealPreview() -> impl IntoView {
    let pending = RwSignal::new(true);
    Effect::new(move |_| {
        if pending.get() {
            let timer = set_timeout_with_handle(
                move || pending.set(false),
                std::time::Duration::from_millis(1200),
            )
            .ok();
            on_cleanup(move || {
                if let Some(timer) = timer {
                    timer.clear();
                }
            });
        }
    });
    view! {<div class="reveal-preview"><Show when=move||!pending.get() fallback=||view!{<div role="status" aria-label="Loading preview"><AsciiSkeleton lines=3/></div>}><AsciiReveal><h2>"Tickets are ready"</h2><p>"Review the board, open a ticket, and keep work moving."</p><button class="button button-secondary" on:click=move|_|pending.set(true)>"Replay loading"</button></AsciiReveal></Show></div>}
}

#[component]
fn BusyPreview() -> impl IntoView {
    let busy = RwSignal::new(false);
    Effect::new(move |_| {
        if busy.get() {
            let timer = set_timeout_with_handle(
                move || busy.set(false),
                std::time::Duration::from_millis(800),
            )
            .ok();
            on_cleanup(move || {
                if let Some(timer) = timer {
                    timer.clear();
                }
            });
        }
    });
    view! {<BusyButton busy kind="button" on_click=Callback::new(move |_| busy.set(true))>"Save changes"</BusyButton>}
}
