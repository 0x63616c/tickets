#[cfg(target_arch = "wasm32")]
mod components;
#[cfg(target_arch = "wasm32")]
mod fixtures;
#[cfg(any(target_arch = "wasm32", test))]
mod markdown;
#[cfg(target_arch = "wasm32")]
mod pages;
#[cfg(target_arch = "wasm32")]
mod ui;

#[cfg(target_arch = "wasm32")]
use leptos::prelude::*;
#[cfg(target_arch = "wasm32")]
use leptos_router::{
    components::{Route, Router, Routes},
    path,
};

#[cfg(target_arch = "wasm32")]
#[component]
fn App() -> impl IntoView {
    provide_context(components::DesignState::new());
    view! {
        <Router><components::AppShell>
            <Routes fallback=|| view! { <components::TopBar/><div class="empty-state"><h1>"Page not found"</h1><a class="button button-secondary" href="/">"Back to tickets"</a></div> }>
                <Route path=path!("/") view=pages::Board/>
                <Route path=path!("/design") view=pages::Gallery/>
                <Route path=path!("/design/board") view=pages::Board/>
                <Route path=path!("/design/list") view=pages::List/>
                <Route path=path!("/design/ticket") view=pages::TicketDetail/>
                <Route path=path!("/design/empty") view=pages::Empty/>
                <Route path=path!("/design/projects") view=pages::Projects/>
                <Route path=path!("/login") view=|| view! { <div class="auth-page"><ui::Login/></div> }/>
                <Route path=path!("/settings/tokens") view=|| view! { <div class="auth-page"><ui::Tokens/></div> }/>
            </Routes>
        </components::AppShell></Router>
    }
}

fn main() {
    #[cfg(target_arch = "wasm32")]
    leptos::mount::mount_to_body(App);
}
