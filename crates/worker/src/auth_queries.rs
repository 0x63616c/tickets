//! Shared authentication statements, exercised against SQLite and D1.
pub const BEARER: &str = "UPDATE api_tokens SET last_used_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE token_hash=? AND revoked_at IS NULL RETURNING user_id,id AS token_id,name AS actor_label";
pub const SESSION: &str = "SELECT user_id,coalesce(nullif(trim(users.display_name),''),users.username) AS actor_label FROM sessions JOIN users ON users.id=sessions.user_id WHERE token_hash=? AND expires_at>strftime('%Y-%m-%dT%H:%M:%fZ','now')";

pub const ACCOUNT: &str = "SELECT username,display_name FROM users WHERE id=?";
pub const UPDATE_ACCOUNT: &str =
    "UPDATE users SET display_name=? WHERE id=? RETURNING username,display_name";
