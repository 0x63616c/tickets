# Ticket history (TIK-60)

## Research and write-path inventory

- Web detail, board ordering/status, list properties, composer and command palette
  use `ui::api` → authenticated `/api/v1` → `api::handle_as` → `ApiRepository`.
- REST POST/PATCH/DELETE and external-ID PUT write tickets; relationship routes
  write `ticket_labels` and `ticket_dependencies`. Deleting labels/tickets also
  removes relationships through foreign-key cascades.
- `repository::Repository` exposes an older typed write surface used by tests.
  It must use the same executor, not remain an unlogged alternate path.
- `xtask/import_github.rs` uses external-ID PUT, fixed-value PATCH, label and
  dependency REST routes. It does not access D1. Reruns must emit only real diffs.
- No MCP server or MCP mutation implementation exists in this repository (searched
  the entire tracked source). Agents currently use the Tickets REST skill. A future
  MCP adapter must call authenticated dispatch, carrying the token actor.
- Signed GitHub PR webhook → `github_prs` → `pull_request::UPSERT` → migration
  0008 link/status triggers. Wrap that statement in the common executor, including
  all old and newly referenced tickets; preserve replay/stale-event guards.
- Migration 0002 maintains numbering, dependency cycles and FTS. Migration 0011
  creates durable `ticket_subjects` and analytics `ticket_events` triggers. Those
  remain unchanged, including PR observation triggers. Retired issue webhooks do
  not write. Comments/account/token/project metadata do not change ticket fields.

## Reference and presentation

[Linear comments and activity](https://linear.app/docs/comment-on-issues) shows compact
actor/action/time rows alongside comment bodies; [issue history](https://linear.app/docs/creating-issues) separates compact
property activity from description history. [GitHub timeline events](https://docs.github.com/en/rest/issues/timeline)
carry actor, event kind, timestamp and rename old/new values. GitHub’s
[Primer timeline component](https://primer.style/product/components/) uses compact
event glyphs with inline actor/action text. Use the existing
Activity section: small event glyph, actor/action/value text, relative time with
exact UTC on click/tap or keyboard activation; expandable old/new description text. Keep comments editable
and visually distinct, and show newest changes first with an explicit older-page
control. Unlike Linear's initial grace period, record every committed change.

## Schema and actors

Add migration 0012 (recheck main before commit): `ticket_history`, indexed by
`(subject_id,id DESC)`, immutable under UPDATE/DELETE triggers. Durable subject FK
survives deletion and ticket-ID reuse; store original numeric ticket ID as a
snapshot, without a cascading FK. Existing tickets receive exactly one `created`
entry at migration time, attributed to system/migration, not their historical
creation time. No other retrospective events are inferred.

Each row stores actor kind (`user`, `api_token`, `webhook`, `system`), stable actor
ID, safe display label, field, nullable JSON old/new values and UTC observation
time. Authentication supplies numeric user/token IDs with `Owner` / `API token #N`
labels, never names, email addresses, credential hashes or caller-supplied actor
metadata. Signed PR ingestion supplies `webhook/github/GitHub`. Internal typed
repository writes use `system/repository/System`. Claim, heartbeat, expiry and
release kinds are reserved for TIK-61; this change adds no claim lifecycle.

## One atomic mutation path

`mutation::execute` is the only Worker path for ticket-affecting SQL, including
cascades and PR trigger transitions. One D1 batch: establish context, snapshot the
bounded affected subjects, run the domain statement, INSERT actual before/after
diffs, clear scratch rows/context. D1 serializes and rolls back the entire batch;
SQLite test adapters use an actual transaction. No application read/modify/write
gap is used for history. Core owns field/set-diff SQL and its reference diff.

Scratch tables are empty after commit; a singleton context fails closed on
nested execution. Scope selectors cover a ticket and adjacent dependencies, an
external ID, attached labels, or PR old/new links. New subjects are identified by
the pre-batch subject high-water mark. Unchanged values and replayed upserts emit
nothing. New tickets get only `created` (initial snapshot), not a row per default.
Deletes get `deleted`; surviving neighbors get relationship removals. Labels and
blockers retain ID and display snapshots. Reordering is not a requested history
field. All generated statements remain within 100 bindings and a mutation batch
counts as one Store round trip. JSON field tables keep the diff within D1’s
five-term compound SELECT limit; host tests apply that limit too.

Old Worker SQL remains valid against the additive schema. It does not know this
new audit executor, so changes during the migration/deployment overlap cannot be
attributed or reconstructed; audit coverage begins when the new Worker serves
writes. Analytics coverage is uninterrupted. No remote/manual migration.

## API and live behavior

`GET /api/v1/tickets/{id-or-key}/history?limit=50&cursor=<last-id>` returns
`{data:[{id,subject_id,ticket_id,actor_kind,actor_id,actor_label,field,old_value,
new_value,recorded_at}],next_cursor}`. IDs descend; exclusive cursor, 1–100 limit,
null at end. Only current ticket identities resolve: deleted rows remain retained
for audit, and reused numeric IDs cannot reveal a previous subject's history.
No history write API. Existing project-room invalidations fetch newer pages until
they overlap the loaded window, then merge immutable entries and keep the oldest
cursor and expanded rows. Older pages load on demand. Failure keeps saved activity
visible. Shared comment/event timestamps refresh each minute and expose exact UTC
via keyboard or touch.
