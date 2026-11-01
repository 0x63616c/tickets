-- Additive ticket columns: old readers/writers and FTS triggers stay compatible.
ALTER TABLE tickets ADD COLUMN type TEXT CHECK(type IS NULL OR type IN ('feat','fix','chore','refactor','docs','perf','test','ci','build'));
ALTER TABLE tickets ADD COLUMN scope TEXT CHECK(scope IS NULL OR (length(scope) BETWEEN 1 AND 32 AND substr(scope,1,1) GLOB '[a-z0-9]' AND scope NOT GLOB '*[^a-z0-9._/-]*'));

-- Extend the history field CHECK, retaining every event and public sequence ID.
DROP VIEW unified_ticket_history;
CREATE TABLE ticket_history_v2 (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 subject_id INTEGER NOT NULL REFERENCES ticket_subjects(id),
 ticket_id INTEGER NOT NULL,
 actor_kind TEXT NOT NULL CHECK(actor_kind IN ('user','api_token','webhook','system')),
 actor_id TEXT NOT NULL,
 actor_label TEXT NOT NULL,
 field TEXT NOT NULL CHECK(field IN ('created','deleted','status','priority','assignee','title','type','scope','description','project','label_added','label_removed','blocker_added','blocker_removed','blocks_added','blocks_removed','claim','heartbeat','expiry','release')),
 old_value TEXT CHECK(old_value IS NULL OR json_valid(old_value)),
 new_value TEXT CHECK(new_value IS NULL OR json_valid(new_value)),
 recorded_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
INSERT INTO ticket_history_v2 SELECT * FROM ticket_history;
DROP TABLE ticket_history;
ALTER TABLE ticket_history_v2 RENAME TO ticket_history;
CREATE INDEX ticket_history_subject ON ticket_history(subject_id,id DESC);
CREATE UNIQUE INDEX ticket_history_birth ON ticket_history(subject_id) WHERE field='created';
CREATE TRIGGER ticket_history_no_update BEFORE UPDATE ON ticket_history BEGIN
 SELECT RAISE(ABORT,'ticket history is append-only');
END;
CREATE TRIGGER ticket_history_no_delete BEFORE DELETE ON ticket_history BEGIN
 SELECT RAISE(ABORT,'ticket history is append-only');
END;
CREATE TRIGGER history_sequence_ticket AFTER INSERT ON ticket_history BEGIN
 INSERT INTO history_sequence(source,event_id) VALUES('ticket',NEW.id);
END;
CREATE VIEW unified_ticket_history AS
 SELECT q.id,h.subject_id,h.ticket_id,h.actor_kind,h.actor_id,h.actor_label,h.field,h.old_value,h.new_value,h.recorded_at FROM history_sequence q JOIN ticket_history h ON q.source='ticket' AND q.event_id=h.id
 UNION ALL
 SELECT q.id,h.subject_id,h.ticket_id,h.actor_kind,h.actor_id,h.actor_label,h.field,h.old_value,h.new_value,h.recorded_at FROM history_sequence q JOIN comment_events h ON q.source='comment' AND q.event_id=h.id;

-- Backfill is independently repeatable: every update excludes already-typed rows.
-- Capture real migration edits at observation time, retaining prior history.
CREATE TRIGGER ticket_classification_backfill_history AFTER UPDATE OF type,scope,title ON tickets
WHEN OLD.type IS NOT NEW.type OR OLD.scope IS NOT NEW.scope OR OLD.title IS NOT NEW.title BEGIN
 INSERT INTO ticket_history(subject_id,ticket_id,actor_kind,actor_id,actor_label,field,old_value,new_value)
 SELECT s.id,NEW.id,'system','migration','System',f.key,
 json_quote(json_extract(json_object('type',OLD.type,'scope',OLD.scope,'title',OLD.title),'$.'||f.key)),
 json_quote(f.value)
 FROM ticket_subjects s,json_each(json_object('type',NEW.type,'scope',NEW.scope,'title',NEW.title)) f
 WHERE s.ticket_id=NEW.id AND json_extract(json_object('type',OLD.type,'scope',OLD.scope,'title',OLD.title),'$.'||f.key) IS NOT f.value;
END;
-- Split on the FIRST colon only; retain subject casing and later colons.
-- Invalid/unknown prefixes remain untouched (including invalid scopes).
WITH prefixes AS (
 SELECT id, title, lower(substr(title,1,instr(title,':')-1)) prefix,
        ltrim(substr(title,instr(title,':')+1),' ') rest
 FROM tickets WHERE type IS NULL AND instr(title,':')>0
), unbroken AS (
 SELECT *, CASE WHEN substr(prefix,-1)='!' THEN substr(prefix,1,length(prefix)-1) ELSE prefix END head
 FROM prefixes
), parsed AS (
 SELECT *, CASE WHEN instr(head,'(')>0 AND substr(head,-1)=')' THEN substr(head,1,instr(head,'(')-1) ELSE head END kind,
 CASE WHEN instr(head,'(')>0 AND substr(head,-1)=')' THEN lower(trim(substr(head,instr(head,'(')+1,length(head)-instr(head,'(')-1))) END parsed_scope
 FROM unbroken
), valid AS (
 SELECT * FROM parsed WHERE kind IN ('feat','fix','chore','refactor','docs','perf','test','ci','build')
 AND (parsed_scope IS NULL OR (length(parsed_scope) BETWEEN 1 AND 32 AND substr(parsed_scope,1,1) GLOB '[a-z0-9]' AND parsed_scope NOT GLOB '*[^a-z0-9._/-]*'))
)
UPDATE tickets SET type=v.kind,scope=v.parsed_scope,title=v.rest,
 updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') FROM valid v WHERE tickets.id=v.id AND tickets.type IS NULL;

-- Label fallback is for unprefixed titles. Leave unknown prefix-like heads alone.
-- A conventional type token contains letters/digits/_/-; spaces before a later
-- colon ("Grid home: details") are ordinary title text. Lowest label ID wins.
WITH heads AS (
 SELECT id,CASE WHEN instr(title,':')>0 THEN substr(title,1,instr(title,':')-1) END head
 FROM tickets WHERE type IS NULL
), tokens AS (
 SELECT id,head,CASE WHEN instr(head,'(')>0 THEN substr(head,1,instr(head,'(')-1) ELSE rtrim(head,'!') END token
 FROM heads
), unprefixed AS (
 SELECT id FROM tokens WHERE head IS NULL OR token='' OR token GLOB '*[^a-zA-Z0-9_-]*'
)
UPDATE tickets SET type=(SELECT lower(substr(l.name,6)) FROM ticket_labels tl JOIN labels l ON l.id=tl.label_id
 WHERE tl.ticket_id=tickets.id AND lower(l.name) IN ('type:feat','type:fix','type:chore','type:refactor','type:docs','type:perf','type:test','type:ci','type:build') ORDER BY l.id LIMIT 1)
WHERE type IS NULL AND id IN (SELECT id FROM unprefixed) AND EXISTS(SELECT 1 FROM ticket_labels tl JOIN labels l ON l.id=tl.label_id
 WHERE tl.ticket_id=tickets.id AND lower(l.name) IN ('type:feat','type:fix','type:chore','type:refactor','type:docs','type:perf','type:test','type:ci','type:build'));
DROP TRIGGER ticket_classification_backfill_history;
-- Ticket update triggers invalidate caches for changed rows. The new Worker's
-- read-v2 schema salt also invalidates representations when no backfill is needed.
