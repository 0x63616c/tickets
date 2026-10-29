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
instance (projects 1/2, tickets 1/2, label/comment 1). HTTP expectations and JSON
subsets in comments are checked by `cargo xtask skill-check` against local D1.

## Access and project selection

Check access, list projects and choose an appropriate unarchived project. Create
one only if needed; prefixes are 1–10 uppercase ASCII letters, colors `#RRGGBB`.
Link with `github_owner`, positive `github_project_number`, `github_project_kind`
(`users` default or `orgs`), and optional `github_repo` (repository name).
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
# expect {"status":200,"body":{"data":[]}}
curl -sS "$TICKETS_URL/api/v1/labels" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":201,"body":{"id":1,"name":"bug"}}
curl -sS "$TICKETS_URL/api/v1/labels" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"name":"bug","color":"#dd6677"}'
# expect {"status":200}
curl -sS -X POST "$TICKETS_URL/api/v1/tickets/1/labels/1" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":200}
curl -sS -X POST "$TICKETS_URL/api/v1/tickets/1/dependencies/2" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":409}
curl -sS -X POST "$TICKETS_URL/api/v1/tickets/2/dependencies/1" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":200,"body":{"labels":[{"id":1,"name":"bug"}],"blocks":[2]}}
curl -sS "$TICKETS_URL/api/v1/tickets/1" -H "Authorization: Bearer $TICKETS_TOKEN"
```

## Lists, filters, search and pagination

All domain lists return `data` and `next_cursor` (null at end). Pass the returned
cursor unchanged for the next page; default limit 50, range 1–100. IDs ascend,
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
  --data-urlencode 'project=1' --data-urlencode 'status=in_progress' --data-urlencode 'label=1' \
  --data-urlencode 'assignee=agent' --data-urlencode 'blocked=false' --data-urlencode 'q=render bug'
# expect {"status":200,"body":{"data":[{"id":2,"blocked_by":[1]}]}}
curl -sS -G "$TICKETS_URL/api/v1/tickets" -H "Authorization: Bearer $TICKETS_TOKEN" \
  --data-urlencode 'blocked=true'
# expect {"status":200}
curl -sS -X DELETE "$TICKETS_URL/api/v1/tickets/1/dependencies/2" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":200}
curl -sS -X DELETE "$TICKETS_URL/api/v1/tickets/1/labels/1" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":200,"body":{"blocks":[],"labels":[]}}
curl -sS "$TICKETS_URL/api/v1/tickets/1" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":200,"body":{"id":1,"project_id":2,"key":"DST-1","assignee":null}}
curl -sS -X PATCH "$TICKETS_URL/api/v1/tickets/1" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"project_id":2,"assignee":null}'
```

## Comments

Bodies are CommonMark + GFM Markdown. Responses include raw `body` and sanitized
`body_html`; render only the latter as HTML. User comments use author `owner`;
PR activity uses `GitHub` or `system`;
callers cannot choose an author. Use returned comment IDs for edits/deletion.

```sh
# expect {"status":201,"body":{"id":1,"body":"Ready for **review**."}}
curl -sS "$TICKETS_URL/api/v1/tickets/1/comments" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"body":"Ready for **review**."}'
# expect {"status":200,"body":{"data":[{"id":1}]}}
curl -sS "$TICKETS_URL/api/v1/tickets/1/comments" -H "Authorization: Bearer $TICKETS_TOKEN"
# expect {"status":200,"body":{"body":"Reviewed."}}
curl -sS -X PATCH "$TICKETS_URL/api/v1/comments/1" -H "Authorization: Bearer $TICKETS_TOKEN" \
  -H 'Content-Type: application/json' -d '{"body":"Reviewed."}'
# expect {"status":200,"body":{"ok":true}}
curl -sS -X DELETE "$TICKETS_URL/api/v1/comments/1" -H "Authorization: Bearer $TICKETS_TOKEN"
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

## Errors

Inspect HTTP status and `{"error":{"code":"...","message":"..."}}`, not just
curl's exit code (`-sS` still exits zero for HTTP errors). 400 malformed request,
401 authentication, 403 cookie Origin failure, 404 missing resource, 409 conflict,
422 invalid fields/references, 500 storage failure. Correct invalid input before
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
Ordinary REST mutations automatically notify watchers; no separate publish call
is required, and notification failures never fail a committed mutation.

## GitHub mirrors

Tickets with `external_id` beginning `github:` are read-only for agents too.
Make changes on GitHub using `external_url`; webhook/cron ingestion overwrites
local edits. Never mutate mirror tickets, labels or comments through this API.
The API still permits writes for the importer. Both voxelite and quantum use
`github:0x63616c/voxelite:N`; preserve issue identities and comment markers.
GitHub author logins appear in imported comment bodies; render sanitized HTML.
The signed webhook endpoint is documented in docs/API.md and is not an agent
write endpoint. GitHub remains the source of truth; nothing syncs back.


## Pull request links and job status

Read `GET /api/v1/tickets/{key}` to obtain `status` and `pull_requests` for job
tracking. PR records include `repo`, `number`, `url`, `title`, `state`
(open/draft/merged/closed), `closes`, `head_sha`, and `updated_at`. Do not infer
completion from a reference PR: only merged closing links mark a ticket done.

End PR titles with `[KEY-N]` and include `Closes KEY-N` (or Fixes/Resolves) in
the body. Use `Refs KEY-N` for reference-only links; it overrides a title bracket.
Multiple existing ticket keys can be linked. Signed GitHub events from configured
`GITHUB_PR_OWNERS` update links and statuses independently of the issue mirror.
Drafts start work, open PRs request review, closing merges finish work, and an
unmerged close returns active tickets to todo only when no open/draft PR remains.
Done/canceled tickets are protected from open/draft events; canceled never completes.
Edits remove links for removed keys. Read back after webhook delivery; older events
and redeliveries are ignored. The webhook is not an agent write endpoint.
