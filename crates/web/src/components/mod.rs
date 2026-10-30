mod ascii_reveal;
pub use ascii_reveal::*;
mod composer;
pub use composer::*;
mod feedback;
mod motion;
mod popup;
pub use feedback::*;
pub use motion::*;
pub use popup::*;
mod controls;
mod ticket_hit;
pub(crate) use ticket_hit::*;
mod icons;
mod overlays;
mod palette;
pub use palette::CommandPalette;
mod shell;
pub use crate::fixtures::{PriorityPresentation, StatusPresentation};
pub use controls::*;
pub use icons::*;
pub use overlays::*;
pub use shell::*;

use crate::fixtures::{self, Status, Ticket};
use leptos::prelude::*;

#[derive(Clone, Copy, PartialEq)]
pub enum DialogKind {
    None,
    Ticket,
    Project,
    Dependency,
    Command,
}

/// In-memory fixtures only. Replace this context with API resources in #7–#10.
#[derive(Clone, Copy)]
pub struct DesignState {
    pub tickets: RwSignal<Vec<Ticket>>,
    pub dialog: RwSignal<DialogKind>,
    pub toast: RwSignal<String>,
    pub projects: RwSignal<Vec<String>>,
    pub new_status: RwSignal<Status>,
    pub dragged: RwSignal<Option<u32>>,
    pub selected_ticket: RwSignal<u32>,
    pub current_project: RwSignal<String>,
}
impl DesignState {
    pub fn new() -> Self {
        Self {
            tickets: RwSignal::new(fixtures::tickets()),
            dialog: RwSignal::new(DialogKind::None),
            toast: RwSignal::new(String::new()),
            projects: RwSignal::new(vec![]),
            new_status: RwSignal::new(Status::Todo),
            dragged: RwSignal::new(None),
            selected_ticket: RwSignal::new(7),
            current_project: RwSignal::new("Tickets".into()),
        }
    }
    pub fn new_ticket(self, status: Status) {
        self.new_status.set(status);
        self.dialog.set(DialogKind::Ticket);
    }
    pub fn move_ticket(self, id: u32, status: Status) {
        use web_sys::wasm_bindgen::JsCast;
        let selector = format!("button[role=combobox][aria-label='Move TKT-{id} to status']");
        let restore_focus = document()
            .active_element()
            .is_some_and(|element| element.matches(&selector).unwrap_or(false));
        self.tickets.update(|tickets| {
            if let Some(ticket) = tickets.iter_mut().find(|t| t.id == id) {
                ticket.status = status;
            }
        });
        self.toast
            .set(format!("TKT-{id} moved to {}", status.name()));
        if restore_focus {
            request_animation_frame(move || {
                if let Ok(Some(element)) = document().query_selector(&selector)
                    && let Some(element) = element.dyn_ref::<web_sys::HtmlElement>()
                {
                    let _ = element.focus();
                }
            });
        }
    }
}

mod history;
pub use history::*;
