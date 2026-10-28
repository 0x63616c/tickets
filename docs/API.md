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
50, maximum 100, minimum 1. Results use ascending numeric IDs; cursors are exclusive.
Pagination is not a snapshot: concurrent updates can change matching results.
IDs and relationship IDs are positive integers. Unknown mutation fields are rejected.
PATCH changes only supplied fields; explicit null clears nullable fields.

## Projects

| Method | Path | Document / behavior |
| --- | --- | --- |
| GET | `/projects` | Paginated projects, including archived |
| GET | `/projects/{id}` | Project |
| POST | `/projects` | Required `name`, `key_prefix`, `color` |
| PATCH | `/projects/{id}` | Any editable project fields, including `archived` |

Project fields: `name`, `key_prefix` (1–10 uppercase ASCII letters), `color`
(`#RRGGBB`), `archived` (boolean), nullable `github_owner`, `github_repo`,
`github_project_number` (positive integer). Archive with `{"archived":true}`;
restore with false. Archive does not delete tickets or prohibit imports.
Changing a prefix changes the displayed keys for that project's existing tickets.

```sh
curl -sS "$TICKETS_URL/api/v1/projects" -H "Authorization: Bearer $TICKETS_TOKEN"
curl -sS "$TICKETS_URL/api/v1/projects" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"name":"Example","key_prefix":"EX","color":"#789abc"}'
curl -sS -X PATCH "$TICKETS_URL/api/v1/projects/1" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"github_owner":"example-org","github_repo":"example","github_project_number":7,"archived":true}'
```

## Tickets

| Method | Path | Document / behavior |
| --- | --- | --- |
| GET | `/tickets` | Paginated tickets with filters below |
| GET | `/tickets/{id-or-key}` | Numeric ID or key such as `EX-12` |
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

Responses contain raw `body` and sanitized `body_html`. Single-user author is
`owner`; callers cannot spoof authors through request fields.

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

After each successful domain mutation, `api::change_event` receives a
`ChangeEvent { entity, id, action }`. For label/dependency links, ID is the
originating ticket. This no-op hook is the integration point for #16.
