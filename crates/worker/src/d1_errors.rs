//! Classify binding failures at the D1 boundary; never expose upstream SQL or values.
use crate::repository::Error;

pub const MESSAGE: &str =
    "Database temporarily unavailable or at capacity. Please try again later.";
pub const MARKER: &str = "D1_UNAVAILABLE";

/// Cloudflare documents availability failures as detailed D1_ERROR messages.
/// Keep SQL/type/constraint failures as internal errors (or existing validation).
pub fn unavailable(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    if [
        "constraint failed",
        "syntax error",
        "d1_type_error",
        "d1_column_notfound",
    ]
    .iter()
    .any(|part| message.contains(part))
    {
        return false;
    }
    [
        "quota exceeded",
        "daily row read limit",
        "daily row write limit",
        "storage limit",
        "memory limit",
        "cpu time limit",
        "response size limit",
        "limit exceeded",
        "overloaded",
        "unavailable",
        "too many requests",
        "too many api requests",
        "too many sql variables",
        "too many sql statements",
        "internal error",
        "timeout",
        "timed out",
        "transient",
        "reset",
        "network connection lost",
        "replica disconnected",
        "client disconnected",
        "exceeded maximum db size",
    ]
    .iter()
    .any(|part| message.contains(part))
}

/// Only extract the constant, because the rest can contain SQL and caller data.
pub fn code(message: &str) -> &str {
    message
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .find(|part| {
            matches!(
                *part,
                "D1_ERROR" | "D1_EXEC_ERROR" | "D1_TYPE_ERROR" | "D1_COLUMN_NOTFOUND"
            )
        })
        .unwrap_or("D1_ERROR")
}

/// worker-rs D1Error::Display prints only `cause`, which can be undefined on
/// current bindings. Read the primary JS message as well as the legacy cause.
#[cfg(target_arch = "wasm32")]
pub fn binding(error: worker::Error) -> Error {
    if let worker::Error::D1(error) = error {
        let inner: &js_sys::Error = error.as_ref();
        let message: String = inner.message().into();
        storage(format!("{message}: {}", error.cause()))
    } else {
        // all()/bind() use worker-rs's generic JS error, whose Display also
        // omits causes. Walk its bounded source chain for legacy bindings.
        let mut message = error.to_string();
        let mut cause = std::error::Error::source(&error);
        while let Some(error) = cause {
            message.push_str(": ");
            message.push_str(&error.to_string());
            cause = error.source();
        }
        storage(message)
    }
}

pub fn storage(message: impl ToString) -> Error {
    let message = message.to_string();
    #[cfg(target_arch = "wasm32")]
    worker::console_error!(
        "D1 failure code={} unavailable={}",
        code(&message),
        unavailable(&message)
    );
    if unavailable(&message) {
        Error::Unavailable
    } else {
        Error::Storage(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn availability_errors_preserve_no_upstream_details() {
        for message in [
            "D1_ERROR: Your account has exceeded D1's free tier daily row read limit.",
            "D1_ERROR: Your account has exceeded D1's free tier daily row write limit.",
            "D1_ERROR: Your account has exceeded D1's maximum account storage limit",
            "D1_ERROR: Exceeded maximum DB size.",
            "D1_ERROR: D1 DB is overloaded. Requests queued for too long.",
            "D1_ERROR: D1 DB is overloaded. Too many requests queued.",
            "D1_ERROR: Cannot resolve D1 DB due to transient issue on remote node.",
            "D1_ERROR: D1 DB storage operation exceeded timeout which caused object to be reset.",
            "D1_ERROR: Internal error while starting up D1 DB storage caused object to be reset.",
            "D1_ERROR: D1 DB reset because its code was updated.",
            "D1_ERROR: D1 DB's isolate exceeded its memory limit and was reset.",
            "D1_ERROR: D1 DB exceeded its CPU time limit and was reset.",
            "D1_ERROR: Network connection lost.",
            "D1_ERROR: Replica disconnected from primary.",
            "D1_ERROR: Database unavailable",
            "D1_ERROR: quota exceeded",
        ] {
            assert_eq!(storage(message), Error::Unavailable, "{message}");
            assert_eq!(storage(message).to_string(), MARKER);
            assert_eq!(code(message), "D1_ERROR");
        }
        for message in [
            "D1_EXEC_ERROR: syntax error",
            "D1_ERROR: near LIMIT: syntax error",
            "D1_TYPE_ERROR: wrong type",
            "D1_COLUMN_NOTFOUND: name",
            "UNIQUE constraint failed",
            "D1_ERROR: UNIQUE constraint failed: reset.quota",
            "D1_TYPE_ERROR: unavailable column",
            "FOREIGN KEY constraint failed",
        ] {
            assert_eq!(storage(message), Error::Storage(message.into()));
        }
        assert_eq!(code("Error: D1_TYPE_ERROR: private data"), "D1_TYPE_ERROR");
    }
}
