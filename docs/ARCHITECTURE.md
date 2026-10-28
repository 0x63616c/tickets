# Live updates

D1 is the source of truth. After a successful domain mutation, the authenticated
Worker sends a small typed invalidation to `BOARD_ROOMS.id_from_name(project_id)`.
The SQLite-backed `BoardRoom` broadcasts to `State::get_websockets()`. It accepts
sockets with `accept_web_socket`, uses hibernation message/close/error handlers,
and keeps no in-memory socket registry, timers, alarms, or polling tasks. SQLite
is selected by `new_sqlite_classes`; no duplicate ticket data or replay log is
stored in the object. The runtime retains the live connection state across sleep.

The only public path is `/api/v1/live/{project_id}`, behind the shared auth gate
and an existence check. Session upgrades also require matching Origin to prevent
cross-site subscription. Bearer tokens stay in headers. Day one is a single owner:
all valid credentials see all projects. Future project ACLs must be enforced at
upgrade and must close existing subscriptions when permissions change.

Ticket writes notify the current, previous and dependency-related projects.
Dependency links notify both endpoint projects. Comments notify their ticket's
project. Labels invalidate every project because labels are global. Project
metadata changes notify their room. Notification runs after the D1 commit and
errors are isolated from the mutation response. A failed notification can leave
an already-connected client stale until reconnect or manual reload; there is no
transactional outbox. This is an intentional day-one tradeoff.

Leptos subscribes to the selected board/detail project, or all projects for list.
Subscriptions are canceled on scope changes. Existing optimistic local edits stay
in place; incoming invalidations coalesce and refetch authoritative REST state.
Own echoes and duplicates are harmless. Board reconciliation waits for moves and
dragging to settle. Detail reconciliation waits for writes and the ticket editor.
Every connection opening triggers a refetch to recover missed messages. Retries
use exponential backoff (500 ms to 30 s, plus up to 50% jitter). The existing
Connected / Reconnecting / Offline indicator reflects transport availability.

## Free tier

Keep the account on Workers Free. [Cloudflare pricing](https://developers.cloudflare.com/durable-objects/platform/pricing/)
currently includes 100,000 DO requests/day and 13,000 GB-s/day. SQLite includes
5 million rows read/day, 100,000 rows written/day and 5 GB total storage.
[Limits](https://developers.cloudflare.com/durable-objects/platform/limits/)
include 100 classes on Free and 1 GB per object. Limits reset daily at 00:00 UTC;
exceeding them fails operations rather than enabling paid overages.

Opening a socket and each Worker notification consume requests. Outgoing frames
are uncharged. Idle hibernation-eligible sockets incur no duration charges.
One active class and no persisted replay data keep usage small for one owner;
large project counts multiply list sockets and global-label fanout, so monitor
request counts before expanding usage. D1/Worker limits remain separate.

## Local verification

`CHROME_BIN=<headless Chromium> cargo xtask live-check` builds the Rust app,
creates isolated local D1 state, and starts local Wrangler on port 8795. It checks
401 on unauthenticated upgrade, authenticated two-client broadcast, a browser
move appearing in another window, list/detail updates and reconnect recovery.
Screenshots and the two-window sequence live in `docs/screenshots/16/`.
Use `TICKETS_BEFORE_ASSETS` pointing to main's built assets for baseline captures.
Never deploy or run remote migrations for this check.
