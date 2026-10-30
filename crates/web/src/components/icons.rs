use crate::fixtures::{Priority, PriorityPresentation, Status, StatusPresentation};
use leptos::prelude::*;

/// One 16px, 1.5px-stroke icon family. Icons inherit their surrounding text color.
#[component]
pub fn Icon(#[prop(default = "plus")] name: &'static str) -> impl IntoView {
    if name == "logo" {
        return view! { <BrandMark/> }.into_any();
    }
    let path = match name {
        "home" => "m2 7 6-5 6 5v7H9v-4H7v4H2Z",
        "user" => "M11 5a3 3 0 1 1-6 0 3 3 0 0 1 6 0ZM2 14v-1a6 6 0 0 1 12 0v1",
        "appearance" => "M14 8A6 6 0 1 1 8 2a4.5 4.5 0 0 0 6 6Z",
        "lock" => "M4 7V5a4 4 0 0 1 8 0v2M3 7h10v7H3ZM8 10v1",
        "copy" => "M6 6h8v8H6ZM10 6V2H2v8h4",
        "code" => "m5 4-4 4 4 4m6-8 4 4-4 4M9 2 7 14",
        "globe" => "M14 8A6 6 0 1 1 2 8a6 6 0 0 1 12 0ZM2 8h12M8 2c-4 4-4 8 0 12 4-4 4-8 0-12Z",
        "box" => "m8 1 6 3v8l-6 3-6-3V4Zm-6 3 6 3 6-3M8 7v8",
        "bolt" => "M9 1 3 9h4v6l6-8H9Z",
        "star" => "m8 1 2 4 4.5.7-3.3 3.2.8 4.6-4-2.2-4 2.2.8-4.6L1.5 5.7 6 5Z",
        "layers" => "m8 2 7 4-7 4-7-4Zm-7 7 7 4 7-4M1 12l7 4 7-4",
        "search" => "M11 11l3 3M12 7a5 5 0 1 1-10 0 5 5 0 0 1 10 0",
        "board" => "M2 3h4v10H2zM10 3h4v6h-4z",
        "list" => "M5 4h9M5 8h9M5 12h9M2 4h.01M2 8h.01M2 12h.01",
        "project" => "M2 5h5l2-2h5v10H2z",
        "chevron" => "m6 4 4 4-4 4",
        "down" => "m4 6 4 4 4-4",
        "close" => "m4 4 8 8M12 4l-8 8",
        "check" => "m3 8 3 3 7-7",
        "filter" => "M2 4h12M4 8h8M6 12h4",
        "settings" => "M2 4h7m3 0h2M2 12h2m3 0h7M9 2v4M4 10v4",
        "comment" => "M3 2.5h10a1 1 0 0 1 1 1v7a1 1 0 0 1-1 1H7l-4 3v-3H2v-8a1 1 0 0 1 1-1Z",
        "link" => "m6 10 4-4M5 6l-2 2a3 3 0 0 0 5 5l2-2M6 5l2-2a3 3 0 0 1 5 5l-2 2",
        "blocked" => "M13.5 8a5.5 5.5 0 1 1-11 0 5.5 5.5 0 0 1 11 0ZM4 12l8-8",
        "arrow" => "M3 8h10m-4-4 4 4-4 4",
        "tag" => "M2 2h6l6 6-6 6-6-6V2Zm3 3h.01",
        "more" => "M3 8h.01M8 8h.01M13 8h.01",
        "grid" => "M2 2h5v5H2zM10 2h4v5h-4zM2 10h5v4H2zM10 10h4v4h-4z",
        "ticket" => "M3 2.5h10v3a2.5 2.5 0 0 0 0 5v3H3v-3a2.5 2.5 0 0 0 0-5v-3ZM8 5v1M8 8v1M8 11v1",
        "clock" => "M14 8A6 6 0 1 1 2 8a6 6 0 0 1 12 0ZM8 4v4l2 2",
        "external" => "M9 2h5v5M14 2 7 9M6 3H2v11h11v-4",
        "help" => {
            "M14 8A6 6 0 1 1 2 8a6 6 0 0 1 12 0ZM6 5.5a2 2 0 0 1 4 0c0 1.5-2 1.5-2 3M8 11h.01"
        }
        "wifi" => "M2 6a9 9 0 0 1 12 0M4.5 9a5 5 0 0 1 7 0M7.5 12h1",
        "refresh" => "M13 6a5.5 5.5 0 1 0 .3 4M10 6h4V2",
        "offline" => "m2 2 12 12M6 4a9 9 0 0 1 8 2M2 6l1-1M4.5 9a5 5 0 0 1 4-1M7.5 12h1",
        _ => "M8 3v10M3 8h10",
    };
    view! { <svg class="icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d=path/></svg> }.into_any()
}

/// The Tickets mark: a notched ticket stub with a perforation line, hinted to the
/// 16px pixel grid (see crates/web/icons/README.md). Fills with the text color.
#[component]
pub fn BrandMark() -> impl IntoView {
    view! { <svg class="brand-mark" viewBox="0 0 16 16" aria-hidden="true"><path fill="currentColor" fill-rule="evenodd" d="M3 4h10a1 1 0 0 1 1 1v2a1.5 1.5 0 0 0 0 3v2a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1v-2a1.5 1.5 0 0 0 0-3V5a1 1 0 0 1 1-1Zm7 1h1v1h-1Zm0 2h1v1h-1Zm0 2h1v1h-1Zm0 2h1v1h-1Z"/></svg> }
}

#[component]
pub fn StatusIcon(status: Status) -> impl IntoView {
    view! {
        <svg class=format!("icon status-{}", status.class()) viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <circle cx="8" cy="8" r="5.5" stroke="currentColor" stroke-width="1.5" stroke-dasharray=if status == Status::Backlog { "1.5 2" } else { "none" }/>
            {match status {
                Status::InProgress => view! { <path d="M8 4a4 4 0 0 1 0 8Z" fill="currentColor"/> }.into_any(),
                Status::InReview => view! { <circle cx="8" cy="8" r="2" fill="currentColor"/> }.into_any(),
                Status::Done => view! { <circle cx="8" cy="8" r="5.5" fill="currentColor"/><path d="m5 8 2 2 4-4" stroke="var(--bg)" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/> }.into_any(),
                Status::Canceled => view! { <path d="m4 12 8-8" stroke="currentColor" stroke-width="1.5"/> }.into_any(),
                _ => ().into_any(),
            }}
        </svg>
    }
}

#[component]
pub fn PriorityIcon(priority: Priority) -> impl IntoView {
    view! {
        <svg class=format!("icon priority-{}", priority.class()) viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" aria-hidden="true">
            {if priority == Priority::Urgent {
                view! { <rect x="2" y="2" width="12" height="12" rx="2"/><path d="M8 5v4M8 11h.01"/> }.into_any()
            } else if priority == Priority::None {
                view! { <path d="M4 8h8"/> }.into_any()
            } else {
                view! { <path d="M3 12V9"/><path d="M7.5 12V6" opacity=if priority == Priority::Low { "0.2" } else { "1" }/><path d="M12 12V3" opacity=if priority == Priority::High { "1" } else { "0.2" }/> }.into_any()
            }}
        </svg>
    }
}
