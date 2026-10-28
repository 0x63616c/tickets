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

The domain REST routes are introduced by #5; do not assume they exist yet.
Browser sessions use HttpOnly cookies; cookie mutations require matching Origin.
