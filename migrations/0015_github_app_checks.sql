-- Separate check metadata avoids firing PR status/activity triggers on CI updates.
CREATE TABLE github_pull_request_checks (
 repo TEXT NOT NULL,
 number INTEGER NOT NULL,
 head_sha TEXT NOT NULL,
 tickets_check TEXT,
 check_run_id INTEGER,
 ci_status TEXT,
 ci_updated_at TEXT,
 updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
 PRIMARY KEY(repo,number,head_sha)
);
