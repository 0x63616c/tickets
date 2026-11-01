-- Durable reconciler runs; terminal runs retain their complete event log.
CREATE TABLE pipeline_runs (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 ticket_id INTEGER NOT NULL REFERENCES tickets(id),
 key TEXT NOT NULL,
 repo TEXT,
 intent TEXT NOT NULL DEFAULT 'merge' CHECK(intent IN ('merge','draft','no_merge')),
 state TEXT NOT NULL DEFAULT 'queued' CHECK(state IN ('queued','claimed','implementing','pr_open','checks','fixing','syncing','merging','awaiting_review','stuck','waiting_on_calum','done','canceled')),
 owner TEXT,
 claim_id TEXT,
 pr INTEGER,
 head_sha TEXT,
 attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts >= 0),
 last_error TEXT,
 entered_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
 next_check_at TEXT,
 escalated_at TEXT,
 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
 updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
 version INTEGER NOT NULL DEFAULT 1
);
CREATE UNIQUE INDEX pipeline_runs_active_ticket ON pipeline_runs(ticket_id)
 WHERE state NOT IN ('awaiting_review','done','canceled');
CREATE INDEX pipeline_runs_state ON pipeline_runs(state);
CREATE INDEX pipeline_runs_repo ON pipeline_runs(repo);
CREATE INDEX pipeline_runs_next_check_at ON pipeline_runs(next_check_at);
CREATE TABLE pipeline_run_events (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 run_id INTEGER NOT NULL REFERENCES pipeline_runs(id),
 at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
 actor TEXT NOT NULL,
 from_state TEXT,
 to_state TEXT NOT NULL,
 detail TEXT NOT NULL CHECK(json_valid(detail))
);
CREATE TRIGGER pipeline_run_events_no_update BEFORE UPDATE ON pipeline_run_events
 BEGIN SELECT RAISE(ABORT,'pipeline run events are append-only'); END;
CREATE TRIGGER pipeline_run_events_no_delete BEFORE DELETE ON pipeline_run_events
 BEGIN SELECT RAISE(ABORT,'pipeline run events are append-only'); END;
