# Claim next ticket (TIK-61)

## Research and decision

PostgreSQL [SKIP LOCKED](https://www.postgresql.org/docs/current/sql-select.html#SQL-FOR-UPDATE-SHARE)
lets queue consumers skip rows another transaction owns. D1/SQLite has no such
row-lock primitive. Its serialized writes and [transactional batch API](https://developers.cloudflare.com/d1/worker-api/d1-database/#batch)
provide the needed boundary instead. [SQLite RETURNING](https://www.sqlite.org/lang_returning.html)
returns changed rows but cannot feed another statement as a writable CTE.

[Sidekiq reliable fetching](https://github.com/sidekiq/sidekiq/wiki/Reliability)
keeps unfinished work durable rather than removing it on delivery.
[SQS visibility timeouts](https://docs.aws.amazon.com/AWSSimpleQueueService/latest/SQSDeveloperGuide/sqs-visibility-timeout.html)
provide temporary ownership, heartbeat extensions and automatic retry after a
worker disappears. We use the same lease principle, not an exactly-once execution
promise: after expiry a previous worker must stop, and external work must remain
idempotent (check existing branches/PRs before creating them).

## Atomic boundary

Use TIK-60's `mutation::execute` batch. Its first INSERT obtains the SQLite write
lock before selecting any candidate. An additive context trigger freezes a
transaction-local lease clock, so capacity and expiry cannot cross a time boundary
between snapshot and update. Old Worker context INSERTs remain compatible. Snapshot the highest-ranked eligible ticket
inside that transaction. A conditional UPDATE selects only that snapshotted ID,
rechecks eligibility and cap, sets a random claim ID and authenticated owner,
lease/heartbeat timestamps, and In Progress, then RETURNING supplies the actual
claimed ticket. Append snapshot diffs and clear scratch data in the same batch.
No HTTP preview/read participates in the claim decision. A loser selects the next
eligible ticket or returns 204. Rule writes serialize with claims too.

Read-only preview uses exactly the same selector. The core pure eligibility and
ranking functions are the reference policy; integration tests compare SQL with
it. Stable ID is the final tie breaker. The cap counts each ticket once when it
is In Progress OR has an unexpired claim, including manually started work.
Archived projects are excluded. Open blockers mean neither Done nor Canceled;
open PR includes drafts. Unblocks means outgoing edges to open tickets.

## Rules and lease lifecycle

One workspace JSON rule document plus complete optional project overrides.
Core rejects unknown fields, wrong types, invalid bounds and duplicate labels.
Todo-only and absence of an outstanding claim are fixed safety invariants.
Editable policy: skip blocked, skip open PR, prefer tickets blocking open work,
priority ranking, excluded label names, cap (1–100), lease seconds (60–86400).
Defaults: both skips and both ranking preferences on, waiting-on-calum excluded,
cap 3, lease 1800 seconds. Oldest creation and ID always break ties.

Claim owner comes from the authenticated actor (safe user/token ID, never token
names or secrets). Heartbeat/release require its identity AND the returned random
claim ID. Heartbeats cannot revive an expired lease. They extend from database
now by the current effective rule duration, never shortening an existing lease.
A monotonically increasing event version records even same-millisecond heartbeats.
Release clears ownership and returns In Progress to Todo. If a human/webhook has
already moved the ticket to Review, Done, Canceled or another status, release and
expiry preserve that status. Agents move to Review/Done then release after work.
Stale/wrong-owner operations return 409 without adding history.

Cron every minute expires up to 25 leases per invocation, atomically through the
same mutation path, oldest expiry first; subsequent ticks drain a backlog. It
clears claims and returns only still-In-Progress tickets to Todo, preserving human
or webhook progress. Expired claims remain unavailable until swept. History uses
claim/heartbeat/expiry/release events with before/after lease metadata; no extra
history writer and no changes to analytics triggers.

## API and UI

GET /api/v1/next?project=<id> previews; POST /api/v1/next/claim accepts optional
project in its JSON body. Both return {ticket, reason}, or genuinely empty 204.
POST /api/v1/tickets/{id-or-key}/claim/{heartbeat,release} accepts {claim_id}.
GET/PUT /api/v1/pickup-rules optionally take ?project=<id>; DELETE removes only a
project override. GET returns effective rules, source and project_id. Settings
uses the existing compact controls with workspace/project scope, explicit save,
validation/retry, inheritance and removal of an override. Screenshots use fixtures.

## Migration and verification

0013 is additive: rule table, nullable ticket lease columns and event metadata,
indexes, and INSERT OR IGNORE for waiting-on-calum. Never delete labels. Existing
rows have no claim and existing APIs remain compatible. Recheck migration numbers
against origin/main before PR. Apply through normal CI only, never remotely by hand.

Tests cover all eligibility/ranking rules and override fallback, validation,
actual concurrent SQLite connections (including cap=1), ownership/fencing,
expired-heartbeat rejection, rollback/history and request budgets; local D1 smoke
exercises the Worker transport and cron. Settings checks cover save, override,
reset, failure and responsive layout. Run repository host/wasm and first-run checks.
