# REST API

All domain routes live under `/api/v1` and require a session cookie or
`Authorization: Bearer $TICKETS_TOKEN`. Set `TICKETS_URL` to your instance URL
(for local development, `http://localhost:8787`). Keep the token in your secure
environment. Cookie mutations also require an `Origin` matching the request URL.
Invalid bearer credentials never fall back to cookies. Health at `/api/health`
and login at `POST /api/v1/login` are public; see `docs/adr/0003-auth.md` for authentication.

Domain responses contain JSON. The GitHub App registration page and manifest callback
below return HTML.
Creation returns 201; reads, patches, deletes and external
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
| 503 | D1 quota/capacity/transient outage (`unavailable`) |

D1 quota, overload and availability failures return HTTP 503 with
`error.code = "unavailable"` and a safe message. Read back mutation state before
retrying writes after any server failure. See [D1 operations](d1-operations.md).

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

All paths below include `/api/v1`. Callers cannot set authors or timestamps;
unknown request fields are rejected. `body` must be nonblank CommonMark/GFM.
Render sanitized `body_html`, never raw `body` as HTML.

| Method | Path | Request JSON | Status / response |
| --- | --- | --- | --- |
| GET | `/api/v1/tickets/{id-or-key}/comments?limit=50&cursor=1` | none | 200 `{ "data": [Comment], "next_cursor": null }` |
| POST | `/api/v1/tickets/{id-or-key}/comments` | `{"body":"Ready for **review**."}` | 201 Comment |
| GET | `/api/v1/comments/{id}` | none | 200 Comment |
| PATCH | `/api/v1/comments/{id}` | `{"body":"Reviewed."}` | 200 Comment, with `edited_at` set |
| DELETE | `/api/v1/comments/{id}` | none | 200 `{"ok":true}` |

A complete Comment response (POST example; PATCH/GET use the same shape):

```json
{"id":1,"ticket_id":1,"author":"Build agent","author_kind":"bot","body":"Ready for **review**.","body_html":"<p>Ready for <strong>review</strong>.</p>\n","created_at":"2026-10-30T10:00:00.000Z","updated_at":"2026-10-30T10:00:00.000Z","edited_at":null,"deleted_at":null,"deleted":false,"can_edit":true}
```

Session authors use their display name (username fallback), `author_kind:"user"`;
bearer authors use the token name, `author_kind:"bot"`. Names are snapshotted at
creation; renaming never transfers ownership. `github` and `system` remain for
imported/machine activity. Legacy authors with no durable identity remain read-only.
`can_edit` is caller-specific and applies to both PATCH and DELETE. Only the same
user or same token may mutate a comment, otherwise 403 `error.code:"forbidden"`.
There is no owner override, including for bot comments. Comment responses omit internal ownership IDs.

DELETE soft-deletes and preserves storage/history. GET and lists retain a stub
with `body:""`, `body_html:""`, `deleted:true`, `deleted_at` set, and
`can_edit:false`; PATCH/DELETE of your deleted comment return 404. Counts on
cards/list rows exclude deleted comments. Lists use ascending ID cursors (default
50, max 100), including stubs. Ticket deletion may still cascade comment storage.

Each successful creation/body change/deletion atomically appends
`comment_created`, `comment_edited`, or `comment_deleted` to the ticket history
endpoint. Event values contain `{"id":comment_id}` (creation: new value only;
edit: both; deletion: old value only). Bodies are omitted to avoid retaining
removed content in history. History and comments share a durable ordered history
sequence; use returned cursors unchanged. No-op body updates emit no history event.

See the [agent skill](../.agents/skills/tickets/SKILL.md#comments) for executable,
`cargo xtask skill-check`-verified curl examples for every method.

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

The browser's shared collection, visibility handling, reconciliation cadence and
read caching are described in [Read caching and weekly analytics](#read-caching-and-weekly-analytics-tik-71)
below. No Durable Object replay log or alarm is used.

## GitHub App registration (Phase 1)

`GET /api/v1/github/app/manifest-callback?code=<CODE>&state=<STATE>` is a
browser callback outside the session/bearer auth gate. It requires the secret
setup state; it is not a general agent API endpoint.

1. Configure Worker secrets `GITHUB_APP_SETUP_STATE` (an unguessable random
   value) and `GITHUB_APP_STORE_KEY` (standard base64 of 32 random bytes).
   CI deploys them from production environment secrets
   `TICKETS_GITHUB_APP_SETUP_STATE` and `TICKETS_GITHUB_APP_STORE_KEY`.
2. In the owner's signed-in browser, open **Settings → Account → GitHub** and
   click **Connect GitHub**. This navigates outside the SPA to
   `GET /github/app/register`. This Worker-rendered page automatically POSTs the
   manifest and configured setup state to `https://github.com/settings/apps/new`; a **Continue to GitHub** button
   also works without JavaScript. The manifest uses the request's HTTP(S) origin
   for its URLs (including local development). `?name=alt` selects **Tickets WWW**
   instead of **Tickets by World Wide Webb**; other names are ignored.
   Signed-out visitors redirect to `/login?next=%2Fgithub%2Fapp%2Fregister`.
   Missing/empty configuration or an invalid store key returns a 503
   **Registration unavailable** page. An existing registration returns 200 with
   its GitHub app link and no form; non-GET requests return 405 with `Allow: GET`
   after the session check. Responses use `Cache-Control: no-store`,
   `Referrer-Policy: no-referrer`, `X-Content-Type-Options: nosniff`,
   `X-Frame-Options: DENY`, and a CSP authorizing only the hashed inline style
   and auto-submit script, with `form-action https://github.com` and
   `frame-ancestors 'none'`. See [GitHub's manifest flow](https://docs.github.com/en/apps/sharing-github-apps/registering-a-github-app-from-a-manifest).
3. GitHub redirects with a one-use code, valid for one hour. The Worker checks
   configuration, compares state in constant time, validates the code, and checks
   for an existing registration before making an unauthenticated
   `POST https://api.github.com/app-manifests/{code}/conversions`
   (`Accept: application/vnd.github+json`, `X-GitHub-Api-Version: 2022-11-28`,
   `User-Agent: tickets-worldwidewebb`). It requires a 201 response.
4. The Worker stores public metadata and encrypts `{pem, webhook_secret,
   client_secret}` with AES-256-GCM. `secrets_ciphertext` is standard base64 of
   a fresh 12-byte IV followed by ciphertext and its 16-byte authentication tag.
   The 200 page confirms registration and links to the app and **Install the
   app**, plus a link back to `/settings`. GitHub returns to
   `/settings` after installation via the manifest `setup_url`. Settings shows
   **Install on GitHub** until an installation is received, then **Connected**,
   the account, repository selection and **Manage on GitHub**.

The code must contain 1–128 ASCII letters, digits, `_` or `-`. Missing/empty
secrets or an invalid store key return 503; incorrect/missing state returns 403;
incorrect/missing code returns 400; an existing app returns 409 without calling
GitHub. Other methods return 405. GitHub transport/conversion/response failures
return 502 with a fixed message and, when available, only the upstream HTTP
status. Storage/encryption failures return a generic 500. Every callback page
uses `Cache-Control: no-store`, `Referrer-Policy: no-referrer` and a CSP allowing
only the page's hashed inline style. Codes, states and credentials are never
included in response bodies or application logs.

Keep the store key available: changing it without re-encrypting the row makes
the saved credentials unreadable. Remove the setup state after registration to
disable the callback; credential loading only needs the store key. There is no
registration overwrite/reset API.

### Encrypted credentials export

`GET /api/v1/github/app/credentials` requires normal API-token bearer
authentication. Browser sessions receive 403 (`forbidden`); missing or invalid
authentication receives 401. Tokens have full workspace access with no separate
admin role. If no `github_app` row exists, it returns 404 (`not_found`).
Responses use `Cache-Control: no-store`.

A 200 response contains exactly these fields:

```json
{"app_id":123,"slug":"example-app","name":"Example App","owner_login":"example-owner","client_id":"example-client-id","html_url":"https://github.com/apps/example-app","secrets_ciphertext":"<stored base64 blob>"}
```

Use this endpoint for one-time retrieval onto the operator box after manifest
registration. The Worker returns the stored encrypted blob unchanged and never
decrypts it for export. On the box, decode standard base64, take the first 12
bytes as the IV and the last 16 bytes as the 128-bit authentication tag, and
decrypt the intervening ciphertext using AES-256-GCM with the locally held
`GITHUB_APP_STORE_KEY` (standard base64 of 32 bytes). The plaintext is JSON with
string fields `{"pem":"...","webhook_secret":"...","client_secret":"..."}`.
Keep the response and decrypted credentials out of logs. Retrieval does not
consume or delete the stored registration; the endpoint remains repeatable.

### GitHub App events and installation status

The App replaces per-repository hooks. Set the manifest `hook_attributes.url` to
`https://tickets.worldwidewebb.co/api/v1/github/app/webhook`, grant **Checks: write**,
**Pull requests: read**, and **Metadata: read** for PR/check handling. The generated
manifest additionally requests **Issues: read**, **Commit statuses: write** and
**Contents: read**, and subscribes to `pull_request`, `check_suite`, `check_run`
and `push` (push is ignored; installation events are automatic). Install via the
callback page's link or `https://github.com/apps/<app-slug>/installations/new`.
Keep `GITHUB_PR_OWNERS` configured as a comma-separated owner allowlist. After
verifying the App, remove the old per-repository hooks; the legacy
`/api/v1/github/webhook` and its `GITHUB_WEBHOOK_SECRET` continue to work unchanged
until then. Duplicate status deliveries are replay-safe.

`POST /api/v1/github/app/webhook` bypasses bearer/session auth and authenticates
raw bytes with the registered App's webhook secret using constant-time
HMAC-SHA256 (`X-Hub-Signature-256: sha256=<hex>`). Missing registration or
unreadable credentials returns 503; invalid signatures 401; bodies over 1 MiB
413; malformed JSON 400; non-POST 405. `ping` returns 204. Installation lifecycle
and repository-selection events persist the account and installed repositories.
Unsupported events (including push) return 202.

PR events share the legacy endpoint's allowlist, link parser, atomic status sync,
and replay rules. Opened/reopened/synchronize/edited/ready_for_review also post a
completed **Tickets** check on the PR head: success for existing linked tickets,
neutral for unknown keys or no key. Output includes ticket links, titles, current
statuses and Closes/Refs intent. Check-post failures log a fixed non-secret
message and preserve the successful status-sync response. Check suite requests
and rerequests recompute checks (fetching PR details as needed); check run
rerequests are handled only for this App's **Tickets** run. Standalone recompute
failures return 502 so GitHub can retry. Other apps' completed suites record the
latest observed CI conclusion for that head (this is not an aggregate across all
CI providers). Older suite timestamps cannot overwrite newer conclusions.

Worker authentication signs short-lived RS256 App JWTs via WebCrypto, accepting
PKCS#1 or PKCS#8 PEM, and exchanges them for installation tokens. Tokens/keys
never appear in logs or responses. `GITHUB_API_BASE_URL` optionally overrides
`https://api.github.com` for local mock API tests; leave it unset in production.
Redirects are never followed with credentials.

`GET /api/v1/github/app` requires normal bearer/session authentication and returns
`{registered, app_slug, app_name, html_url, installations:[{account_login,
repository_selection, repos:["owner/repo"]}]}`. `app_slug`, `app_name` and `html_url`
are null before registration; repos and installations are ordered. This read
never decrypts credentials or calls GitHub, and exposes no secrets.

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
GitHub author/timestamp attribution in sanitized Markdown. Existing imports are
read-only comments; normal authenticated mutations can still edit imported tickets.

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

Linked ticket `pull_requests` entries include nullable `tickets_check` (the Tickets
check conclusion) and `ci_status` (latest other-app CI suite conclusion). Both
are scoped to the current `head_sha`; a new head has null values until reported.
Historical heads never supply current results. `tickets_check_run_id` and
`checks_updated_at` are also nullable and scoped to that head. Conclusions include
`success`, `failure`, `neutral`, `cancelled`, `timed_out`, or `skipped`. Null means unreported.

Open PRs move backlog/todo/in_progress tickets to in_review; drafts move
backlog/todo to in_progress. Neither moves in_review backwards or changes done
or canceled. A merged closing link marks done unless canceled. An unmerged close
returns in_review/in_progress to todo only when no other open/draft PR remains.
Reference merges leave status unchanged. GitHub/system comments record link and
status changes. The legacy endpoint makes no requests back to GitHub; the App
endpoint also posts the Tickets check described above.

A persisted per-PR snapshot prevents stale events from restoring removed links.
Newer GitHub timestamps win; equal timestamps prefer merged, closed, open, draft
in that order. Equal-time conflicting edits of the same state retain the first
snapshot. Exact redeliveries produce no writes/activity. Stale/replayed snapshots return 202. Ingestion uses one atomic
mutation batch with eight PR parameters. Affected project IDs (old/new links and
relationship neighbors) return in the same batch; there is no global project
lookup or query per key. Signed malformed PR payloads return 400.

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
snapshot the authenticated display name/username or token name; webhook/system
actors retain `GitHub`/`System`. Older rows retain their original aliases. IDs
are stable and no credentials are copied.
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

### Read caching and weekly analytics (TIK-71)

`GET /api/v1/projects`, `/tickets`, `/labels`, `/search` and
`/analytics/home` support weak `ETag` / `If-None-Match`. Reuse the tag for the
**same complete URL**, including filters and cursor. A matching, still-cached
successful representation returns an empty 304. Every request is authenticated
first; responses use `Cache-Control: private, no-cache`. Comment responses are
never shared because `can_edit` depends on the author.

The Worker keeps serialized list/analytics responses in its Cache API for 60
seconds, keyed by URL, schema version and the workspace's transactional version.
Analytics also includes the UTC minute in its key. A hit reads only auth and one
version row; it does not scan ticket history, render Markdown, or parse and
serialize the cached JSON. Cache failures fall back to authoritative SQL. A cold
miss still computes the existing analytics snapshot; this is a response rollup
cache, not reconstructed historical data. Cache storage is local to a Cloudflare
location. There is no promise of a warm hit in another location.

For Analytics page weekly flow, reuse `GET /api/v1/analytics/home[?project=ID]`.
Its stable typed shape includes `as_of`, `history_from`, and
`weeks: [{start, end, coverage, created, completed, imported}]` (eight UTC Monday
weeks, oldest first). `coverage` is `none|partial|full`; unavailable counts are
null, never zero. Created means native creation, imported is separate, and
completed counts each durable subject once per week at its observed completion
project. Project changes/deletion retain historical semantics. The same response
also supplies current counts/queues and per-project inventory. Drill down using
`metric=created|completed&week=0..7&cursor=ID`; reuse the returned `next_cursor`.
See [the analytics contract](design/home-analytics.md) for the full shape.

`GET /api/v1/version` returns `{ "version": <integer> }` under the same auth
gate. List responses expose that version as `X-Workspace-Version`. Clients may
compare it after connecting live rooms to close the initial snapshot/subscription
gap cheaply. Ticket, project, label, comment, dependency, label membership, PR-link, event
and coverage writes advance it through additive triggers. Check metadata advances
it only for a linked current head; unlinked/stale-head checks cannot change
cached ticket payloads. These triggers cover
writes from older Workers. Versions are opaque invalidation values,
not event IDs. Reconnect still requires reconciliation: live events have no replay.

The web app shares its ticket collection for the shell's lifetime, deduplicates
project/label/analytics/search requests, and patches affected ticket records on
live events. It subscribes to every workspace project across route changes so
inactive boards remain coherent. Healthy focus and idle tabs do not reload lists;
hidden-tab events queue until visible. Automatic notice/reconnect reconciliation
batches for 5 seconds with at most one reconciliation per 30 seconds per tab.
Notices target affected projects only; an empty scope does not broadcast. Label
edits notify projects using that label; unused/new labels and new projects become
visible elsewhere on the next metadata refresh. Claim heartbeats and CI-only
check updates send no notices; CI metadata appears on the next normal or manual
refresh. A reconnect reloads authoritative state
and can reuse the edge response cache. Mutations invalidate cached reads; explicit
Reload/Retry actions force reconciliation.

The authenticated web transport uses fetch `cache: no-store`: browser disk-cache
revalidation is separate from the app's session cache and Worker Cache API.
This avoids missing-body/cache-read failures across hard navigation. Agents may
still explicitly send `If-None-Match`; warm and cold successful representations
honor validators, while invalid requests retain their normal errors.

[Mission Control: report ingest, aggregated read and dashboard](mission-control.md).

### Profile photos

`GET /api/v1/me` and `PATCH /api/v1/me` include nullable `avatar_url`.
`GET /api/v1/users` returns `{data:[{id,username,display_name,avatar_url}]}` for
resolving legacy name-based assignees. URLs contain a content version (`?v=…`)
and change on replacement. Comment responses include nullable `avatar_url` based
on durable user identity; bot, imported and system authors have no profile photo.

- `PUT` or `POST /api/v1/me/avatar`: signed-in **session only**, matching Origin,
  raw JPEG/PNG/WebP body with the matching Content-Type. Returns 200 account
  (`username`, `display_name`, `avatar_url`). Replaces the current photo atomically.
- `DELETE /api/v1/me/avatar`: same session/Origin requirements; idempotently removes
  the photo and returns 200 account with `avatar_url:null`.
- `GET /api/v1/users/{id}/avatar`: authenticated via session or bearer, returns raw
  bytes with Content-Type, `X-Content-Type-Options: nosniff`, a content ETag and
  `Cache-Control: private, no-cache`. Supports `If-None-Match` (304); 404 if absent.
  Authentication runs before conditional responses, including cached versions.

Uploads are bounded to 512 KiB, including streamed bodies without Content-Length
(413 `payload_too_large`). Magic bytes must match JPEG/PNG/WebP and the declared
Content-Type (415 `unsupported_media_type`). Missing authentication returns 401;
bearer writes or missing/mismatched cookie Origin return 403. Errors retain the
normal `error.code` / `error.message` envelope. Images are BLOBs in D1 migration
`0019_user_avatars.sql`; no R2 binding is needed. The account UI accepts originals
up to 5 MB, decodes and center-crops to 256×256 with canvas, then encodes WebP
(JPEG fallback), targeting less than 100 KiB before upload.
