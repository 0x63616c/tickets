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
CHROME_BIN. Shared machine policy was not modified. The fixture server binds an
OS-assigned port; local Worker checks wait for port 8795 rather than stopping
another worktree's processes. Rust checks use this worktree's target directory
because shared target output was being replaced by parallel builds.

Review: [critique and polish](critique-and-polish.md). The committed metric and
scope contract is [Home analytics](../../design/home-analytics.md).
