# TIK-43 Home evidence

Before: real `/` (Board) from merged main **6f1b42a**, built before UI edits.
After: real `/` (Home), with the same immutable fixture project/ticket dataset.
No production data, token values or personal profiles are used. Account is Sample
User, prefixes and titles are fixture values. The `/design/home` preview shares
the illustrative Home data.

Fixtures: `docs/fixtures/home-api.json` and `home.json`; eight weeks ending
2026-10-29, two unavailable history weeks, five projects, review/blocked queues.
Historical fixture events are illustrative independent observations, not dates
inferred from today's status. Both full viewport pairs are 1440×900 / 390×900.
Additional crops expose the chart/data table, initial/refresh error, loading and
first-project states. The mobile-full image includes the lower queues.

Reproduce:

```sh
cargo xtask build
CHROME_BIN=/usr/bin/google-chrome cargo xtask home-shot
TICKETS_BEFORE_ASSETS=/absolute/base/dist CHROME_BIN=/usr/bin/google-chrome cargo xtask home-shot
```

This box's managed Chrome and ungoogled Chromium proxy policy rejected loopback
(`ERR_SOCKS_CONNECTION_FAILED`). Captures use the installed Playwright Chromium
headless shell (`chromium_headless_shell-1248/chrome-headless-shell-linux64/`) via
CHROME_BIN. Native tab-visibility checks use the installed full Playwright
Chromium (`chromium-1248/chrome-linux64/chrome`); headless shell keeps both tabs
visible. Shared machine policy was not modified. The fixture server binds an
OS-assigned port; local Worker checks wait for port 8795 rather than stopping
another worktree's processes. Early checks used this worktree's target directory
after shared output was replaced by parallel builds. Final checks use `.job.env`'s
shared target following TIK-45's runtime workspace-resolution fix.

Review: [critique and polish](critique-and-polish.md). The committed metric and
scope contract is [Home analytics](../../design/home-analytics.md).

Final local validation passed: `cargo xtask ci` (host/wasm Clippy, formatting,
workspace tests), `design-check`, `first-run-check`, `mirror-check`, `board-check`,
`list-check`, `detail-check`, `live-check`, `contrast-check`, `polish-check`,
`palette-check`, `projects-check`, `project-repos-check`, `skill-check`,
`component-shot all` and `home-shot`. First-run verifies actual completion counts
through an empty-D1 UI workflow; live-check verifies aggregate-only Home refresh.
