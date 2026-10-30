# Home analytics — TIK-43, first version

Home answers **what needs review or unblocking, and is work finishing?** It is
an Operate surface within the existing dense dark shell. It becomes `/` and
`/home`; Board moves to `/board`. The metric strip leads, then two charts and
two five-row queues. All measurements have exact ticket drill-downs.

This is the trimmed implementation of the Phase 1 handoff. It deliberately avoids
activity-volume scores, invented historic completion dates, and cycle statistics
with unknown starts. It uses existing tokens, shared SelectField, ProjectIcon,
Skeleton and AsciiSpinner. Charts have no entrance or count-up animation.

## Metrics

Default scope is all currently unarchived projects; one active project can be
selected. Archived blockers still block their active dependents. Open means
backlog, todo, in_progress or in_review; done and canceled are terminal.
Inventory categories follow Board/List: a Todo ticket tagged `later` appears in
Backlog. Active/resolved statuses take precedence; stored statuses and history
remain unchanged.

| Measurement | Definition |
| --- | --- |
| In flight | Current in_progress + in_review tickets. |
| In review | Current in_review tickets, whether or not linked to a PR. |
| Blocked | Distinct open tickets with an unresolved incoming dependency. Done/canceled prerequisites resolve it. |
| Completed this week | Distinct durable ticket subjects transitioning from a non-done state into done during this UTC week. Reopened and deleted tickets remain in the cohort. Birth in done is excluded. |
| Open work by project | Current open counts, stacked by status on a shared zero-based count scale; includes empty projects. |
| Created / completed | Eight Monday-based UTC weeks, including the current week so far. Created counts native births captured after installation; imported arrivals are counted separately. Completed uses the KPI rule, deduplicated per ticket/week within scope. |

UTC is explicit throughout v1. Bucket ranges are half-open; current data includes
all observations through the response's `as_of` millisecond. History coverage is
`none`, `partial` (capture began during this week), or `full`. Unavailable values
are null and drawn as gaps/dashes. A measured zero is zero. Current week is always
labeled “So far”; no partial/full-week percentage comparisons are offered.

Historical scope uses the project **at the event** and its current archive flag;
current scope uses today's ticket project. Moving or reopening a completed ticket
does not rewrite history. Deleted subjects are visible as unlinked deleted rows.
Historical totals across projects need not sum after moves and re-completion.

## Persistence and safety

Migration 0011 adds a durable subject identity, coverage cutover, ticket events,
and PR observation history. Ticket birth/status/project/deletion triggers run in
the same statement as the mutation, covering REST, repository writes, imports and
nested webhook updates. No-op updates, reorders, comments and title edits do not
create status events. An event failure rolls back the domain mutation too.

Existing rows receive a baseline plus recovered local arrival evidence. Recovered
arrivals have no historical project or status attribution and are excluded from
v1 flow. Already-done baselines never count as completions. Existing PR snapshots
are baselines. The migration is applied once by the normal migration ledger/CI;
there is no manual production backfill or import.

PR history records first accepted observations, accepted state changes, and link
changes, including the accepted snapshot's links. Existing stale/replay/equal-time
precedence is unchanged. Title/head-only updates do not generate milestones.
`source_updated_at` is the snapshot version, **not** a merge or lifecycle timestamp.
No PR-merged metric is exposed. No raw webhook bodies, identities, credentials,
ticket bodies or comment bodies are copied to history. Generic ticket triggers
are the sole source of completion events, including PR-driven completion.

## Transport and interaction

`GET /api/v1/analytics/home` is authenticated and no-store. One parameterized SQL
statement aggregates current state, eight buckets, five-row queues and a selected
cohort in a coherent snapshot: one D1 call plus authentication, five bindings.
Indexes support event kind/time and subject history. No cache or client ticket
collection download is needed for Home.

Optional parameters: `project` (positive active ID), `metric` (in_flight, review,
blocked, inventory, created, completed), `week` (0–7, oldest first; only flow;
default 7), `status` (open status; only inventory), `cursor` (positive subject ID;
only with a metric). Unknown/duplicate/incompatible parameters return 422. Cohorts
use ascending subject IDs, 50 rows and `next_cursor`. They share Home predicates;
current cohorts can change while paging. Deleted historical subjects remain.

Chart counts and exact-cohort links are in keyboard-operable disclosures; SVG is
a static labelled graphic with a table equivalent, not a second interaction tree.
Metrics and queue links open an inline cohort using URL state. Numeric ticket
links survive moves. A new scope clears old data while loading; same-scope refresh
keeps saved data, timestamps and an inline retry error. Obsolete requests are
aborted. Shared live invalidation, focus/online and visible-tab catch-up refresh
Home without loading all tickets. The existing composer loads editing data only
when invoked.

## Deferred from Phase 1

- Cycle-time episodes, median/IQR and exclusions; stale-work thresholds/age clocks.
- General activity feed and comment/label/dependency audit events.
- Source-timed PR lifecycle facts, webhook action/delivery metadata and recovered
  comment parsing; PR observations currently retain key-based snapshot membership.
- IANA/browser timezone selection and 4/12-week controls (v1 is eight UTC weeks).
- Archived-project analytics and the legacy blocked-label diagnostic.
- Priority grouping, interactive SVG marks/roving chart focus, cache and exports.

These are omitted UI/features, never fabricated measurements or deleted data.
The additive history provides reliable local status evidence for later work.
