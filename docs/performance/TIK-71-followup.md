# TIK-71 follow-up: bounded live reads

Base audited: `5a7ea24` (main including PRs #105 and #114).
The live ticket and its comments could not be read: the Tickets hostname failed
DNS resolution in this environment. The audit uses the six newer acceptance
items supplied with the job. The original production CPU/rows targets cannot be
confirmed here, so the proposed PR uses **Refs TIK-71** pending that verification.

## Existing behavior retained

PR #105 already provides shared/coalesced browser stores, grouped project counts,
a private post-auth edge cache for projects and Home analytics (60-second TTL,
workspace-version invalidation, plus the analytics UTC-minute key), ETags, and
analytics rollups. PR #114 supplies safe 503 responses for D1 limits. These are
not reimplemented. Version changes can expire cached representations early;
cache locality and request URLs also affect hit rates.

## Changes and read reasoning

| Request/event | Before this follow-up | After |
|---|---|---|
| Warm projects/Home request | Auth + singleton version SELECT; cached body | Same: 2 D1 calls, no aggregate query |
| Cold projects/Home request | Auth + version + grouped-count/analytics SELECT | Same 3 D1 calls; analytics filters and reuses scoped work |
| Project/PR notice routing | Global fallback SELECT for projects; every room notified | Zero routing SELECTs; only affected rooms, deduplicated |
| PR link removal | Global recovery to reach removed links | Old/new links and dependency neighbors returned by the existing atomic mutation batch |
| Claim heartbeat | Durable renewal/history plus a live notice | Same durable renewal/history; zero notices or resulting tab reads |
| CI-only checks | Tickets check globally notified; other CI queried current-head tickets and sent point notices | Metadata still stored; zero notices and no notification lookup |
| Burst of notices | Reconcile after 350 ms, repeatedly during a burst | 5-second batching; at most one automatic reconciliation per 30 seconds per tab |
| Mission Control visible timer | 120 GETs/hour (every 30 seconds) | 20 GETs/hour (every 3 minutes); same query cost per GET |

The saved calls above are statement counts, not D1 rows_read. A point or version
lookup has very different row cost from an aggregate. Coalescing also saves
metadata requests, ticket hydration, Markdown rendering and response serialization.
Local writes and explicit refresh remain immediate; hidden-tab/edit/drag queues
remain deferred. CI-only metadata appears on the next ordinary/manual refresh.
Unused/new labels and new projects have no existing affected room; other tabs
learn them at their next metadata refresh.

The old index `(project_id,status,updated_at)` supports grouped counts, but cannot
serve a project/time activity range or ordering without the intervening status.
Additive migration 0020 adds `(project_id,updated_at)`. Local full-chain SQLite:

```text
EXPLAIN QUERY PLAN
SELECT id FROM tickets WHERE project_id=? AND updated_at>=? ORDER BY updated_at;
SEARCH tickets USING COVERING INDEX tickets_project_updated_at (project_id=? AND updated_at>?)
```

No temporary sort is required. No remote migration was run.

## Local evidence and limits

A full-migration Python SQLite comparison used 600 synthetic tickets across
three projects, all statuses/priorities, labels, cross-project dependencies,
completion events, deletion, and an archived project. All **264** combinations of
scope, metric, week, inventory status and cursor matched the old SQL (excluding
clock-dependent as_of/last bucket end). On one selected-project Home query,
SQLite VM instructions decreased from **86,959 to 70,765 (18.6%)**. This is a
local execution-work proxy, **not D1 rows_read, latency or Worker CPU**. A first
materialization variant increased work and was rejected before the final rewrite.
The final statement keeps inventory inline for covering indexes, filters history
before weekly joins, and materializes only scoped open-ticket work for reuse.

The actual PR UPSERT/RETURNING SQL was also exercised with SQLite: added/removed
links include both projects and cross-project dependency neighbors, unrelated
projects are excluded, and exact replay returns no notice scope.

Production analytics were unavailable; no post-deployment CPU or daily-read claim
is made. The previous supplied baseline/measurement is in [TIK-71.md](TIK-71.md).
The original <10 ms/request and <1M rows/day targets still need observation after
normal CI deployment. This job does not deploy or change plans/secrets.

## Validation continuation (PR #115)

The writable job environment reproduced the CI budget failure. The replay was
using today's analytics SQL on both sides, although ticket/project baseline SQL
was pinned to `b42be78`. This made an analytics optimization retroactively shrink
the historical denominator. The baseline now loads its analytics SQL from the
same commit, and migration 0020 is applied only after baseline measurement.
The request schedule, three separated updates, cold analytics charges, fixed
256,552 rows/day outside the top families, proportional factor 2,414,178 and
strict 1,000,000 rows/day ceiling are unchanged.

Correcting the baseline alone was insufficient: 90,270 before / 28,152 after
projected **1,009,448 rows/day**. The original stale-baseline replay measured
85,968 / 28,152 and projected **1,047,125 rows/day**. This exposed a real
all-projects analytics regression despite the earlier selected-project VM gain.

The query now computes blocker count and first blocker in one grouped LEFT JOIN
per scoped open ticket instead of two correlated dependency probes. Resolved
blockers remain excluded, cross-project blockers remain included, and unblocked
tickets retain a zero count/null first blocker. A regression exercises multiple,
archived-project, done and canceled blockers, plus empty/resolved-only edges.

Final local D1 replay: **90,270 before / 27,328 after** (110 / 49 statements),
projecting **987,411 rows/day** with the unchanged conservative formula.
See [the measured report](TIK-71-followup-rows.json). Reports are now saved and
printed even on budget failure, including the computed projection. Local D1
statement durations are diagnostic only; production CPU and actual daily reads
remain unverified. Keep Refs TIK-71.

Validation results are recorded in PR #115. No deployment, remote migration,
plan/secret change or manual merge was performed.
