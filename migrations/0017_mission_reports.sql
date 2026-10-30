-- Latest source snapshots; additive and compatible with the previous Worker.
CREATE TABLE mission_reports (
 source TEXT PRIMARY KEY,
 body TEXT NOT NULL,
 received_at TEXT NOT NULL
);
