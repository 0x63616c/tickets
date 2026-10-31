# Dedicated Analytics · TIK-66

Operate mode, inheriting Tickets' dense, near-black shell and tokens. Home is for
acting now: retain In flight / In review / Blocked, the two queues, and a compact
project waiting table. Analytics is for comparing flow and inventory.

## Reference review

- [Linear Insights](https://linear.app/docs/insights): pair visual summaries with
  exact tabular data and links to the contributing tickets; keep scope explicit.
- [Vercel Analytics](https://vercel.com/docs/analytics/using-web-analytics): a
  dedicated navigation destination and a quiet, shared project scope.
- [Tremor bar charts](https://www.tremor.so/docs/visualizations/bar-chart): shared
  zero-based scales, restrained gridlines, consistent legends and value tooltips.
- [shadcn charts](https://ui.shadcn.com/docs/components/base/chart): one reusable
  card frame, coherent tooltip treatment and an accessible alternative to marks.

## Decisions

Two ChartCards share title, subtitle, metric, legend, plot, table and coverage
footer. Paired weekly bars compare native-created and observed-completed work;
stacked horizontal inventory bars compare projects on one absolute count scale.
Counts are integers with four equal tick intervals. Status names accompany color.
SVG points are keyboard reachable and expose the same exact-value tooltip on
focus and hover; expandable tables provide drill-down links and complete values.
A shared project selector scopes both charts and the URL. No chart library or new
runtime dependency is needed for these two small SVG plots.

Use only GET /api/v1/analytics/home and the shared deduplicated read cache from
TIK-71. Keep one initial settlement gate for the rollup and sidebar projects,
matching TIK-65. No polling, full ticket collection load or route-local timer.
Retain saved charts during refresh/errors; project/filter results commit together.
Use the existing ascii-reveal wrapper once on settlement, respecting reduced
motion. Skeletons reserve the same card/plot geometry. Tables, empty states and
history gaps are first-class states.

Coverage stays honest: null is unavailable, partial weeks start at capture, the
current week is in progress, and imported arrivals are excluded from native
creation. Cycle time, priority distributions and stale-age charts are deferred:
the cached contract has no such fields; deriving them from ticket scans or
updated_at would violate the metric and performance contracts.
