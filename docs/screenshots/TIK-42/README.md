# TIK-42 visual evidence

Baseline: `40ef5f9525721aa0281b408a552b6ea8456d13da` (origin/main at start).

All captures use immutable, local API fixtures via `cargo xtask project-repos-check`.
They render the actual `/projects` and `/t/TKT-42` UI with loaded bundled fonts,
dark mode, and identical fixture payloads before/after. No production data,
credentials, personal account details, or GitHub network requests are used.
`example/tickets` and `example/quantum` deliberately have null
`github_project_number` values. Aggregate counts exceed the loaded ticket list
so the screenshots also expose the previous client-side counting bug.

- `*-projects-desktop.png`: 1440×900 project page.
- `*-projects-mobile.png`: 390×900 project page; repo shortcuts remain available.
- `*-projects-table.png`: tightly cropped project table at 2× DPR.
- `*-detail-desktop.png`, `*-detail-mobile.png`: existing linked PR rendering,
  verified for both a closing open PR and a merged reference PR.

The API adds read-only `done_ticket_count` and nullable `last_activity_at`.
Last activity means the latest ticket update across all statuses; canceled
issues contribute activity but count toward neither open nor done.

## Review

Impeccable critique used independent design and detector assessments. The source
detector returned zero findings. The design pass identified inherited mobile CSS
hiding repositories, an unlabeled tablet activity date, and small touch links;
polish addressed these with scoped visibility rules, an activity label, and
44px repository link targets on phones. The established near-black palette,
Inter, shared ProjectIcon, compact desktop rows, and focus treatment are retained.
The rendered follow-up found no remaining blockers (30/40, Good). Questions were
skipped because the refinement has a tightly scoped, established brief.

Fixture captures use `/usr/bin/google-chrome` through the existing
`/workspace/tickets-polish/chrome-noproxy` wrapper, which overrides the managed
proxy policy only inside the child process mount namespace. Local Worker checks
use `/usr/bin/google-chrome` directly with the same isolated policy override, in
a separate network namespace to avoid other jobs' port 8795 listeners. A local
virtual Ethernet pair preserves the browser's online signal for live catch-up
acceptance; it provides no access to production.

Reproduce the baseline with `TICKETS_BEFORE_ASSETS=<base crates/web/dist>` and
`CHROME_BIN=<loopback-capable chromium> cargo xtask project-repos-check`; omit
`TICKETS_BEFORE_ASSETS` for the current assets.

## Validation

Passed: `cargo fmt --all --check`, host and wasm32 workspace Clippy with warnings
as errors, `cargo test --workspace`, `project-repos-check` before/after,
`component-shot icons`, `projects-check`, `first-run-check`, `contrast-check`,
`board-check`, `list-check`, `detail-check`, and `skill-check` (34 curl examples).
The contrast audit also corrected the shared sidebar shortcut from an inherited
10px override to the existing 11px `--text-xs` token.
