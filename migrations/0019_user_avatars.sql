-- Processed profile photos stay in D1; no external object storage is required.
CREATE TABLE user_avatars (
    user_id INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    content_type TEXT NOT NULL CHECK(content_type IN ('image/jpeg','image/png','image/webp')),
    bytes BLOB NOT NULL CHECK(length(bytes) BETWEEN 1 AND 524288),
    version TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
