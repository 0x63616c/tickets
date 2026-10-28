pub use tickets_core::domain::{Priority, Status};

/// Display-only extensions to the shared domain types.
pub trait StatusPresentation: Sized {
    const BOARD: [Self; 4];
    const ALL: [Self; 6];
    fn name(self) -> &'static str;
    fn class(self) -> &'static str;
    fn from_name(name: &str) -> Self;
}

impl StatusPresentation for Status {
    const BOARD: [Self; 4] = [Self::Backlog, Self::Todo, Self::InProgress, Self::Done];
    const ALL: [Self; 6] = [
        Self::Backlog,
        Self::Todo,
        Self::InProgress,
        Self::InReview,
        Self::Done,
        Self::Canceled,
    ];
    fn name(self) -> &'static str {
        match self {
            Self::Backlog => "Backlog",
            Self::Todo => "Todo",
            Self::InProgress => "In progress",
            Self::InReview => "In review",
            Self::Done => "Done",
            Self::Canceled => "Canceled",
        }
    }
    fn class(self) -> &'static str {
        match self {
            Self::Backlog => "backlog",
            Self::Todo => "todo",
            Self::InProgress => "progress",
            Self::InReview => "review",
            Self::Done => "done",
            Self::Canceled => "canceled",
        }
    }
    fn from_name(name: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|s| s.name() == name)
            .unwrap_or(Self::Todo)
    }
}

pub trait PriorityPresentation: Sized {
    const ALL: [Self; 5];
    fn name(self) -> &'static str;
    fn class(self) -> &'static str;
}
impl PriorityPresentation for Priority {
    const ALL: [Self; 5] = [
        Self::None,
        Self::Low,
        Self::Medium,
        Self::High,
        Self::Urgent,
    ];
    fn name(self) -> &'static str {
        match self {
            Self::None => "No priority",
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::Urgent => "Urgent",
        }
    }
    fn class(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Urgent => "urgent",
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct Ticket {
    pub id: u32,
    pub title: String,
    pub status: Status,
    pub priority: Priority,
    pub label: &'static str,
    pub comments: u8,
    pub blocked: bool,
    pub description: String,
    pub project: String,
    pub extra_labels: Vec<String>,
    pub added_comments: Vec<String>,
    pub blocked_by: Vec<u32>,
}

pub fn tickets() -> Vec<Ticket> {
    use Priority::*;
    use Status::*;
    [
        (
            24,
            "Remember filters when switching views",
            Backlog,
            Low,
            "Feature",
            0,
            false,
        ),
        (
            25,
            "Import labels from GitHub issues",
            Backlog,
            Medium,
            "Feature",
            2,
            false,
        ),
        (
            26,
            "Add a keyboard shortcut reference",
            Backlog,
            Low,
            "Design",
            0,
            false,
        ),
        (
            27,
            "Archive completed projects",
            Backlog,
            None,
            "Feature",
            0,
            false,
        ),
        (
            8,
            "Group the ticket list by status",
            Todo,
            High,
            "Feature",
            2,
            false,
        ),
        (9, "Create and edit tickets", Todo, High, "Feature", 3, true),
        (
            10,
            "Link projects to GitHub",
            Todo,
            Medium,
            "Feature",
            1,
            false,
        ),
        (
            16,
            "Keep open views in sync",
            Todo,
            Medium,
            "Infrastructure",
            2,
            true,
        ),
        (
            7,
            "Move tickets between board columns",
            InProgress,
            High,
            "Feature",
            3,
            false,
        ),
        (
            6,
            "Define the dark-mode design system",
            InProgress,
            High,
            "Design",
            4,
            false,
        ),
        (
            5,
            "Expose the ticket REST API",
            InProgress,
            High,
            "Infrastructure",
            2,
            false,
        ),
        (
            23,
            "Preserve focus after closing a dialog",
            InProgress,
            Medium,
            "Bug",
            1,
            false,
        ),
        (
            2,
            "Set up the Rust workspace",
            Done,
            High,
            "Infrastructure",
            2,
            false,
        ),
        (
            3,
            "Model tickets and dependencies",
            Done,
            High,
            "Infrastructure",
            1,
            false,
        ),
        (
            4,
            "Add password login and API tokens",
            Done,
            High,
            "Feature",
            3,
            false,
        ),
    ]
    .into_iter()
    .map(
        |(id, title, status, priority, label, comments, blocked)| Ticket {
            id,
            title: title.into(),
            status,
            priority,
            label,
            comments,
            blocked,
            description: if id == 7 { DESCRIPTION.into() } else { format!("{title}.\n\nKeep the change focused and cover the main flow with a regression check.") },
            project: "Tickets".into(),
            extra_labels: vec![],
            added_comments: vec![],
            blocked_by: if id == 7 { vec![3] } else if id == 16 { vec![5, 7] } else if blocked { vec![5] } else { vec![] },
        },
    )
    .collect()
}

pub const DESCRIPTION: &str = "Drag a ticket to another column to change its status. Update the board immediately and keep other open views in sync.\n\n### Acceptance criteria\n- [x] Group tickets by status, with priority and labels visible\n- [x] Offer a **status menu** as an alternative to dragging\n- [ ] Keep keyboard focus on the ticket after a move\n- [ ] Restore the previous status if saving fails\n\nIf a move fails, show `Couldn’t move ticket` with a **Retry** action. Keep the ticket visible in its original column.";
