//! Runtime-independent domain types shared by the API and CLI.
use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Health {
    pub ok: bool,
    pub version: &'static str,
}

impl Default for Health {
    fn default() -> Self {
        Self {
            ok: true,
            version: env!("CARGO_PKG_VERSION"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_reports_the_workspace_version() {
        let health = Health::default();
        assert!(health.ok);
        assert_eq!(health.version, "0.1.0");
    }
}

pub mod dependency;
pub mod domain;
pub mod markdown;
pub mod ticket_type;

pub mod api;
pub mod auth;
pub mod github;
pub mod search;

pub mod live;

pub mod navigation;

pub mod github_app;
pub mod github_mirror;

pub mod pull_request;

pub mod analytics;

pub mod history;

pub mod pickup;

pub mod github_app_checks;

pub mod mission;

pub mod pipeline;
