---
name: tickets
description: Drive Tickets through its REST API using a personal token; manage projects, tickets, labels, dependencies, comments and imports.
---

# Tickets

Read `TICKETS_URL` (instance base URL, no trailing slash) and `TICKETS_TOKEN`
(personal API token) from the caller's secure environment. Obtain access from the
owner via Settings → API tokens; do not ask for their password. Never hardcode,
log, commit or put credentials in URLs. Disable shell tracing when using curl.
Bearer mutations need no Origin header. A 401 means missing/invalid/revoked
access: stop and request replacement access, rather than retrying.

Use [the API reference](../../../docs/API.md) for full fields, defaults, routes
and authentication details. Examples below form a disposable walkthrough, **not
an instruction to seed a live workspace**. For real work, list existing resources
and substitute their returned IDs/keys, cursor and desired fields. Execute only
the operations needed for the requested task. The walkthrough assumes an empty
instance (projects 1/2, tickets 1/2, label 2/comment 1; label 1 is the seeded
waiting-on-calum label). HTTP expectations and JSON subsets in comments are checked by `cargo xtask skill-check` against local D1.

For GitHub App setup, the owner visits `/github/app/register` in a signed-in
browser (`?name=alt` uses the fallback app name); the Worker supplies the manifest
and setup state. See the API reference’s GitHub App registration section.

## Access and project selection

`GET /api/v1/me` also returns `username` and nullable `display_name`.
Update the account label with `PATCH /api/v1/me` and only `display_name`
(trimmed text, at most 80 characters; null clears it). This never changes login
credentials. Project reads include `open_ticket_count`, excluding done/canceled,
`done_ticket_count` (done only), and nullable `last_activity_at` (latest ticket
update across all statuses). Repository links use `github_owner`/`github_repo`;
`github_project_number` is optional and independent of repository linking.

Check access, list projects and choose an appropriate unarchived project. Create
one only if needed; prefixes are 1–10 uppercase ASCII letters, colors `#RRGGBB`.
Link with `github_owner`, positive `github_project_number`, `github_project_kind`
(`users` default or `orgs`), and optional `github_repo` (repository name).
Set optional `icon` to a named glyph (`project`, `ticket`, `board`, `grid`, `code`,
`globe`, `box`, `bolt`, `star`, `layers`) or a short emoji (up to 16 Unicode scalars).
PATCH `icon: null` restores the colored initial; omission preserves it.
Archive/restore with PATCH `archived`; tickets are retained. Clear nullable GitHub
fields with null. Linking stores metadata only and never calls GitHub.

```sh
# expect {"status":200,"body":{"authenticated":true}}
curl -sS "$TICKETS_URL/api/v1/me" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":200,"body":{"data":[],"next_cursor":null}}
curl -sS "$TICKETS_URL/api/v1/projects" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":201,"body":{"id":1,"key_prefix":"EX"}}
curl -sS "$TICKETS_URL/api/v1/projects" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"name":"Example","key_prefix":"EX","color":"#789abc"}'
# expect {"status":201,"body":{"id":2}}
curl -sS "$TICKETS_URL/api/v1/projects" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"name":"Destination","key_prefix":"DST","color":"#789abc"}'
# expect {"status":200,"body":{"github_owner":"example-org","github_project_kind":"orgs","github_project_number":7,"github_repo":"example"}}
curl -sS -X PATCH "$TICKETS_URL/api/v1/projects/1" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"github_owner":"example-org","github_project_kind":"orgs","github_project_number":7,"github_repo":"example"}'
```

## Project icon round trip

```sh
# expect {"status":200,"body":{"icon":"box"}}
curl -sS -X PATCH "$TICKETS_URL/api/v1/projects/1" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"icon":"box"}'
# expect {"status":200,"body":{"icon":"box"}}
curl -sS "$TICKETS_URL/api/v1/projects/1" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":200,"body":{"icon":null}}
curl -sS -X PATCH "$TICKETS_URL/api/v1/projects/1" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"icon":null}'
# expect {"status":422}
curl -sS -X PATCH "$TICKETS_URL/api/v1/projects/1" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"icon":"unknown"}'
```

## Create, read, update and move tickets

Creation requires `project_id` and nonblank `title`. PATCH changes only supplied
fields; null clears nullable fields. Unknown mutation fields are rejected.
Status: `backlog` (default), `todo`, `in_progress`, `in_review`, `done`, `canceled`.
Priority: 0 none, 1 urgent, 2 high, 3 medium, 4 low. Use numeric IDs or keys for
individual tickets. A project move assigns a new destination number; the old key
stops resolving. Keep the returned ID/key for subsequent requests.

```sh
# expect {"status":201,"body":{"id":1,"key":"EX-1","status":"backlog"}}
curl -sS "$TICKETS_URL/api/v1/tickets" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"project_id":1,"title":"Render bug","body":"**Details**","priority":2}'
# expect {"status":201,"body":{"id":2,"key":"EX-2"}}
curl -sS "$TICKETS_URL/api/v1/tickets" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"project_id":1,"title":"Follow-up","status":"todo"}'
# expect {"status":200,"body":{"id":1,"title":"Render bug","status":"backlog","pull_requests":[]}}
curl -sS "$TICKETS_URL/api/v1/tickets/EX-1" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":200,"body":{"status":"in_progress","assignee":"agent"}}
curl -sS -X PATCH "$TICKETS_URL/api/v1/tickets/1" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"title":"Render bug fix","status":"in_progress","assignee":"agent"}'
```

## Labels and dependencies

List labels before creating one. Attach/detach using the label ID. Dependencies
point **blocker → blocked**: ticket 1 blocks ticket 2 below. `blocks` and
`blocked_by` contain IDs. A 409 on addition means a cycle, self-edge or duplicate;
inspect the graph/error and correct the edge instead of retrying. A missing link
on deletion returns 404. A ticket is blocked while any incoming blocker is
neither done nor canceled.

```sh
# expect {"status":200,"body":{"data":[{"id":1,"name":"waiting-on-calum"}]}}
curl -sS "$TICKETS_URL/api/v1/labels" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":201,"body":{"id":2,"name":"bug"}}
curl -sS "$TICKETS_URL/api/v1/labels" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"name":"bug","color":"#dd6677"}'
# expect {"status":200}
curl -sS -X POST "$TICKETS_URL/api/v1/tickets/1/labels/2" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":200}
curl -sS -X POST "$TICKETS_URL/api/v1/tickets/1/dependencies/2" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":409}
curl -sS -X POST "$TICKETS_URL/api/v1/tickets/2/dependencies/1" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":200,"body":{"labels":[{"id":2,"name":"bug"}],"blocks":[2]}}
curl -sS "$TICKETS_URL/api/v1/tickets/1" -H "Authorization: Bearer $TICKETS_TOKEN"
```

## Lists, filters, search and pagination

All domain lists return `data` and `next_cursor` (null at end). Pass the returned
cursor unchanged for the next page; default limit 50, range 1–100. IDs ascend
(except ticket history, which descends),
cursors are exclusive, and pagination is not a snapshot during concurrent writes.
Filters combine with AND: `project` ID, `status`, `label` ID, exact `assignee`,
`blocked=true|false`, `q`. Search is a literal FTS5 phrase over title/body,
case insensitive by tokens, not query syntax. Use `--data-urlencode` for values.
The first page below returns cursor `1`; use that returned value in the next call.

```sh
# expect {"status":200,"body":{"data":[{"id":1}],"next_cursor":"1"}}
curl -sS -G "$TICKETS_URL/api/v1/tickets" -H "Authorization: Bearer $TICKETS_TOKEN" \
  --data-urlencode 'limit=1'
# expect {"status":200,"body":{"data":[{"id":2}],"next_cursor":null}}
curl -sS -G "$TICKETS_URL/api/v1/tickets" -H "Authorization: Bearer $TICKETS_TOKEN" \
  --data-urlencode 'limit=1' --data-urlencode 'cursor=1'
# expect {"status":200,"body":{"data":[{"id":1}],"next_cursor":null}}
curl -sS -G "$TICKETS_URL/api/v1/tickets" -H "Authorization: Bearer $TICKETS_TOKEN" \
  --data-urlencode 'project=1' --data-urlencode 'status=in_progress' --data-urlencode 'label=2' \
  --data-urlencode 'assignee=agent' --data-urlencode 'blocked=false' --data-urlencode 'q=render bug'
# expect {"status":200,"body":{"data":[{"id":2,"blocked_by":[1]}]}}
curl -sS -G "$TICKETS_URL/api/v1/tickets" -H "Authorization: Bearer $TICKETS_TOKEN" \
  --data-urlencode 'blocked=true'
# expect {"status":200}
curl -sS -X DELETE "$TICKETS_URL/api/v1/tickets/1/dependencies/2" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":200}
curl -sS -X DELETE "$TICKETS_URL/api/v1/tickets/1/labels/2" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":200,"body":{"blocks":[],"labels":[]}}
curl -sS "$TICKETS_URL/api/v1/tickets/1" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":200,"body":{"id":1,"project_id":2,"key":"DST-1","assignee":null}}
curl -sS -X PATCH "$TICKETS_URL/api/v1/tickets/1" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"project_id":2,"assignee":null}'
```

## Ranked command-palette search

Use `GET /api/v1/search` for ranked key/title/body results instead of the
ID-ordered list. `q` accepts `status:todo` and `p:EX` filters and up to 8 words /
200 characters. Exact keys rank first, then literal ordered-character fuzzy
matches with open-status, update-age and recently-viewed boosts. `recent` accepts
up to 20 newest-first numeric IDs; omit it for agent requests without view history.
An empty query suggests recent and open in-progress/assigned tickets. `limit` is
1–50, default 30. The compact `{data:[...]}` response has no pagination cursor.
Invalid filters/parameters return 400. This does not change `/tickets?q=` FTS.

```sh
# expect {"status":200,"body":{"data":[{"id":1,"key":"DST-1"}]}}
curl -sS -G "$TICKETS_URL/api/v1/search" -H "Authorization: Bearer $TICKETS_TOKEN" \
  --data-urlencode 'q=DST-1 p:DST'
# expect {"status":400}
curl -sS -G "$TICKETS_URL/api/v1/search" -H "Authorization: Bearer $TICKETS_TOKEN" \
  --data-urlencode 'q=status:unknown'
```

## Comments

Bodies are CommonMark + GFM Markdown. Responses include raw `body` and sanitized
`body_html`; render only the latter as HTML. Session authors use display name
(username fallback) and `author_kind:"user"`; bearer authors use their token name
and `author_kind:"bot"`. Imported activity keeps `github`/`system`. Callers cannot
choose an author. Only the same user/token can PATCH or DELETE (403 otherwise);
the owner session cannot edit bot comments. Renaming never transfers ownership.

Exact methods/paths (all write requests reject unknown fields):
- GET `/api/v1/tickets/{id-or-key}/comments`: no body; 200
  `{"data":[Comment],"next_cursor":null}`; ascending IDs, limit 1–100, default 50.
- POST `/api/v1/tickets/{id-or-key}/comments`: `{"body":"Ready for **review**."}`;
  201 Comment.
- GET `/api/v1/comments/{id}`: no body; 200 Comment.
- PATCH `/api/v1/comments/{id}`: `{"body":"Reviewed."}`; 200 Comment, edited_at set.
- DELETE `/api/v1/comments/{id}`: no body; 200 `{"ok":true}`.

Complete Comment example (all three single-comment response methods):

```json
{"id":1,"ticket_id":1,"author":"Build agent","author_kind":"bot","body":"Reviewed.","body_html":"<p>Reviewed.</p>\n","created_at":"2026-10-30T10:00:00.000Z","updated_at":"2026-10-30T10:01:00.000Z","edited_at":"2026-10-30T10:01:00.000Z","deleted_at":null,"deleted":false,"can_edit":true}
```

`can_edit` is caller-specific; comment responses omit ownership IDs. DELETE retains an
empty-body stub in GET/list (`deleted:true`, `deleted_at` set, `can_edit:false`).
Counts exclude deleted comments. Mutating a deleted own comment returns 404;
legacy comments without durable identity are read-only. History records
`comment_created`/`comment_edited`/`comment_deleted`, with comment IDs in event
values. Use the returned IDs; never infer them from the walkthrough.

```sh
# expect {"status":201,"body":{"id":1,"body":"Ready for **review**.","author_kind":"bot","deleted":false,"edited_at":null,"can_edit":true}}
curl -sS "$TICKETS_URL/api/v1/tickets/1/comments" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"body":"Ready for **review**."}'
# expect {"status":200,"body":{"data":[{"id":1}]}}
curl -sS "$TICKETS_URL/api/v1/tickets/1/comments" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":200,"body":{"body":"Reviewed.","author_kind":"bot","deleted":false,"can_edit":true}}
curl -sS -X PATCH "$TICKETS_URL/api/v1/comments/1" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"body":"Reviewed."}'
# expect {"status":200,"body":{"body":"Reviewed.","author_kind":"bot","can_edit":true}}
curl -sS "$TICKETS_URL/api/v1/comments/1" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":200,"body":{"ok":true}}
curl -sS -X DELETE "$TICKETS_URL/api/v1/comments/1" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":200,"body":{"data":[{"id":1,"body":"","body_html":"","deleted":true,"can_edit":false}],"next_cursor":null}}
curl -sS "$TICKETS_URL/api/v1/tickets/1/comments" -H "Authorization: Bearer $TICKETS_TOKEN"
```

## Importer upsert

Use namespaced external IDs, e.g. `github:example-org/example:12`, URL encoded in
the query. PUT requires `project_id` and `title`; repeating the same ID updates
supplied fields without duplicates, preserving omitted fields and creation time.
A project change still reallocates the number. Attach labels/dependencies via
the relationship endpoints after resolving imported ticket IDs.

```sh
# expect {"status":200,"body":{"id":3,"title":"Imported issue"}}
curl -sS -X PUT "$TICKETS_URL/api/v1/tickets/by-external?id=github%3Aexample-org%2Fexample%3A12" \
  -H "Authorization: Bearer $TICKETS_TOKEN" -H 'Content-Type: application/json' \
  -d '{"project_id":1,"title":"Imported issue","external_url":"https://github.com/example-org/example/issues/12"}'
# expect {"status":200,"body":{"id":3,"title":"Updated import","external_url":"https://github.com/example-org/example/issues/12"}}
curl -sS -X PUT "$TICKETS_URL/api/v1/tickets/by-external?id=github%3Aexample-org%2Fexample%3A12" \
  -H "Authorization: Bearer $TICKETS_TOKEN" -H 'Content-Type: application/json' \
  -d '{"project_id":1,"title":"Updated import"}'
```

## Ticket change history

Read `GET /api/v1/tickets/{id-or-key}/history` to see committed changes and actors.
Unlike other domain lists, history uses **descending IDs (newest first)**.
Pass `next_cursor` unchanged with `cursor`; limit is 1–100 (default 50).
Rows contain `field`, JSON `old_value`/`new_value`, UTC `recorded_at`, durable
`subject_id`, and server-selected `actor_kind`, `actor_id`, `actor_label`.
Fields cover creation, status, priority, assignee, labels, title, description,
blockers, project moves, claims, heartbeats, expiry, release and comment creation/edits/deletion. History is
read-only; no-ops emit no rows. Successful heartbeats always record renewal.
Deleted tickets return 404; retained history never leaks through a reused ID.
Description/title values are raw text, not trusted HTML. Migration `created`
entries mark the start of tracking, not a reconstructed edit history.

```sh
# expect {"status":200,"body":{"data":[{"field":"title","actor_kind":"api_token","old_value":"Imported issue","new_value":"Updated import"}]}}
curl -sS "$TICKETS_URL/api/v1/tickets/3/history?limit=1" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":400}
curl -sS "$TICKETS_URL/api/v1/tickets/3/history?limit=101" -H "Authorization: Bearer $TICKETS_TOKEN"
```

## Errors

Inspect HTTP status and `{"error":{"code":"...","message":"..."}}`, not just
curl's exit code (`-sS` still exits zero for HTTP errors). 400 malformed request,
401 authentication, 403 comment ownership or cookie Origin failure, 404 missing resource, 409 conflict,
422 invalid fields/references, 500 storage failure, 503 `unavailable` for database
quota/capacity/transient outages. A 503 message is safe to show to the user;
wait before retrying reads. Correct invalid input before
retrying. Before retrying a write after a transport/server failure, read back its
state to avoid duplicate creates/comments; use external upsert for imports.

```sh
# expect {"status":404}
curl -sS -i "$TICKETS_URL/api/v1/tickets/EX-1" -H "Authorization: Bearer $TICKETS_TOKEN"
```

## Live subscriptions

For a long-lived watcher, connect a WebSocket to `/api/v1/live/{project_id}` using
`Authorization: Bearer $TICKETS_TOKEN` in the upgrade headers (never in the URL).
The owner can access all projects. On each typed `{project_id,entity,id,action}`
text event, refetch the relevant REST data. Link IDs identify the originating
ticket; labels are global. Events are best effort invalidations, may duplicate,
and have no replay. Reconnect with backoff plus jitter and refetch after reconnect.
Do not send application messages. See the API reference for cookie/Origin rules.
Ticket-affecting REST mutations notify only affected project rooms; claim
heartbeats and CI-only check updates send no notice. No separate publish call is
required, and notification failures never fail a committed mutation. Browser tabs
batch automatic reconciliation for 5 seconds with a 30-second cooldown; external
watchers should also coalesce invalidations rather than refetch for every event.

## GitHub migration provenance

Tickets is the sole system of record since 2026-10-29. GitHub Issues is retired.
Use native keys QUA, TIK, APP (apps), WWW (world-wide-webb), BOX (grok-box-config).
Imported `github:` tickets support all normal edits. Preserve external IDs/URLs
and imported comment markers, original authors and timestamps as provenance.
Both quantum and voxelite retain `github:0x63616c/voxelite:N` identities.

The native one-shot tool is `cargo xtask import-github --repo owner/repo --project
PREFIX --dry-run` (existing project, open issues by default). Options include
`--state all|open|closed`, `--since YYYY-MM-DD` for closed_at, `--exclude N,M`,
and `--close-github` to post the Tickets URL and close verified source issues
without duplicate movement comments. Production imports require explicit owner
authorization and `--allow-remote` with HTTPS; never infer authorization from
these examples. The signed webhook stays alive for PR integration; issue events
are ignored and there is no issue-mirror cron. See docs/DEVELOPMENT.md.

## Pull request links and job status

Read `GET /api/v1/tickets/{key}` to obtain `status` and `pull_requests` for job
tracking. PR records include `repo`, `number`, `url`, `title`, `state`
(open/draft/merged/closed), `closes`, `head_sha`, and `updated_at`. Do not infer
completion from a reference PR: only merged closing links mark a ticket done.

End PR titles with `[KEY-N]` and include `Closes [KEY-N](https://tickets.worldwidewebb.co/t/KEY-N)` (or Fixes/Resolves) in
the body. Use `Refs [KEY-N](https://tickets.worldwidewebb.co/t/KEY-N)` for reference-only links; it overrides a title bracket.
Multiple existing ticket keys can be linked. Signed GitHub events from configured
`GITHUB_PR_OWNERS` update links and statuses; retired issue events remain ignored.
Drafts start work, open PRs request review, closing merges finish work, and an
unmerged close returns active tickets to todo only when no open/draft PR remains.
Done/canceled tickets are protected from open/draft events; canceled never completes.
Edits remove links for removed keys. Read back after webhook delivery; older events
and redeliveries are ignored. The webhook is not an agent write endpoint.

## Home analytics

Read `GET /api/v1/analytics/home` for current in-flight/review/blocked counts and
8 UTC Monday weeks of native-created/observed-completed flow. Coverage is explicit:
null counts mean unavailable history. Optional `project` selects an active project;
`metric=in_flight|review|blocked|inventory|created|completed` returns its exact
50-row cohort with ascending durable subject-ID `cursor` / `next_cursor`.
`week=0..7` selects a flow bucket (default 7); `status` narrows inventory only.
Deleted historical subjects retain counts but have no ticket link. Use the
[metric contract](../../../docs/design/home-analytics.md); do not infer completion
from updated_at or treat PR observation versions as merge timestamps.

```sh
# expect {"status":200,"body":{"cohort":[],"next_cursor":null}}
curl -sS "$TICKETS_URL/api/v1/analytics/home" -H "Authorization: Bearer $TICKETS_TOKEN"
```

## Claim work, renew it, and release it

Use the saved rules rather than choosing work by judgment. Preview with
`GET /api/v1/next?project=<id>` if useful, then claim with
`POST /api/v1/next/claim` and `{"project":<id>}` (omit project for all projects).
A 200 returns `{ticket, reason}`; a 204 is empty and means wait for eligible work.
Use the **returned** ticket, which may differ from a prior preview. Todo-only,
active-claim exclusion and archived-project exclusion are fixed. Defaults also
skip open blockers, open/draft PRs and `waiting-on-calum`, prefer tickets blocking
open work, then priority and oldest creation, and cap each project at 3 tickets
In Progress or actively claimed. Rules are editable in Settings → Pickup rules;
GET `/api/v1/pickup-rules[?project=<id>]` shows the effective document and source.
Do not change rules just to obtain a blocked ticket.

Retain the numeric ticket ID, `claim_id` and `claim_expires_at`. The claim belongs
to the authenticated token/user, not an arbitrary agent name. With the same token,
POST `/api/v1/tickets/{id}/claim/heartbeat` with the exact JSON body
`{"claim_id":"<returned claim_id>"}` before expiry, typically every third of the
lease (10 minutes for the 30-minute default). Use each returned expiry to schedule
the next renewal. A 409 means ownership is gone: stop work and reconcile. An
uncertain/failed renewal is not proof of ownership; stop before the known expiry.
The claim ID fences stale workers even if they share a token.

After opening a PR or completing work, PATCH status to `in_review` or `done`, then
POST `/api/v1/tickets/{id}/claim/release` with that same claim ID. Release preserves
those statuses; abandoning an In Progress ticket releases it back to Todo.
Cron reclaims expired leases every minute in batches of 25. Every claim,
heartbeat, release and expiry appears in ticket history. Do not blindly retry an
uncertain claim POST: first read tickets and reconcile `claimed_by`/`claim_id`.
Work can be retried after expiry; keep external operations idempotent and check
for existing branches/PRs. A lease does not guarantee exactly-once external work.

The disposable walkthrough claims its remaining Todo ticket, then has no
eligible tickets:

```sh
# expect {"status":200,"body":{"source":"workspace","rules":{"concurrency_cap":3,"lease_seconds":1800}}}
curl -sS "$TICKETS_URL/api/v1/pickup-rules" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":200,"body":{"ticket":{"id":2,"status":"todo"}}}
curl -sS "$TICKETS_URL/api/v1/next" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":200,"body":{"ticket":{"id":2,"status":"in_progress"}}}
curl -sS -X POST "$TICKETS_URL/api/v1/next/claim" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{}'
# expect {"status":204}
curl -sS "$TICKETS_URL/api/v1/next" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":204}
curl -sS -X POST "$TICKETS_URL/api/v1/next/claim" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{}'
```


## GitHub App and linked PR checks

`GET /api/v1/github/app` returns `{registered, app_slug, app_name, html_url,
installations}` with each installation's `account_login`, `repository_selection`,
and ordered `repos` names.
It requires bearer/session authentication, exposes no secrets, and makes no GitHub
requests. Register/install from Settings → Account → GitHub in the owner's browser:
**Connect GitHub** navigates to `/github/app/register`, which POSTs the manifest
with server-side state; **Install on GitHub** follows registration. GitHub returns
to Settings after install, showing the account and repository selection. See
docs/API.md; the App replaces per-repository hooks while the legacy webhook
remains supported.
`GITHUB_PR_OWNERS` controls ingestion for both webhook endpoints.
`GET /api/v1/github/app/credentials` is bearer-only encrypted export for one-time
retrieval onto the operator box; decrypt locally with `GITHUB_APP_STORE_KEY` as
documented in docs/API.md. Sessions receive 403; an unregistered App returns 404.

Ticket reads' `pull_requests` include nullable `tickets_check` and `ci_status`
conclusions for the current PR head SHA. `tickets_check` is success when linked
tickets exist, neutral for missing/unknown keys. `ci_status` is the latest observed
CI event from another app (suite, run or commit status), not an aggregate of providers. `pending` means a check is running or queued. Null means no
report for this head; do not treat it as passed. Use `Closes KEY-N` or `Refs KEY-N`
and `[KEY-N]` title links as before. Nullable `tickets_check_run_id` and
`checks_updated_at` provide the current head's stored check metadata.

```sh
# expect {"status":200,"body":{"registered":false,"app_slug":null,"installations":[]}}
curl -sS "$TICKETS_URL/api/v1/github/app" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":404,"body":{"error":{"code":"not_found"}}}
curl -sS "$TICKETS_URL/api/v1/github/app/credentials" -H "Authorization: Bearer $TICKETS_TOKEN"
```

## Efficient repeated reads

List/search/Home analytics responses carry weak `ETag` and
`X-Workspace-Version` headers. Reuse `If-None-Match` for the same complete URL;
a 304 has no body. Responses remain authenticated and private.
`GET /api/v1/version` returns `{version: integer}` for cheap reconnect checks.
The eight UTC weeks in `/api/v1/analytics/home` are the shared cached weekly
shape for Analytics clients; preserve coverage/null semantics. See docs/API.md.

## Mission Control

PUT `/api/v1/mission/reports/{source}` replaces the latest raw JSON snapshot for
that source (1–40 lowercase ASCII letters/digits/hyphens). Maximum 512 KiB;
body must be an object; optional `jobs`, `pull_requests`, `merges`, `builds`,
`blocked`, `needs_calum` must be arrays of objects. Unknown fields are retained.
200 returns `{source,received_at}`; 413 means oversized and 422 means invalid.
GET `/api/v1/mission` returns `generated_at`, source `reports` with `received_at`,
`stale` after ten minutes and parsed `report`, plus cross-project ticket summaries
in `waiting_on_calum`, `in_review` (linked PRs/current-head CI), `recently_done`
(last 48 hours, maximum 30). Archived projects are excluded. See
[the report contract](../../../docs/mission-control.md). Cookie writes need Origin.

```sh
# expect {"status":200,"body":{"source":"skill-check"}}
curl -sS -X PUT "$TICKETS_URL/api/v1/mission/reports/skill-check" \
  -H "Authorization: Bearer $TICKETS_TOKEN" -H 'Content-Type: application/json' \
  -d '{"jobs":[],"builds":[]}'
# expect {"status":200,"body":{"reports":[{"source":"skill-check","stale":false,"report":{"jobs":[],"builds":[]}}]}}
curl -sS "$TICKETS_URL/api/v1/mission" -H "Authorization: Bearer $TICKETS_TOKEN"
```

## Profile photos

Account responses include nullable, versioned `avatar_url`.
GET `/api/v1/users` returns
`{data:[{id,username,display_name,avatar_url}]}` for resolving assignees.
Comment `avatar_url` follows durable user identity; bot/imported authors have none.

GET `/api/v1/users/{id}/avatar` requires authentication and serves image bytes
with a private revalidating cache and ETag; If-None-Match supports 304.

PUT/POST/DELETE `/api/v1/me/avatar` require a browser session and matching
Origin. Bearer tokens cannot change profile photos. Upload raw JPEG/PNG/WebP
with matching Content-Type, at most 512 KiB. Oversized uploads return 413;
unsupported or mismatched magic bytes return 415. Writes return the updated
account. DELETE removes the photo and returns `avatar_url:null`.

## Ticket classification and PR titles

Ticket titles are plain (for example `Grid home`, never `feat(ui): Grid home`).
Ticket `type` is nullable and must be exactly `feat`, `fix`, `chore`, `refactor`,
`docs`, `perf`, `test`, `ci`, or `build`. Optional nullable `scope` is trimmed and
lowercased and must match `[a-z0-9][a-z0-9._/-]{0,31}`; an empty string clears it.
The shared `tickets_core::ticket_type::pr_title` helper uses the title verbatim:
`<type>(<scope>): <title> [KEY-N]` with scope, or `<type>: <title> [KEY-N]`
without scope. An unset type falls back to `chore`, including when scope is set.

POST /tickets, PATCH /tickets/{id-or-key}, and PUT /tickets/by-external accept
these fields; omission preserves existing values on update and null clears them.
Invalid values return 422 with a clear error.message. All full ticket responses
include type, scope, and pr_title. Preview/claim return them inside `ticket`;
use that ready-made pr_title for the PR. History records type and scope changes.

```sh
# expect {"status":200,"body":{"id":3,"type":"feat","scope":"ui","pr_title":"feat(ui): Updated import [EX-3]"}}
curl -sS -X PATCH "$TICKETS_URL/api/v1/tickets/3" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"type":"feat","scope":" UI "}'
# expect {"status":422,"body":{"error":{"code":"validation"}}}
curl -sS -X PATCH "$TICKETS_URL/api/v1/tickets/3" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"type":"Feature"}'
# expect {"status":200,"body":{"type":null,"scope":null,"pr_title":"chore: Updated import [EX-3]"}}
curl -sS -X PATCH "$TICKETS_URL/api/v1/tickets/3" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"type":null,"scope":""}'
```
