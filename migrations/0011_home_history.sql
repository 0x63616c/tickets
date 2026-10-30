-- Additive, compatible with the previous Worker. Observation time is NOT PR merge time.
CREATE TABLE analytics_coverage (
 id INTEGER PRIMARY KEY CHECK(id=1),
 started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
INSERT INTO analytics_coverage(id) VALUES(1);
-- A durable identity survives ticket deletion and SQLite row-ID reuse.
CREATE TABLE ticket_subjects (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 ticket_id INTEGER UNIQUE REFERENCES tickets(id) ON DELETE SET NULL
);
CREATE TABLE ticket_events (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 subject_id INTEGER NOT NULL REFERENCES ticket_subjects(id),
 project_id INTEGER,
 kind TEXT NOT NULL CHECK(kind IN ('baseline','recovered_created','recovered_imported','created','imported','status','project','deleted')),
 old_value TEXT, new_value TEXT,
 recorded_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX ticket_events_flow ON ticket_events(kind,recorded_at,project_id,subject_id);
CREATE INDEX ticket_events_subject ON ticket_events(subject_id,id);
INSERT INTO ticket_subjects(ticket_id) SELECT id FROM tickets;
INSERT INTO ticket_events(subject_id,project_id,kind,new_value)
 SELECT s.id,t.project_id,'baseline',t.status FROM tickets t JOIN ticket_subjects s ON s.ticket_id=t.id;
-- Only the local arrival is known; do not backdate today's project or status.
INSERT INTO ticket_events(subject_id,kind,recorded_at)
 SELECT s.id,CASE WHEN t.external_id IS NULL THEN 'recovered_created' ELSE 'recovered_imported' END,t.created_at
 FROM tickets t JOIN ticket_subjects s ON s.ticket_id=t.id;
CREATE TRIGGER analytics_ticket_birth AFTER INSERT ON tickets BEGIN
 INSERT INTO ticket_subjects(ticket_id) VALUES(NEW.id);
 INSERT INTO ticket_events(subject_id,project_id,kind,new_value)
 SELECT id,NEW.project_id,CASE WHEN NEW.external_id IS NULL THEN 'created' ELSE 'imported' END,NEW.status
 FROM ticket_subjects WHERE ticket_id=NEW.id;
END;
CREATE TRIGGER analytics_ticket_update AFTER UPDATE ON tickets BEGIN
 INSERT INTO ticket_events(subject_id,project_id,kind,old_value,new_value)
 SELECT id,NEW.project_id,'status',OLD.status,NEW.status FROM ticket_subjects
 WHERE ticket_id=NEW.id AND OLD.status<>NEW.status;
 INSERT INTO ticket_events(subject_id,project_id,kind,old_value,new_value)
 SELECT id,NEW.project_id,'project',CAST(OLD.project_id AS TEXT),CAST(NEW.project_id AS TEXT)
 FROM ticket_subjects WHERE ticket_id=NEW.id AND OLD.project_id<>NEW.project_id;
END;
CREATE TRIGGER analytics_ticket_delete BEFORE DELETE ON tickets BEGIN
 INSERT INTO ticket_events(subject_id,project_id,kind,old_value)
 SELECT id,OLD.project_id,'deleted',OLD.status FROM ticket_subjects WHERE ticket_id=OLD.id;
END;
-- Accepted snapshots are observations; source_updated_at is version metadata only.
-- Keep link membership from that snapshot, independent of trigger execution order.
CREATE TABLE pull_request_history (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 repo TEXT NOT NULL, number INTEGER NOT NULL,
 kind TEXT NOT NULL CHECK(kind IN ('baseline','observed','state','links')),
 old_state TEXT, state TEXT NOT NULL, source_updated_at TEXT NOT NULL,
 links TEXT NOT NULL,
 recorded_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX pull_request_history_identity ON pull_request_history(repo,number,id);
INSERT INTO pull_request_history(repo,number,kind,state,source_updated_at,links)
 SELECT repo,number,'baseline',state,updated_at,links FROM github_pull_request_events;
CREATE TRIGGER analytics_pr_insert AFTER INSERT ON github_pull_request_events BEGIN
 INSERT INTO pull_request_history(repo,number,kind,state,source_updated_at,links)
 VALUES(NEW.repo,NEW.number,'observed',NEW.state,NEW.updated_at,NEW.links);
END;
CREATE TRIGGER analytics_pr_update AFTER UPDATE ON github_pull_request_events
WHEN OLD.state<>NEW.state OR OLD.links<>NEW.links BEGIN
 INSERT INTO pull_request_history(repo,number,kind,old_state,state,source_updated_at,links)
 VALUES(NEW.repo,NEW.number,CASE WHEN OLD.state<>NEW.state THEN 'state' ELSE 'links' END,OLD.state,NEW.state,NEW.updated_at,NEW.links);
END;
