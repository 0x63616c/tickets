# GitHub mirror detail

Dark-mode screenshots of the real `/ticket?id=1` route, using an imported issue
with a GitHub comment, assignee and label. No production data or credentials.

- Before: frontend assets built from `origin/main` at `daea3e5` in a temporary
  detached worktree under `/tmp`, then served by the isolated local Worker/D1
  lifecycle with `TICKETS_BEFORE_ASSETS`. That temporary worktree was removed.
- After: this branch, same seeded issue, route, loaded fonts and viewports.
- Desktop: 1440×900. Mobile: 390×900.
- `target-desktop.png`: separate `/design/ticket` fixture reference, not a baseline.

Historical captures only: the issue mirror was retired on 2026-10-29.
`cargo xtask mirror-check` now verifies native imported tickets and captures
`docs/screenshots/tik-33/`. The original check verified the read-only
controls, keyboard no-op, board drag attribute, list controls, and captures after.
The check first verifies two issues (including closed history) and PR exclusion,
then removes the second issue to retain the baseline screenshot seed.

The box's managed Google Chrome forces a proxy; these captures use the installed
Playwright Chromium binary through `CHROME_BIN`. No policy changes are needed.
