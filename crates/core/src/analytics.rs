//! Home v1 contract: current inventory and observed flow, never timestamp proxies.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Home {
    pub as_of: String,
    pub history_from: String,
    pub projects: Vec<HomeProject>,
    pub in_flight: u64,
    pub in_review: u64,
    pub blocked: u64,
    pub weeks: Vec<Week>,
    pub review_queue: Vec<HomeTicket>,
    pub blocked_queue: Vec<HomeTicket>,
    pub cohort: Vec<HomeTicket>,
    pub cohort_total: u64,
    pub next_cursor: Option<i64>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HomeProject {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub icon: Option<String>,
    pub backlog: u64,
    pub todo: u64,
    pub in_progress: u64,
    pub in_review: u64,
}
impl HomeProject {
    pub fn total(&self) -> u64 {
        self.backlog + self.todo + self.in_progress + self.in_review
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Week {
    pub start: String,
    pub end: String,
    /// none, partial (capture started mid-week), or full.
    pub coverage: String,
    pub created: Option<u64>,
    pub completed: Option<u64>,
    pub imported: Option<u64>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HomeTicket {
    pub subject_id: i64,
    pub ticket_id: Option<i64>,
    pub key: Option<String>,
    pub title: Option<String>,
    pub status: Option<String>,
    pub blocker_id: Option<i64>,
    pub blocker_key: Option<String>,
    pub blocker_count: u64,
}
