# TIK-71 measurements and caller audit

Baseline: `b42be78` (TIK-65 + TIK-72). Implementation also merges TIK-75 loading exclusivity.
Cloudflare baseline supplied by Calum: 2,670,730 rows/day, 5,472 Worker requests,
average CPU 12.7 ms/request (p50 4.76 ms, p99 74.9 ms).

## Measured local replay

| Metric | Before | After |
|---|---:|---:|
| D1 rows read per modeled session | 90,270 | 26,600 |
| SQL statements (including auth/version and three after-side updates) | 110 | 49 |
| Local D1 statement duration, ms/session | 20.0 | 9.0 |

**70.53% fewer rows.** Overall proportional projection: 786,988
rows/day. A more conservative projection holds all queries outside the three
largest families fixed: `2,670,730 - 2,414,178 + 2,414,178 × 26600/90270` =
**967,942 rows/day**, below 1M. These are workload-mix projections, not a
post-deployment production measurement. A write-heavy workload or cold-cache
traffic in many Cloudflare locations can change the result.

Run `cargo xtask perf-check`. This uses a disposable, loopback-only wrangler
Worker with local D1. It sums actual `D1Result.meta.rows_read`; wrangler CLI
`d1 execute --json` strips those fields, so CLI duration alone is insufficient.
Raw SQL and metadata live in ignored `target/d1-perf-<pid>/`; the committed
`TIK-71-rows.json` includes measurements and EXPLAIN plans.

The deterministic schedule uses 240 synthetic tickets, five projects, labels,
comments and dependencies: Home initial load + room open; Board, List and detail
mount + room reopen; return to Home/Analytics; five visible idle minute ticks;
three separated live ticket updates. Before replays 12 Home snapshots and six
collection loads. After replays one collection, the initial version handshake,
one Home snapshot and three conservative cold Home snapshots + grouped counts
+ point reads after writes. Idle ticks are elided after. It models the high-volume
query families and session auth; detail thread/history, static assets, socket
upgrades and mutation write overhead are outside the replay. Actual browser
request-count assertions run separately in `perf-browser-check`.

The replay intentionally charges cold analytics SQL after every update; it does
not assume edge hits. Repeat views within the unchanged workspace/minute instead
read only session auth and the singleton version. HTTP cache acceptance verifies
identical warm bodies, empty 304, auth before cache, changed tags after mutation,
and invalid-parameter rejection.

## CPU interpretation

Production before: **12.7 ms/request**. Production after: **unmeasured**, target
**<10 ms/request**. Local D1 duration is not Worker CPU and must not be labeled as
such. This PR does not deploy; the coordinator must verify the CPU target in
Cloudflare after the normal CI deployment. No credible CPU number can be derived
by multiplying rows read by CPU. The implementation removes minute polling,
repeated full-ticket Markdown rendering and JSON serialization on edge hits;
cached responses stream their stored body. Added work is a single version-row
lookup plus a small URL/version hash. Version triggers add write work, which
must be included in post-deployment observation.

## Every caller of the five baseline query families

1. **Full ticket list** (1,015 runs / 1,256,477 rows): `BoardState::fetch` is the
   sole web collection loader (`all::<BoardTicket>("tickets")`). Board, List,
   detail, Projects, Settings, palette project creation and Home's create action
   invoke `refresh`. Previously every invocation downloaded the collection;
   the shell's global store now loads it once and reuses it. Explicit error
   recovery and reconnect still reconcile authoritatively. `live::subscribe`
   previously invoked the loader every visible 60 seconds, on focus/visibility,
   every first room open, reconnect, and each event; route-dependent subscriptions
   reopened rooms during navigation. Timer polling and healthy focus reloads are
   gone. Rooms stay subscribed across routes; a version check closes the initial
   snapshot gap. Events patch one ticket, dependency peers, or metadata.
2. **Projects with counts** (948 runs / 619,195 rows): `BoardState::fetch`, Home's
   `all::<Project>("projects")`, and Sidebar's ticket-signal effect each queried
   it. Every optimistic ticket update triggered another sidebar project load.
   That effect is removed; shared/coalesced reads feed Home and the shell. Counts
   now use one indexed ticket pass grouped by project/status. Event batches
   refresh metadata once. Ticket creation/patch project validation uses a point
   existence lookup, avoiding aggregate work during writes.
3. **Weekly analytics** (126 runs / 538,506 rows): `HomePage` is the sole browser
   caller. Query changes, manual refresh, initial live opens, minute/focus recovery
   and live invalidations reran the CTE. Home query responses now share a bounded
   session cache; explicit refresh, mutations and live events invalidate it. The
   Worker caches serialized rollups for 60 seconds by workspace version and UTC
   minute. Existing `/analytics/home` weeks/cohort shape is documented for TIK-66.
4. **Compact list without PRs** (28 runs / 28,464 rows): current code is ranked
   `/search`, called by palette search and suggestions. The detail dependency picker uses the shared ticket store.
   It remains necessary: replacing fuzzy search with full ticket downloads would
   increase work. Repeated identical searches now use bounded client caching and
   the versioned edge cache. Cold fuzzy searches remain scans; project/status
   filters and joins remain available. No second legacy full-list loader exists.
5. **Full dependency graph** (114 runs / 16,449 rows):
   `Repository::dependencies`, called by `add_dependency` to run core graph checks
   before the atomic SQL cycle trigger. This is mutation validation, not web
   polling; retained. Ticket-list relationship aggregates use indexed lookups.

## Query plans and correctness

`EXPLAIN QUERY PLAN` confirms `tickets_project_status_activity` is a covering
index for grouped counts; list keys use a project PK join; visible comments use
`comments_visible_ticket`; label membership uses its composite primary key;
dependency lookups use forward/reverse indexes; PR/check lookups use composite
indexes. Sparse ordered label/dependency/PR aggregates deliberately remain indexed
correlated probes, avoiding row multiplication or aggregating the entire workspace
for a small page. `ticket_key` keeps indexed prefix/number lookup. A reverse label
index supports label-first queries. Migration 0017 is additive; triggers include
older writers, comments, label membership, dependencies and PR/check updates.

Tests cover old-writer invalidation, indexed plans, existing query budgets,
counts/analytics history semantics, HTTP cache security, shared browser stores,
loader continuity, navigation disposal and live reconciliation.

## Impeccable critique and polish

Two independent assessments reviewed the data/loading changes against DESIGN.md
and the existing desktop/mobile Home PNGs. Mechanical detector: exit 0, `[]`.
The polish pass fixed hidden-tab queue flushing, failed-cache eviction, explicit
Reload/Retry behavior, visible reconciliation errors, and reconnect invalidation
of an unused ticket collection. Layout, tokens and visual hierarchy are preserved.
Questions skipped: fixed implementation scope and existing approved design.
