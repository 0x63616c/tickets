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

pub mod api;
pub mod auth;
pub mod github;
