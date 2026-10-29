#[cfg(target_arch = "wasm32")]
mod components;
#[cfg(any(target_arch = "wasm32", test))]
mod count;
#[cfg(target_arch = "wasm32")]
mod fixtures;
#[cfg(target_arch = "wasm32")]
mod live;
#[cfg(any(target_arch = "wasm32", test))]
mod login;
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
    provide_context(pages::board::BoardState::new());
    view! {
        <Router><Show when=|| window().location().pathname().unwrap_or_default() == "/login" fallback=|| view! { <components::AppShell>
            <Routes fallback=|| view! { <components::TopBar/><div class="empty-state"><h1>"Page not found"</h1><a class="button button-secondary" href="/">"Back to tickets"</a></div> }>
                <Route path=path!("/") view=pages::ApiBoard/>
                <Route path=path!("/list") view=pages::list::ApiList/>
                <Route path=path!("/t/:key") view=pages::api_detail::ApiDetail/>
                <Route path=path!("/ticket") view=pages::api_detail::ApiDetail/>
                <Route path=path!("/projects") view=pages::projects::ApiProjects/>
                <Route path=path!("/design") view=pages::Gallery/>
                <Route path=path!("/design/board") view=pages::Board/>
                <Route path=path!("/design/list") view=pages::List/>
                <Route path=path!("/design/ticket") view=pages::TicketDetail/>
                <Route path=path!("/design/empty") view=pages::Empty/>
                <Route path=path!("/design/projects") view=pages::Projects/>
                <Route path=path!("/settings/tokens") view=|| view! { <div class="auth-page"><ui::Tokens/></div> }/>
            </Routes>
        </components::AppShell> }><ui::Login/></Show></Router>
    }
}

fn main() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();
    #[cfg(target_arch = "wasm32")]
    leptos::mount::mount_to_body(App);
}
