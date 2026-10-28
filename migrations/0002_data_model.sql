CREATE TABLE users (
 id INTEGER PRIMARY KEY CHECK (id = 1),
 username TEXT NOT NULL UNIQUE CHECK (length(trim(username)) > 0),
 password_hash TEXT NOT NULL,
 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE TABLE sessions (
 token_hash TEXT PRIMARY KEY NOT NULL,
 user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
 expires_at TEXT NOT NULL
);
CREATE INDEX sessions_expiry ON sessions(expires_at);
CREATE INDEX sessions_user ON sessions(user_id);
CREATE TABLE api_tokens (
 id INTEGER PRIMARY KEY,
 user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
 token_hash TEXT NOT NULL UNIQUE,
 name TEXT NOT NULL CHECK(length(trim(name)) > 0),
 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
 last_used_at TEXT,
 revoked_at TEXT
);
CREATE INDEX api_tokens_user ON api_tokens(user_id);
CREATE TABLE projects (
 id INTEGER PRIMARY KEY,
 name TEXT NOT NULL CHECK(length(trim(name)) > 0),
 key_prefix TEXT NOT NULL UNIQUE CHECK(length(key_prefix) BETWEEN 1 AND 10 AND key_prefix NOT GLOB '*[^A-Z]*'),
 color TEXT NOT NULL,
 archived INTEGER NOT NULL DEFAULT 0 CHECK(archived IN (0,1)),
 github_owner TEXT,
 github_project_number INTEGER CHECK(github_project_number > 0),
 github_repo TEXT,
 next_ticket_number INTEGER NOT NULL DEFAULT 1 CHECK(next_ticket_number > 0)
);
CREATE TABLE tickets (
 id INTEGER PRIMARY KEY,
 project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
 number INTEGER NOT NULL CHECK(number > 0),
 title TEXT NOT NULL CHECK(length(trim(title)) > 0),
 body TEXT NOT NULL DEFAULT '',
 status TEXT NOT NULL DEFAULT 'backlog' CHECK(status IN ('backlog','todo','in_progress','in_review','done','canceled')),
 priority INTEGER NOT NULL DEFAULT 0 CHECK(priority BETWEEN 0 AND 4),
 assignee TEXT,
 sort_order REAL NOT NULL DEFAULT 0,
 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
 updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
 external_url TEXT,
 external_id TEXT UNIQUE,
 UNIQUE(project_id, number)
);
CREATE INDEX tickets_board ON tickets(project_id,status,sort_order);
CREATE TABLE labels (
 id INTEGER PRIMARY KEY,
 name TEXT NOT NULL UNIQUE CHECK(length(trim(name)) > 0),
 color TEXT NOT NULL
);
CREATE TABLE ticket_labels (
 ticket_id INTEGER NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
 label_id INTEGER NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
 PRIMARY KEY(ticket_id,label_id)
);
CREATE INDEX ticket_labels_label ON ticket_labels(label_id);
CREATE TABLE ticket_dependencies (
 blocker_id INTEGER NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
 blocked_id INTEGER NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
 PRIMARY KEY(blocker_id,blocked_id),
 CHECK(blocker_id <> blocked_id)
);
CREATE INDEX dependencies_blocked ON ticket_dependencies(blocked_id);
-- Atomic enforcement also protects against concurrent requests that read the same graph.
CREATE TRIGGER dependencies_no_cycle BEFORE INSERT ON ticket_dependencies
BEGIN
 SELECT RAISE(ABORT, 'dependency cycle') WHERE EXISTS (
  WITH RECURSIVE reachable(id) AS (
   SELECT NEW.blocked_id UNION
   SELECT d.blocked_id FROM ticket_dependencies d JOIN reachable r ON d.blocker_id = r.id
  ) SELECT 1 FROM reachable WHERE id = NEW.blocker_id
 );
END;
CREATE TRIGGER dependencies_no_update BEFORE UPDATE ON ticket_dependencies
BEGIN
 SELECT RAISE(ABORT, 'delete and reinsert dependencies');
END;
CREATE TABLE comments (
 id INTEGER PRIMARY KEY,
 ticket_id INTEGER NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
 author TEXT NOT NULL CHECK(length(trim(author)) > 0),
 body TEXT NOT NULL,
 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
 updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX comments_ticket ON comments(ticket_id,created_at,id);
-- Allocation happens in INSERT ... SELECT; this trigger advances the counter in
-- the same statement, including when imported tickets supply explicit numbers.
CREATE TRIGGER tickets_advance_number AFTER INSERT ON tickets
BEGIN
 UPDATE projects SET next_ticket_number = max(next_ticket_number, NEW.number + 1)
 WHERE id = NEW.project_id;
END;
