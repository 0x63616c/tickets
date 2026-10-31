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
project. Label edits notify projects using that label; an empty scope does not
broadcast. Project metadata changes notify their room. Claim heartbeats and
CI-only check updates do not send notices. Notification runs after the D1 commit and
errors are isolated from the mutation response. A failed notification can leave
an already-connected client stale until reconnect or manual reload; there is no
transactional outbox. This is an intentional day-one tradeoff.

Leptos shares ticket state across routes and subscribes to workspace projects.
Incoming invalidations update affected records through authoritative REST reads.
Automatic reconciliation batches notices for 5 seconds and runs at most once per
30 seconds per tab; hidden-tab events wait until visible, and local edits defer
reconciliation. Reconnects refetch authoritative state because live events have
no replay. Retries use exponential backoff (500 ms to 30 s, plus up to 50% jitter).
The Connected / Reconnecting / Offline indicator reflects transport availability.
See [the API caching and live contract](API.md#read-caching-and-weekly-analytics-tik-71)
for cache versions, refresh behavior and metadata visibility limits.

## Hosting and limits

The maintainer instance uses Workers Paid. SQLite Durable Objects are also
available on Workers Free; account allowances and pricing references are recorded
in [DEPLOY.md](DEPLOY.md#free-tier-limits-worth-knowing). Choose and monitor the
plan for your own workload; this architecture does not require staying on Free.

Opening a socket and each Worker notification consume requests. Outgoing frames
are uncharged. Idle hibernation-eligible sockets incur no duration charges.
Large project counts multiply sockets and notification work, so monitor request
counts before expanding usage. D1/Worker limits remain separate.

## Local verification

`CHROME_BIN=<headless Chromium> cargo xtask live-check` builds the Rust app,
creates isolated local D1 state, and starts local Wrangler on port 8795. It checks
401 on unauthenticated upgrade, authenticated two-client broadcast, a browser
move appearing in another window, list/detail updates and reconnect recovery.
Screenshots and the two-window sequence live in `docs/screenshots/16/`.
Use `TICKETS_BEFORE_ASSETS` pointing to main's built assets for baseline captures.
Never deploy or run remote migrations for this check.
