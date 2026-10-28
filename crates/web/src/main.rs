#[cfg(target_arch = "wasm32")]
use leptos::prelude::*;
#[cfg(target_arch = "wasm32")]
use leptos_router::{
    components::{A, Route, Router, Routes},
    path,
};

#[cfg(target_arch = "wasm32")]
#[component]
fn App() -> impl IntoView {
    view! {
        <Router>
            <aside><strong>"Tickets"</strong><nav aria-label="Workspace"><A href="/">"All tickets"</A></nav></aside>
            <main>
                <Routes fallback=|| view! { <h1>"Page not found"</h1><a href="/">"Back to tickets"</a> }>
                    <Route path=path!("/") view=|| view! {
                        <header><h1>"All tickets"</h1></header>
                        <section aria-label="Tickets placeholder"><h2>"No tickets"</h2><p>"Create a project to start tracking work."</p></section>
                    }/>
                </Routes>
            </main>
        </Router>
    }
}

fn main() {
    #[cfg(target_arch = "wasm32")]
    leptos::mount::mount_to_body(App);
}
