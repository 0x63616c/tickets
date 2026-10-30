-- Additive audit storage. The analytics tables/triggers in 0011 are untouched.
CREATE TABLE ticket_history (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 subject_id INTEGER NOT NULL REFERENCES ticket_subjects(id),
 ticket_id INTEGER NOT NULL,
 actor_kind TEXT NOT NULL CHECK(actor_kind IN ('user','api_token','webhook','system')),
 actor_id TEXT NOT NULL,
 actor_label TEXT NOT NULL,
 field TEXT NOT NULL CHECK(field IN ('created','deleted','status','priority','assignee','title','description','project','label_added','label_removed','blocker_added','blocker_removed','blocks_added','blocks_removed','claim','heartbeat','expiry','release')),
 old_value TEXT CHECK(old_value IS NULL OR json_valid(old_value)),
 new_value TEXT CHECK(new_value IS NULL OR json_valid(new_value)),
 recorded_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX ticket_history_subject ON ticket_history(subject_id,id DESC);
CREATE UNIQUE INDEX ticket_history_birth ON ticket_history(subject_id) WHERE field='created';
CREATE TRIGGER ticket_history_no_update BEFORE UPDATE ON ticket_history BEGIN
 SELECT RAISE(ABORT,'ticket history is append-only');
END;
CREATE TRIGGER ticket_history_no_delete BEFORE DELETE ON ticket_history BEGIN
 SELECT RAISE(ABORT,'ticket history is append-only');
END;
INSERT INTO ticket_history(subject_id,ticket_id,actor_kind,actor_id,actor_label,field,new_value)
 SELECT s.id,t.id,'system','migration','System','created',json_object('title',t.title,'baseline',json('true'))
 FROM tickets t JOIN ticket_subjects s ON s.ticket_id=t.id;
-- Transaction-local scratch space: written and cleared in the SAME D1 batch.
-- The singleton rejects accidental nesting. Failed batches roll everything back.
CREATE TABLE ticket_mutation_context (
 id INTEGER PRIMARY KEY CHECK(id=1),
 actor_kind TEXT NOT NULL, actor_id TEXT NOT NULL, actor_label TEXT NOT NULL,
 last_subject INTEGER NOT NULL
);
CREATE TABLE ticket_mutation_before (
 subject_id INTEGER PRIMARY KEY, ticket_id INTEGER NOT NULL, snapshot TEXT NOT NULL
);
