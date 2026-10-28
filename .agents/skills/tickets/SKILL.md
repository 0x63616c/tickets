---
name: tickets
description: Authenticate to the Tickets API with a personal API token.
---

# Tickets API

Obtain a personal token from the workspace owner via Settings → API tokens.
Read the base URL and token from the caller's secure environment; never hardcode,
print, commit, or put credentials into URLs. Do not request the owner's password.

Send `Authorization: Bearer <token>` with every private API request. Check access
with `GET /api/v1/me`. A 401 means the token is invalid or revoked; stop and request
replacement access rather than repeatedly retrying. Health is public at `/api/health`.

Token management (only when requested):
- `GET /api/v1/tokens`: metadata with id, name, created_at, last_used_at, revoked_at.
- `POST /api/v1/tokens` with JSON `{ "name": "<agent-name>" }`: returns `token`
  once and `metadata`. Deliver the raw token only through a secure channel.
- `DELETE /api/v1/tokens/<id>`: immediately revokes access, including your own token.

Domain routes are documented in `docs/API.md`. Lists return `data` and
`next_cursor`; pass the cursor and a `limit` of 1–100 for subsequent pages.
Use `/api/v1/projects`, `/tickets`, `/labels` for list/create, and append an ID
for get/PATCH/delete (projects archive with PATCH `{"archived":true}`).
Tickets also accept keys such as `EX-12`. Filter tickets by `project`, `status`,
`label`, `assignee`, `blocked=true|false`, and `q` (title/body full-text phrase).
Ticket create requires `project_id` and `title`. Status values are backlog, todo,
in_progress, in_review, done, canceled; priority is 0–4 (none to low).
Attach/detach labels with POST/DELETE `/tickets/{id}/labels/{label_id}`.
Add/remove outgoing blocker edges with POST/DELETE
`/tickets/{blocker}/dependencies/{blocked}`. A cycle, self-edge or duplicate
returns 409; correct the graph instead of retrying blindly.
List/create comments at `/tickets/{id}/comments`; get/PATCH/delete a comment at
`/comments/{id}`. Comment input is `{"body":"Markdown"}`.
Import with PUT `/tickets/by-external?id={URL-encoded-source:id}`, including
`project_id`, `title` and optional `external_url`. Repeating the same external ID
updates the ticket without duplicates. Use namespaced IDs, e.g. `github:org/repo:12`.
Errors contain `error.code` and `error.message`; 400 is malformed request,
404 missing resource, 409 conflict, 422 invalid fields. Raw Markdown is in `body`;
`body_html` is sanitized. Never render raw Markdown as HTML.
Browser sessions use HttpOnly cookies; cookie mutations require matching Origin.
