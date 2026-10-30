-- Additive claim state; old tickets remain unclaimed. No labels are removed.
CREATE TABLE pickup_rules (
 scope_id INTEGER PRIMARY KEY CHECK(scope_id>=0),
 rules TEXT NOT NULL CHECK(json_valid(rules)),
 updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
-- scope_id=0 is workspace; positive IDs are projects. Projects are archived, not deleted.
INSERT INTO pickup_rules(scope_id,rules) VALUES(0,'{"skip_blocked":true,"skip_open_pr":true,"blockers_first":true,"priority_first":true,"excluded_labels":["waiting-on-calum"],"concurrency_cap":3,"lease_seconds":1800}');
ALTER TABLE tickets ADD COLUMN claimed_by TEXT;
ALTER TABLE tickets ADD COLUMN claim_id TEXT;
ALTER TABLE tickets ADD COLUMN claim_expires_at TEXT;
ALTER TABLE tickets ADD COLUMN claim_heartbeat_at TEXT;
ALTER TABLE tickets ADD COLUMN claim_event TEXT;
ALTER TABLE tickets ADD COLUMN claim_version INTEGER NOT NULL DEFAULT 0;
CREATE INDEX tickets_claim_expiry ON tickets(claim_expires_at) WHERE claimed_by IS NOT NULL;
CREATE INDEX tickets_pickup ON tickets(status,project_id,created_at,id);
INSERT OR IGNORE INTO labels(name,color) SELECT 'waiting-on-calum','#f0bf00' WHERE NOT EXISTS(SELECT 1 FROM labels WHERE name='waiting-on-calum' COLLATE NOCASE);
-- Freeze the lease clock once the mutation batch has acquired its write lock.
-- A trigger preserves the old Worker's positional context INSERT compatibility.
CREATE TABLE ticket_mutation_clock (
 context_id INTEGER PRIMARY KEY REFERENCES ticket_mutation_context(id) ON DELETE CASCADE,
 now TEXT NOT NULL
);
CREATE TRIGGER ticket_mutation_clock_start AFTER INSERT ON ticket_mutation_context BEGIN
 INSERT INTO ticket_mutation_clock VALUES(NEW.id,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
END;
