-- App registration is independent of the existing per-repository webhook.
CREATE TABLE github_app (
 id INTEGER PRIMARY KEY CHECK(id=1),
 app_id INTEGER NOT NULL,
 slug TEXT NOT NULL,
 name TEXT NOT NULL,
 owner_login TEXT NOT NULL,
 client_id TEXT NOT NULL,
 html_url TEXT NOT NULL,
 secrets_ciphertext TEXT NOT NULL,
 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Phase 2 will populate installations and their repository selections.
CREATE TABLE github_app_installations (
 installation_id INTEGER PRIMARY KEY,
 account_login TEXT NOT NULL,
 repository_selection TEXT,
 suspended_at TEXT,
 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
 updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE TABLE github_app_installation_repos (
 installation_id INTEGER NOT NULL REFERENCES github_app_installations(installation_id) ON DELETE CASCADE,
 repo_full_name TEXT NOT NULL,
 PRIMARY KEY(installation_id,repo_full_name)
);
