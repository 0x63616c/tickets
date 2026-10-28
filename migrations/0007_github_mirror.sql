CREATE TABLE github_mirror_cursors (
 repo_id INTEGER NOT NULL,
 stream TEXT NOT NULL CHECK(stream IN ('issues','comments')),
 since TEXT NOT NULL DEFAULT '1970-01-01T00:00:00Z',
 high_water TEXT NOT NULL DEFAULT '1970-01-01T00:00:00Z',
 page INTEGER NOT NULL DEFAULT 1,
 etag TEXT,
 PRIMARY KEY(repo_id,stream)
);
-- The importer uses the public comment API. Enforce marker identity in SQLite
-- too, so a concurrent webhook/import cannot create a second source comment.
CREATE UNIQUE INDEX comments_github_marker ON comments(
 ticket_id, substr(body,1,instr(body,' -->')+3)
) WHERE body LIKE '<!-- tickets-import:github:%' AND instr(body,' -->')>0;
