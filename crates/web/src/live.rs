//! Scoped subscriptions with cancellation, jittered reconnect and authoritative reconciliation.
use crate::{components::ConnectionState, pages::board::BoardState};
use futures_util::{
    StreamExt,
    future::{AbortHandle, Abortable},
};
use gloo_net::websocket::{Message, futures::WebSocket};
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use tickets_core::live::ChangeEvent;
use wasm_bindgen_futures::spawn_local;
use web_sys::wasm_bindgen::{JsCast, closure::Closure};

pub(crate) fn subscribe(board: BoardState, path: Memo<String>) {
    let dirty = RwSignal::new(false);
    let events = RwSignal::new(BTreeMap::<(String, i64), ChangeEvent>::new());
    let recover = RwSignal::new(false);
    let online = RwSignal::new(window().navigator().on_line());
    let went_offline = window_event_listener(leptos::ev::offline, move |_| online.set(false));
    let request = move || {
        recover.set(true);
        dirty.set(true);
    };
    let went_online = window_event_listener(leptos::ev::online, move |_| {
        online.set(true);
        request();
    });
    // Healthy live sockets already deliver changes. Focus is recovery only.
    let focus = window_event_listener(leptos::ev::focus, move |_| {
        if !matches!(board.connection.get_untracked(), ConnectionState::Connected) {
            request();
        }
    });
    let visibility = Closure::<dyn Fn()>::new(move || {
        if !document().hidden()
            && (dirty.get_untracked()
                || !matches!(board.connection.get_untracked(), ConnectionState::Connected))
        {
            if !matches!(board.connection.get_untracked(), ConnectionState::Connected) {
                recover.set(true);
            }
            dirty.set(true);
        }
    });
    document()
        .add_event_listener_with_callback("visibilitychange", visibility.as_ref().unchecked_ref())
        .expect("visibility listener");
    let visibility = StoredValue::new_local(visibility);
    on_cleanup(move || {
        visibility.with_value(|listener| {
            let _ = document().remove_event_listener_with_callback(
                "visibilitychange",
                listener.as_ref().unchecked_ref(),
            );
        });
        went_offline.remove();
        went_online.remove();
        focus.remove();
    });
    // A single debounce coalesces room broadcasts, reconnects and browser events.
    Effect::new(move |_| {
        let revision = board.live_revision.get();
        if revision > 0 {
            recover.set(true);
            dirty.set(true);
        }
    });
    let scheduled = RwSignal::new(false);
    Effect::new(move |_| {
        let active = [
            "/",
            "/home",
            "/board",
            "/list",
            "/ticket",
            "/projects",
            "/mission",
        ]
        .contains(&path.get().as_str())
            || path.get().starts_with("/t/");
        if dirty.get()
            && active
            && online.get()
            && !document().hidden()
            && board.signed_in.get()
            && !board.busy.get()
            && board.dragged.get().is_none()
            && board.composer.get().is_none()
            && !board.refreshing.get()
            && !scheduled.get()
        {
            scheduled.set(true);
            spawn_local(async move {
                TimeoutFuture::new(350).await;
                if online.get_untracked()
                    && ([
                        "/",
                        "/home",
                        "/board",
                        "/list",
                        "/ticket",
                        "/projects",
                        "/mission",
                    ]
                    .contains(&path.get_untracked().as_str())
                        || path.get_untracked().starts_with("/t/"))
                    && !document().hidden()
                    && !editing()
                    && !board.busy.get_untracked()
                    && board.dragged.get_untracked().is_none()
                    && board.composer.get_untracked().is_none()
                    && !board.refreshing.get_untracked()
                {
                    dirty.set(false);
                    let pending = events.get_untracked().into_values().collect::<Vec<_>>();
                    events.update(|e| e.clear());
                    crate::ui::invalidate_reads();
                    if recover.get_untracked() {
                        recover.set(false);
                        if ["/", "/home"].contains(&path.get_untracked().as_str()) {
                            // Home does not need tickets, but a future Board/List must
                            // not reuse a collection from before the disconnect.
                            board.invalidate_tickets();
                        } else {
                            board.catch_up();
                        }
                        board.catchup_revision.update(|r| *r += 1);
                    } else {
                        board.reconcile(pending);
                    }
                }
                scheduled.set(false);
            });
        }
    });
    let subscriptions = Memo::new(move |_| {
        let path = path.get();
        let projects = board
            .projects
            .get()
            .iter()
            .map(|p| p.id)
            .collect::<Vec<_>>();
        // The store covers the whole workspace, so keep all project rooms across
        // navigation. Otherwise an inactive project's cached tickets become stale.
        let active = online.get() && board.signed_in.get() && !path.starts_with("/design");
        if active { projects } else { vec![] }
    });
    Effect::new(move |_| {
        let ids: Vec<i64> = subscriptions.get();
        let states = Rc::new(RefCell::new(BTreeMap::new()));
        board.connection.set(ConnectionState::Offline);
        for id in ids {
            states.borrow_mut().insert(id, false);
            let states = states.clone();
            let (abort, registration) = AbortHandle::new_pair();
            on_cleanup(move || abort.abort());
            wasm_bindgen_futures::spawn_local(async move {
                let _ = Abortable::new(
                    async move {
                        let mut attempt = 0u32;
                        let mut connected_once = false;
                        loop {
                            board.connection.set(if window().navigator().on_line() {
                                ConnectionState::Reconnecting
                            } else {
                                ConnectionState::Offline
                            });
                            let location = window().location();
                            let scheme = if location.protocol().unwrap_or_default() == "https:" {
                                "wss"
                            } else {
                                "ws"
                            };
                            let url = format!(
                                "{scheme}://{}/api/v1/live/{id}",
                                location.host().unwrap_or_default()
                            );
                            if let Ok(mut socket) = WebSocket::open(&url) {
                                // gloo emits messages only after opening. Await OPEN without sending data.
                                while matches!(
                                    socket.state(),
                                    gloo_net::websocket::State::Connecting
                                ) {
                                    TimeoutFuture::new(25).await;
                                }
                                if matches!(socket.state(), gloo_net::websocket::State::Open) {
                                    states.borrow_mut().insert(id, true);
                                    if states.borrow().values().all(|open| *open) {
                                        board.connection.set(ConnectionState::Connected);
                                        // Close the snapshot-to-subscription gap with two cheap
                                        // rows (auth + version), not another collection download.
                                        if !connected_once && crate::ui::version_changed().await {
                                            request();
                                        }
                                    }
                                    if connected_once {
                                        board.live_revision.update(|r| *r += 1);
                                    }
                                    connected_once = true;
                                    attempt = 0;
                                    while let Some(Ok(message)) = socket.next().await {
                                        if let Message::Text(text) = message
                                            && let Ok(event) =
                                                serde_json::from_str::<ChangeEvent>(&text)
                                            && event.project_id == id
                                        {
                                            events.update(|events| {
                                                events.insert(
                                                    (event.entity.clone(), event.id),
                                                    event,
                                                );
                                            });
                                            dirty.set(true);
                                        }
                                    }
                                }
                            }
                            states.borrow_mut().insert(id, false);
                            board.connection.set(if window().navigator().on_line() {
                                ConnectionState::Reconnecting
                            } else {
                                ConnectionState::Offline
                            });
                            let base = (500u32.saturating_mul(1 << attempt.min(6))).min(30_000);
                            TimeoutFuture::new(
                                base + (js_sys::Math::random() * f64::from(base) * 0.5) as u32,
                            )
                            .await;
                            attempt = attempt.saturating_add(1);
                        }
                    },
                    registration,
                )
                .await;
            });
        }
    });
}

#[component]
pub(crate) fn LiveIndicator() -> impl IntoView {
    let board = expect_context::<BoardState>();
    let shown = RwSignal::new(None::<ConnectionState>);
    Effect::new(move |_| {
        let connection = board.connection.get();
        let (abort, registration) = AbortHandle::new_pair();
        on_cleanup(move || abort.abort());
        if matches!(connection, ConnectionState::Connected) || !window().navigator().on_line() {
            shown.set(Some(connection));
        } else {
            spawn_local(async move {
                let _ = Abortable::new(
                    async move {
                        TimeoutFuture::new(2_000).await;
                        shown.set(Some(connection));
                    },
                    registration,
                )
                .await;
            });
        }
    });
    view! { <span class="live-indicator">{move || shown.get().map(|state|view! { <crate::components::ConnectionIndicator state/> })}</span> }
}

// Preserve native inputs, Markdown drafts and modal editors during reconciliation.
pub(crate) fn editing() -> bool {
    document()
        .query_selector(
            "dialog[open], .comment-form.has-draft, .popup[data-state=open]:not(.tooltip)",
        )
        .ok()
        .flatten()
        .is_some()
        || document().active_element().is_some_and(|el| {
            ["INPUT", "TEXTAREA", "SELECT"].contains(&el.tag_name().as_str())
                || el.has_attribute("contenteditable")
        })
}
