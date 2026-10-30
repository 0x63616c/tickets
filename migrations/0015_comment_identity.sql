-- Preserve imported bodies/provenance and leave legacy identities unclaimed.
ALTER TABLE comments ADD COLUMN author_kind TEXT NOT NULL DEFAULT 'system' CHECK(author_kind IN ('user','bot','github','system'));
ALTER TABLE comments ADD COLUMN author_id TEXT;
ALTER TABLE comments ADD COLUMN edited_at TEXT;
ALTER TABLE comments ADD COLUMN deleted_at TEXT;
UPDATE comments SET author_kind='github' WHERE author='GitHub' OR body LIKE '<!-- tickets-import:github:%';
CREATE INDEX comments_active_ticket ON comments(ticket_id,id) WHERE deleted_at IS NULL;

-- Extend history without rebuilding its append-only table or CHECK constraint.
-- A shared sequence preserves existing IDs and orders both event sources.
CREATE TABLE comment_events (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 subject_id INTEGER NOT NULL REFERENCES ticket_subjects(id),
 ticket_id INTEGER NOT NULL,
 actor_kind TEXT NOT NULL, actor_id TEXT NOT NULL, actor_label TEXT NOT NULL,
 field TEXT NOT NULL CHECK(field IN ('comment_created','comment_edited','comment_deleted')),
 old_value TEXT, new_value TEXT,
 recorded_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE TABLE history_sequence (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 source TEXT NOT NULL CHECK(source IN ('ticket','comment')),
 event_id INTEGER NOT NULL,
 UNIQUE(source,event_id)
);
INSERT INTO history_sequence(id,source,event_id) SELECT id,'ticket',id FROM ticket_history ORDER BY id;
CREATE TRIGGER history_sequence_ticket AFTER INSERT ON ticket_history BEGIN
 INSERT INTO history_sequence(source,event_id) VALUES('ticket',NEW.id);
END;
CREATE TRIGGER history_sequence_comment AFTER INSERT ON comment_events BEGIN
 INSERT INTO history_sequence(source,event_id) VALUES('comment',NEW.id);
END;
CREATE VIEW unified_ticket_history AS
 SELECT q.id,h.subject_id,h.ticket_id,h.actor_kind,h.actor_id,h.actor_label,h.field,h.old_value,h.new_value,h.recorded_at FROM history_sequence q JOIN ticket_history h ON q.source='ticket' AND q.event_id=h.id
 UNION ALL
 SELECT q.id,h.subject_id,h.ticket_id,h.actor_kind,h.actor_id,h.actor_label,h.field,h.old_value,h.new_value,h.recorded_at FROM history_sequence q JOIN comment_events h ON q.source='comment' AND q.event_id=h.id;
CREATE TRIGGER comment_events_no_update BEFORE UPDATE ON comment_events BEGIN SELECT RAISE(ABORT,'comment history is append-only'); END;
CREATE TRIGGER comment_events_no_delete BEFORE DELETE ON comment_events BEGIN SELECT RAISE(ABORT,'comment history is append-only'); END;
-- The API establishes the actor context in the same atomic batch as the write.
CREATE TRIGGER comments_history_create AFTER INSERT ON comments WHEN EXISTS(SELECT 1 FROM ticket_mutation_context) BEGIN
 INSERT INTO comment_events(subject_id,ticket_id,actor_kind,actor_id,actor_label,field,new_value)
 SELECT s.id,NEW.ticket_id,x.actor_kind,x.actor_id,x.actor_label,'comment_created',json_object('id',NEW.id) FROM ticket_subjects s CROSS JOIN ticket_mutation_context x WHERE s.ticket_id=NEW.ticket_id;
END;
CREATE TRIGGER comments_history_edit AFTER UPDATE OF body ON comments WHEN NEW.body IS NOT OLD.body AND EXISTS(SELECT 1 FROM ticket_mutation_context) BEGIN
 INSERT INTO comment_events(subject_id,ticket_id,actor_kind,actor_id,actor_label,field,old_value,new_value)
 SELECT s.id,NEW.ticket_id,x.actor_kind,x.actor_id,x.actor_label,'comment_edited',json_object('id',NEW.id),json_object('id',NEW.id) FROM ticket_subjects s CROSS JOIN ticket_mutation_context x WHERE s.ticket_id=NEW.ticket_id;
END;
CREATE TRIGGER comments_history_delete AFTER UPDATE OF deleted_at ON comments WHEN OLD.deleted_at IS NULL AND NEW.deleted_at IS NOT NULL AND EXISTS(SELECT 1 FROM ticket_mutation_context) BEGIN
 INSERT INTO comment_events(subject_id,ticket_id,actor_kind,actor_id,actor_label,field,old_value)
 SELECT s.id,NEW.ticket_id,x.actor_kind,x.actor_id,x.actor_label,'comment_deleted',json_object('id',NEW.id) FROM ticket_subjects s CROSS JOIN ticket_mutation_context x WHERE s.ticket_id=NEW.ticket_id;
END;
CREATE INDEX comment_events_subject ON comment_events(subject_id,id DESC);
CREATE TRIGGER history_sequence_no_update BEFORE UPDATE ON history_sequence BEGIN SELECT RAISE(ABORT,'history sequence is append-only'); END;
CREATE TRIGGER history_sequence_no_delete BEFORE DELETE ON history_sequence BEGIN SELECT RAISE(ABORT,'history sequence is append-only'); END;
