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
    pub r#type: Option<String>,
    pub scope: Option<String>,
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
            r#type: Some("feat".into()),
            scope: Some("ui".into()),
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

/// Mission snapshots use relative timestamps so the 48-hour ready queue stays useful.
pub fn mission() -> tickets_core::mission::Mission {
    use serde_json::json;
    let now = js_sys::Date::now();
    let stamp = |minutes: f64| {
        String::from(
            js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(now - minutes * 60_000.))
                .to_iso_string(),
        )
    };
    serde_json::from_value(json!({
        "generated_at":stamp(0.),
        "reports":[{"source":"box","received_at":stamp(2.),"stale":false,"report":{
            "jobs":[{"repo":"tickets","ticket":"TIK-77","branch":"TIK-77-mission-control","kind":"run","state":"running","age":"41m","detail":"Building Mission Control v1"},{"repo":"quantum","ticket":"QUA-120","branch":"QUA-120-rendering","state":"running","age":"12m","detail":"Checking rendering performance"}],
            "needs_calum":[{"title":"Approve GitHub App installation","detail":"Repository access is ready for review","ticket_key":"TIK-70"}],
            "pull_requests":[{"repo":"example/tickets","number":97,"title":"Mission Control status ingest","url":"https://github.com/example/tickets/pull/97","state":"open","ci":"pass","created_at":stamp(41.),"ticket_key":"TIK-77","auto_merge":true},{"repo":"example/quantum","number":42,"title":"Improve renderer startup","url":"https://github.com/example/quantum/pull/42","state":"open","ci":"pending","created_at":stamp(90.),"ticket_key":"QUA-120"},{"repo":"example/apps","number":31,"title":"Panel keyboard shortcuts","url":"https://github.com/example/apps/pull/31","state":"open","ci":"fail","created_at":stamp(140.),"ticket_key":"APP-18"}],
            "merges":[{"repo":"example/tickets","number":96,"title":"Ticket activity and history","url":"https://github.com/example/tickets/pull/96","merged_at":stamp(130.),"ticket_key":"TIK-60"}],
            "builds":[{"app":"World Wide Webb Panel","kind":"testflight","build_number":"130","status":"success","url":"https://github.com/example/apps/actions/runs/1","finished_at":stamp(55.),"title":"Panel 130 ready in TestFlight"},{"app":"Tickets","kind":"deploy","status":"success","url":"https://github.com/example/tickets/actions/runs/2","finished_at":stamp(120.),"title":"Tickets deploy is live"}],
            "blocked":[{"repo":"apps","pr":31,"reason":"Keyboard shortcut checks failed","url":"https://github.com/example/apps/pull/31","since":stamp(80.)}]
        }},{"source":"build-host","received_at":stamp(18.),"stale":true,"report":{}}],
        "waiting_on_calum":[{"id":1,"key":"TIK-79","title":"Approve build notifications","project":"Tickets","status":"todo","updated_at":stamp(40.),"labels":[{"id":1,"name":"waiting-on-calum"}]}],
        "in_review":[],
        "recently_done":[{"id":2,"key":"TIK-60","title":"Ticket activity and history","project":"Tickets","status":"done","updated_at":stamp(130.),"labels":[]}]
    })).expect("Mission fixtures")
}
