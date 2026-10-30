# REST API

All domain routes live under `/api/v1` and require a session cookie or
`Authorization: Bearer $TICKETS_TOKEN`. Set `TICKETS_URL` to your instance URL
(for local development, `http://localhost:8787`). Keep the token in your secure
environment. Cookie mutations also require an `Origin` matching the request URL.
Invalid bearer credentials never fall back to cookies. Health at `/api/health`
and login at `POST /api/v1/login` are public; see `docs/adr/0003-auth.md` for authentication.

Responses contain JSON. Creation returns 201; reads, patches, deletes and external
upserts return 200. Deletes return `{"ok":true}`. Errors have the form
`{"error":{"code":"validation","message":"..."}}`:

| Status | Meaning |
| --- | --- |
| 400 | Malformed JSON, invalid path/query, empty patch |
| 401 | Missing, expired, invalid or revoked authentication |
| 403 | Cookie mutation without matching Origin |
| 404 | Resource or route not found |
| 409 | Uniqueness conflict or dependency cycle/self/duplicate |
| 422 | Invalid document, field value or reference |
| 500 | Unexpected storage failure (details withheld) |

All domain lists return `{"data":[...],"next_cursor":"123"}`. `next_cursor` is
null at the end. Pass it as `cursor` to fetch the next page. `limit` defaults to
50, maximum 100, minimum 1. Results use ascending numeric IDs (ticket history
uses descending IDs); cursors are exclusive.
Pagination is not a snapshot: concurrent updates can change matching results.
IDs and relationship IDs are positive integers. Unknown mutation fields are rejected.
PATCH changes only supplied fields; explicit null clears nullable fields.

## Account

`GET /api/v1/me` returns `authenticated`, `username`, and nullable `display_name`.
`PATCH /api/v1/me` accepts only `display_name`: trimmed text of at most 80 Unicode
characters, without control characters. Null or blank clears the name. Both
routes require authentication; cookie writes require Origin. Sign-in credentials
remain managed by deployment secrets, separate from this account label.

## Projects

| Method | Path | Document / behavior |
| --- | --- | --- |
| GET | `/projects` | Paginated projects, including archived |
| GET | `/projects/{id}` | Project |
| POST | `/projects` | Required `name`, `key_prefix`, `color` |
| PATCH | `/projects/{id}` | Any editable project fields, including `archived` |

Project reads include read-only `done_ticket_count` (done only), nullable
`last_activity_at` (latest ticket `updated_at`, including canceled tickets), and
`open_ticket_count`, counting all statuses except
`done` and `canceled` in the same SQL query, independent of ticket pagination.

Project fields: `name`, `key_prefix` (1–10 uppercase ASCII letters), `color`
(`#RRGGBB`), nullable `icon`, `archived` (boolean), nullable `github_owner`, `github_repo`,
`github_project_number` (positive integer), and `github_project_kind`
(`users` by default, or `orgs`). The kind preserves GitHub’s user/organization
URL distinction; no GitHub API requests are made. The UI accepts an owner and
number or a pasted `https://github.com/users|orgs/<owner>/projects/<number>` URL.
`github_repo` is the optional repository name under that owner. Archive with `{"archived":true}`;
restore with false. Archive does not delete tickets or prohibit imports.
Changing a prefix changes the displayed keys for that project's existing tickets.

```sh
curl -sS "$TICKETS_URL/api/v1/projects" -H "Authorization: Bearer $TICKETS_TOKEN"
curl -sS "$TICKETS_URL/api/v1/projects" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"name":"Example","key_prefix":"EX","color":"#789abc"}'
curl -sS -X PATCH "$TICKETS_URL/api/v1/projects/1" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"github_owner":"example-org","github_project_kind":"orgs","github_repo":"example","github_project_number":7,"archived":true}'
```

## Tickets

| Method | Path | Document / behavior |
| --- | --- | --- |
| GET | `/tickets` | Paginated tickets with filters below |
| GET | `/tickets/{id-or-key}` | Numeric ID or key such as `EX-12` |
| GET | `/tickets/{id-or-key}/history` | Append-only changes, newest first; see Ticket change history |
| POST | `/tickets` | Required `project_id`, `title`; other editable fields optional |
| PATCH | `/tickets/{id-or-key}` | Editable fields below |
| DELETE | `/tickets/{id-or-key}` | Deletes ticket, comments, label links and dependency edges |
| PUT | `/tickets/by-external?id={external-id}` | Atomic external-ID upsert; requires `project_id`, `title` |

Editable fields: `title` (nonblank), `body` (raw Markdown), `status`, `priority`,
`project_id`, nullable `assignee`, finite `sort_order`, nullable `external_url`
(HTTP/S URL) and `external_id` (namespaced string such as `github:org/repo:12`).
The external upsert's query ID is authoritative. It creates a ticket or updates
supplied fields on the existing one, preserving ID, creation time and number
unless moved to a different project. Omitted fields are preserved on update.

Status: `backlog` (default), `todo`, `in_progress`, `in_review`, `done`, `canceled`.
Priority: 0 none (default), 1 urgent, 2 high, 3 medium, 4 low. Body defaults to
empty, assignee to null, sort order to 0. Moving projects allocates a new number
atomically in the destination. Numbers are never reused; the old key stops resolving.

Responses include raw `body`, sanitized `body_html`, `key`, full `labels`,
`blocks` and `blocked_by` arrays of ticket IDs. Markdown uses CommonMark + GFM
and the shared sanitizer, including disabled task checkboxes.

Filters can be combined (AND): `project` (ID), `status`, `label` (ID), `assignee`
(exact string), `blocked=true|false`, and `q`. Blocked means an incoming dependency
whose blocker is neither done nor canceled. `q` searches title and body with FTS5,
using the provided text as a literal phrase (case insensitive token matching;
FTS syntax/operators are not interpreted). Lists order by ID, independently of
Kanban `sort_order`.

```sh
curl -sS "$TICKETS_URL/api/v1/tickets" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"project_id":1,"title":"First ticket","body":"**Details**","priority":2}'
curl -sS "$TICKETS_URL/api/v1/tickets/EX-1" -H "Authorization: Bearer $TICKETS_TOKEN"
curl -sS -G "$TICKETS_URL/api/v1/tickets" -H "Authorization: Bearer $TICKETS_TOKEN" \
  --data-urlencode 'project=1' --data-urlencode 'status=todo' --data-urlencode 'q=render bug' --data-urlencode 'limit=20'
curl -sS -X PATCH "$TICKETS_URL/api/v1/tickets/EX-1" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"status":"in_progress","assignee":"agent","sort_order":1.5}'
curl -sS -X PUT "$TICKETS_URL/api/v1/tickets/by-external?id=github%3Aexample-org%2Fexample%3A12" \
  -H "Authorization: Bearer $TICKETS_TOKEN" -H 'Content-Type: application/json' \
  -d '{"project_id":1,"title":"Imported issue","external_url":"https://github.com/example-org/example/issues/12"}'
```

## Ranked palette search

`GET /api/v1/search?q=mesh%20status:todo%20p:QUA&recent=7,3&limit=30`
uses the usual authentication gate. Unlike the ID-ordered `/tickets` list, it
returns a bounded ranked `{ "data": [...] }` without a cursor. Default limit 30,
maximum 50. Each result includes `id`, `project_id`, `key`, `title`, `status`,
`priority`, `assignee`, `updated_at`, and nullable `external_url`.

`q` is optional (up to 200 characters / 8 words). Case-insensitive `status:` and
`p:` filters combine with AND; duplicate/invalid filters return 400 with a useful
message. Status values use the domain names, such as `in_progress`. Matching uses
literal, ordered-character subsequences in title/body, not FTS syntax. Exact keys
always rank first; title relevance outweighs body relevance, with smaller boosts
for open status, update recency and recent viewing. `%`, `_`, and backslash are
literal characters. Ties use newest update then descending numeric ID.

`recent` is an optional newest-first comma-separated list of up to 20 positive
numeric ticket IDs, supplied from localStorage by the UI. It never persists server
history. An empty unfiltered query returns existing recent tickets plus open
in-progress/assigned tickets. Filter-only queries search all matching tickets.
Search performs one database statement, two calls including authentication; the
existing `/tickets?q=...` literal FTS and cursor contract is unchanged.

## Labels and dependencies

| Method | Path | Behavior |
| --- | --- | --- |
| GET | `/labels` | Paginated labels |
| GET | `/labels/{id}` | Label |
| POST | `/labels` | Required `name`, `color` |
| PATCH | `/labels/{id}` | `name` and/or `color` |
| DELETE | `/labels/{id}` | Delete label and ticket links |
| POST / DELETE | `/tickets/{id-or-key}/labels/{label-id}` | Attach / detach label |
| POST / DELETE | `/tickets/{id-or-key}/dependencies/{blocked-ticket-id}` | Add / remove outgoing dependency |

A dependency points from blocker to blocked. Adding one rejects a cycle, self
edge or duplicate with 409 and a clear message. Missing endpoints return 404.
Deleting a missing edge or label link returns 404.

```sh
curl -sS "$TICKETS_URL/api/v1/labels" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"name":"bug","color":"#dd6677"}'
curl -sS -X POST "$TICKETS_URL/api/v1/tickets/EX-1/labels/1" -H "Authorization: Bearer $TICKETS_TOKEN"
curl -sS -X POST "$TICKETS_URL/api/v1/tickets/EX-1/dependencies/2" -H "Authorization: Bearer $TICKETS_TOKEN"
curl -sS -X DELETE "$TICKETS_URL/api/v1/tickets/EX-1/dependencies/2" -H "Authorization: Bearer $TICKETS_TOKEN"
```

## Comments

| Method | Path | Behavior |
| --- | --- | --- |
| GET | `/tickets/{id-or-key}/comments` | Paginated comments |
| POST | `/tickets/{id-or-key}/comments` | Required nonblank Markdown `body` |
| GET | `/comments/{id}` | Comment |
| PATCH | `/comments/{id}` | Update `body` |
| DELETE | `/comments/{id}` | Delete comment |

Responses contain raw `body` and sanitized `body_html`. User-created comments use author
`owner`; PR activity uses `GitHub` or `system`. Callers cannot spoof authors through request fields.

```sh
curl -sS "$TICKETS_URL/api/v1/tickets/EX-1/comments" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"body":"Ready for **review**."}'
curl -sS -X PATCH "$TICKETS_URL/api/v1/comments/1" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"body":"Reviewed."}'
curl -sS -X DELETE "$TICKETS_URL/api/v1/comments/1" -H "Authorization: Bearer $TICKETS_TOKEN"
```

## Verification and change events

`cargo test --workspace` runs domain route integration tests against SQLite with
foreign keys and the full migration chain. `cargo xtask api-smoke` builds the app,
creates an isolated local D1 database, starts `wrangler dev` on port 8795, tests
cookie and bearer access plus all domain route families, and stops the server.
Its generated credentials and logs stay in an ignored `target/api-smoke-*` folder.
No remote database or deployment is involved.

### Project WebSocket

`GET /api/v1/live/{project_id}` with `Upgrade: websocket` subscribes to one project.
Use the same bearer header or session cookie as REST. Missing/invalid credentials
return 401; cookie upgrades require a matching Origin (403 otherwise). An unknown
project returns 404; a normal authenticated GET returns 426. Day one has one owner,
so all authenticated credentials can see every project, including archived ones.
Never put tokens in query strings. Browser WebSockets automatically send cookies.

Each text frame is a `tickets_core::live::ChangeEvent`, for example:

```json
{"project_id":1,"entity":"tickets","id":42,"action":"update"}
```

Entities are `projects`, `tickets`, `labels`, `dependencies`, `comments`; actions
are `create`, `update`, `upsert`, `delete`. Link events identify the originating
ticket. Events invalidate cached data; refetch authoritative REST state, including
comments/dependencies as needed. Delivery is best effort and frames may duplicate
or arrive out of order. Reconnect with exponential backoff and jitter, then refetch.
The stream is receive-only; sending application messages closes it with 1008.
Cross-project moves notify both rooms; global labels notify all existing rooms.
D1 writes succeed even when notification fails. There is no replay log.

`cargo xtask live-check` tests local upgrade authentication and two WebSocket
clients, then checks a move in two browser windows plus list/detail and reconnect.
See [architecture](ARCHITECTURE.md) for free-tier constraints and consistency limits.

The browser also reconciles when a tab becomes visible, on window focus/online,
and every 60 seconds while visible. These triggers and stream invalidations share
a 350 ms debounce and one board/list fetch at a time. Reads retain the displayed
content and wait for local edits, comment drafts and board moves. Each catch-up
uses the existing paginated REST endpoints (projects, tickets, labels, plus ticket
and comments on detail); no new D1 query, replay log or Durable Object alarm is used.

## GitHub webhook and imported tickets

Tickets is the sole system of record since 2026-10-29; GitHub Issues is retired.
`POST /api/v1/github/webhook` remains independently HMAC authenticated for
pull-request integration. It requires `X-Hub-Signature-256: sha256=<hex>` over
exact raw bytes using `GITHUB_WEBHOOK_SECRET`. Missing/invalid signatures return
401; unset/empty secret returns 503. Body limit is 1 MiB (413), malformed JSON
400, non-POST requests 405. Signed ping returns 200. Signed `issues`,
`issue_comment` and `label` events return 202 with `ignored: true` and reason
`issue mirror retired`, without database access. No issue cron runs.

Tickets with `github:` external IDs are native editable tickets. Preserve
`external_id`/`external_url` as provenance; the detail page links to their origin.
Imported comments keep `<!-- tickets-import:github:... -->` markers and original
GitHub author/timestamp attribution in sanitized Markdown; API author is `owner`.
Use normal authenticated mutations for editing imported tickets and comments.

`cargo xtask import-github --repo owner/repo --project PREFIX --dry-run` migrates
into an existing project. Open issues are the default; use `--state all|open|closed`,
`--since YYYY-MM-DD` for a closed_at cutoff, `--exclude N,M`, and `--close-github`
to comment/close only after verified import. The quantum/voxelite alias preserves
existing QUA identities. See [migration usage](DEVELOPMENT.md#github-import).

## GitHub pull requests

The same signed webhook handles `pull_request` using its owner allowlist.
`GITHUB_PR_OWNERS` is a comma-separated, case-insensitive repository-owner allowlist
(local and production vars); unknown owners return 202. A signed ping needs no
repository and returns 200. PR actions opened, reopened, ready_for_review,
converted_to_draft, edited, synchronize and closed are supported.

PR bodies must contain `Closes [KEY-N](https://tickets.worldwidewebb.co/t/KEY-N)`
(or `Refs [KEY-N](https://tickets.worldwidewebb.co/t/KEY-N)` for reference-only links).
The web route `/t/KEY-N` resolves keys case-insensitively; legacy `/ticket?id=N`
links remain supported. Markdown link destinations do not contribute keys or intent.

Title brackets `[TIK-32]` create closing links by default. Body `Closes`, `Fixes`
or `Resolves` (case insensitive, optional colon) create closing links; `Refs [TIK-32](https://tickets.worldwidewebb.co/t/TIK-32)`
creates a reference link and overrides a title bracket. Multiple keys are supported;
only existing project/ticket keys resolve. Unknown keys return 202 when none resolve.
Edits replace the PR's complete key set, removing missing links.

Ticket responses (including `GET /api/v1/tickets/{key}` for the box job CLI) contain
`status` and an ordered `pull_requests` array, empty when unlinked. Each PR has
`repo`, `number`, `url`, `title`, `state` (`open`, `draft`, `merged`, `closed`),
`closes` (boolean), `head_sha` and `updated_at`. Order is repository then number.

Open PRs move backlog/todo/in_progress tickets to in_review; drafts move
backlog/todo to in_progress. Neither moves in_review backwards or changes done
or canceled. A merged closing link marks done unless canceled. An unmerged close
returns in_review/in_progress to todo only when no other open/draft PR remains.
Reference merges leave status unchanged. GitHub/system comments record link and
status changes. No requests are made back to GitHub.

A persisted per-PR snapshot prevents stale events from restoring removed links.
Newer GitHub timestamps win; equal timestamps prefer merged, closed, open, draft
in that order. Equal-time conflicting edits of the same state retain the first
snapshot. Exact redeliveries produce no writes/activity. Stale/replayed snapshots return 202. Ingestion uses one atomic
D1 statement with eight parameters, plus one project lookup for live invalidation;
it does not query per key. Signed malformed PR payloads return 400.

### Project icons

`icon` is optional on POST and PATCH and returned on every project read. Omitted
PATCH fields are preserved; `null` clears the icon and restores the colored initial.
Use a named glyph (`project`, `ticket`, `board`, `grid`, `code`, `globe`, `box`,
`bolt`, `star`, `layers`) or an emoji sequence of at most 16 Unicode scalars,
including joiners, variation selectors, skin tones and flag sequences. Blank,
plain text, controls and markup are rejected with 422. `color` applies to glyphs
and the fallback tile; emoji retain their native color. Migration 0009 is additive.

## Home analytics

`GET /api/v1/analytics/home` returns the authenticated, no-store Home snapshot:
`as_of`, `history_from`, current `in_flight`/`in_review`/`blocked` counts, active
`projects` with status counts, eight UTC Monday `weeks`, five-row `review_queue`
and `blocked_queue`, and `cohort`, `cohort_total`, `next_cursor`.

Optional query: `project=<active ID>`,
`metric=in_flight|review|blocked|inventory|created|completed`, `week=0..7`
(oldest first, flow only, default 7), `status=<open status>` (inventory only),
`cursor=<subject ID>` (metric only). Cohorts use 50 rows and ascending durable
subject IDs. Unknown, duplicate and incompatible parameters return 422.
Inventory counts and status filters follow Board/List's `later` → Backlog display
rule; cohort rows retain the stored ticket status.
Weeks contain `start`, `end`, `coverage=none|partial|full`, nullable `created`,
`completed`, `imported`. Null is unavailable, never zero. Deleted historic cohort
rows have null ticket/key/title and remain counted. Full metric definitions,
coverage, event-time attribution and deferred scope: [Home analytics](design/home-analytics.md).

## Ticket change history

`GET /api/v1/tickets/{id-or-key}/history?limit=50&cursor=<id>` requires the same
session/bearer authentication as ticket reads. It returns `data` and
`next_cursor` (null at end). **History alone uses descending IDs**, newest first,
with an exclusive cursor; default limit 50, range 1–100. Pass the returned cursor
unchanged. New writes do not shift older pages; this is not a snapshot.

Each entry contains `id`, `subject_id` (durable ticket identity), `ticket_id`,
`actor_kind` (`user`, `api_token`, `webhook`, `system`), `actor_id`, `actor_label`,
`field`, `old_value`, `new_value`, and `recorded_at` (UTC ISO 8601). Actor labels
are safe server-generated aliases (`Owner`, `API token #N`, `GitHub`, `System`);
IDs are stable and no credentials, email addresses or account names are copied.
History actor fields cannot be supplied in mutation documents.

Fields: `created`, `deleted`, `status`, `priority`, `assignee`, `title`,
`description`, `project`, `label_added`, `label_removed`, `blocker_added`,
`blocker_removed`, `blocks_added`, `blocks_removed`. `claim`, `heartbeat`, `expiry`
and `release` record the pickup lease lifecycle described below. Old/new values
are JSON scalars for ordinary fields, `{id,name,key}` for projects,
`{id,name,color}` for labels and `{id,key,title}` for dependencies. Absent sides
are null. `created` contains an initial snapshot; migration baselines contain
`{title,baseline:true}` and use the observation timestamp, not a fabricated past
change. Title/description history is raw text; render it as text or use the shared
Markdown sanitizer, never raw HTML.

History is append-only and commits atomically with each ticket write. No-op
updates and replayed imports/webhooks emit no changes. Ticket and label deletion
cascades record removed relationships on surviving tickets. History survives
ticket deletion internally, but this endpoint returns 404 for a deleted ticket;
reused numeric IDs resolve only the new durable identity. No history write or
purge endpoint is exposed. Analytics `ticket_events` is independent and unchanged.

## Pickup rules and ticket claims

`GET /api/v1/pickup-rules` returns `{rules, source:"workspace", project_id:null}`.
With `?project=<positive ID>`, it returns that project's complete override, or the
workspace rules with `source:"workspace"`. `PUT` on the same URL saves a complete
validated document (all fields below required; unknown fields rejected with 422):

```json
{"skip_blocked":true,"skip_open_pr":true,"blockers_first":true,"priority_first":true,"excluded_labels":["waiting-on-calum"],"concurrency_cap":3,"lease_seconds":1800}
```

`DELETE /api/v1/pickup-rules?project=<id>` removes only the override and returns
inherited rules. Deleting the workspace default is rejected. Cap is 1–100;
lease duration is 60–86400 seconds; excluded labels are at most 20 unique names,
1–100 characters each, matched without ASCII case. Saving rules never deletes labels.
An override replaces the whole document; subsequent workspace edits affect only
inheriting projects. Settings → Pickup rules exposes the same settings.

`GET /api/v1/next[?project=<id>]` previews without writes. `POST
/api/v1/next/claim` accepts `{}` (or an empty body), or `{"project":<id>}`.
Both return HTTP 200 with `{ticket, reason}`, or **204 with no response body**.
A preview is advisory; always use the claim response to choose work. Claims are
atomic with ranking, capacity checks, In Progress, and history, including when
multiple workers claim at the same time. Successful claims return the full ticket,
including `claimed_by`, `claim_id`, `claim_expires_at`, `claim_heartbeat_at` (UTC).
`claimed_by` is the safe authenticated identity, e.g. `api_token:12`, never a name
or credential supplied by the caller. Do not use `assignee` as lease ownership.

Todo is the only eligible status; Backlog and archived projects are always skipped.
Tickets with an outstanding claim remain excluded until released/swept. Defaults
exclude open blockers (anything except Done/Canceled), open or draft PRs, and the
seeded `waiting-on-calum` label. Ranking prefers blockers of open tickets, then
Urgent, High, Medium, Low, None, then oldest creation and numeric ID. The cap counts
each In Progress ticket or unexpired claim once within its project, including
manual work. Across projects, the same rank tuple uses each project's effective
preferences. Disabling priority gives a neutral rank ahead of priority-ranked
candidates; disabling blocker preference removes that candidate's blocker boost.
Example reason: `High priority, oldest eligible Todo in its ranking group,
unblocks TIK-50`.

`POST /api/v1/tickets/{id-or-key}/claim/heartbeat` and `.../claim/release`
require `{"claim_id":"<the 32-character ID returned by claim>"}` and the same
user/token identity that acquired it. Both return the updated full ticket. Missing
or malformed input is 400; an expired, replaced, released or differently owned
claim is 409 `claim_conflict`, with no history write. Unknown ticket is 404.
Heartbeat extends from server time by the current effective lease duration, never
shortens it, and cannot revive expiry. Every successful heartbeat is recorded,
including repeated requests within one timestamp interval.

Release clears the lease and returns In Progress to Todo. To finish, first PATCH
the ticket to In Review/Done, then release: later statuses are preserved. A cron
every minute clears up to 25 expired leases (oldest first) and returns only those
still In Progress to Todo; later ticks drain a larger backlog. Claim, heartbeat,
release and expiry use the same atomic mutation/history path as other edits.
History's claim event values contain owner, claim ID, expiry, heartbeat, action and
monotonic event version; regular ticket reads also include the event/version.

This is renewable ownership, not exactly-once execution. Stop work after losing
or failing to renew a lease; confirm ownership before side effects. Do not blindly
retry a timed-out claim request: it may have committed. Reconcile by reading
In Progress tickets and their `claimed_by`/`claim_id` first. External operations
should be idempotent; inspect existing branches and PRs before creating them.
