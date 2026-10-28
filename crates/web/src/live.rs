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

pub(crate) fn subscribe(board: BoardState, path: Memo<String>) {
    let dirty = RwSignal::new(false);
    let online = RwSignal::new(window().navigator().on_line());
    let went_offline = window_event_listener(leptos::ev::offline, move |_| online.set(false));
    let went_online = window_event_listener(leptos::ev::online, move |_| online.set(true));
    on_cleanup(move || {
        went_offline.remove();
        went_online.remove();
    });
    Effect::new(move |_| {
        let revision = board.live_revision.get();
        if revision > 0 {
            dirty.set(true);
        }
    });
    Effect::new(move |_| {
        if dirty.get() && !board.busy.get() && board.dragged.get().is_none() && !board.loading.get()
        {
            dirty.set(false);
            board.refresh();
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
        let project = board.project.get();
        let active = online.get()
            && board.signed_in.get()
            && ["/", "/list", "/ticket"].contains(&path.as_str());
        if !active {
            vec![]
        } else if path == "/list" {
            projects
        } else {
            projects.into_iter().filter(|id| *id == project).collect()
        }
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
                                    }
                                    board.live_revision.update(|r| *r += 1);
                                    attempt = 0;
                                    while let Some(Ok(message)) = socket.next().await {
                                        if let Message::Text(text) = message
                                            && let Ok(event) =
                                                serde_json::from_str::<ChangeEvent>(&text)
                                            && event.project_id == id
                                        {
                                            board.live_revision.update(|r| *r += 1);
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
    view! { {move || view! { <crate::components::ConnectionIndicator state=board.connection.get()/> }} }
}
