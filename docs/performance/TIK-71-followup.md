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

## Verification status

Passed: `cargo fmt --all --check`, `git diff --check`, Node CSS value-policy test,
the standalone Rust timing-policy test, the 264-case SQLite comparison, PR scope SQL checks, and EXPLAIN above.

Added/updated Rust coverage: notice delay/cooldown/clock skew; heartbeat silence;
label edit/deletion scope; PR old/new/dependency scope and replay; activity index
plan. Live browser acceptance allows 40 seconds for the cooldown; the shared-store
check also rejects reads in the first 2 seconds of a notice burst.

Attempted but blocked: host/wasm clippy, workspace tests, xtask ci, token check,
perf-check, skill-check, first-run-check and perf-browser-check. Sccache startup is denied and, even without it, the
required job-managed shared Cargo cache is read-only. No alternate Cargo target
or worktree was created. npm ci fails DNS; CSS lint/browser/first-run checks and
before/after screenshots consequently remain unverified. No Chromium executable
is available. These checks must run in a writable job environment before merge.
The Tickets skill is mounted read-only, so its live-behavior paragraph could not
be updated; the canonical API and contributor documentation are updated.

## Critique and polish

⚠️ DEGRADED: single-context (optional consent step skipped per job instruction).
Impeccable context/detector engine download was unavailable. Source critique
against PRODUCT.md and DESIGN.md found no layout/token changes. The polish pass
preserves edit/drag/hidden-tab deferral, checks sign-in again after the timer, and
keeps manual refresh immediate. Visual inspection and new PNGs are blocked by
the build/runtime limitations above, so this is not a completed visual critique.
Questions skipped: the job explicitly requires autonomous completion without
optional consent; scope is the supplied performance checklist.

## Publication status

Publishing was attempted and failed: git add cannot create index.lock because
/workspace/repos/tickets/.git/worktrees/TIK-71 is read-only; gh pr create cannot
connect to api.github.com; the progress-comment POST fails Tickets DNS resolution.
There is no commit, push, PR or posted comment from this job. Edits remain in the
provided worktree. The reviewable PR body and comment payload are saved at
/tmp/TIK-71-pr.md and /tmp/TIK-71-progress.json; /tmp/TIK-71.patch includes all
tracked and new changes. The claim was not released.
