//! Shared authentication statements, exercised against SQLite and D1.
pub const BEARER: &str = "UPDATE api_tokens SET last_used_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE token_hash=? AND revoked_at IS NULL RETURNING user_id";
pub const SESSION: &str = "SELECT user_id FROM sessions WHERE token_hash=? AND expires_at>strftime('%Y-%m-%dT%H:%M:%fZ','now')";

pub const ACCOUNT: &str = "SELECT username,display_name FROM users WHERE id=?";
pub const UPDATE_ACCOUNT: &str =
    "UPDATE users SET display_name=? WHERE id=? RETURNING username,display_name";
