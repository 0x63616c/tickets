# TIK-33: imported tickets are native

Base: `b1bb53d` (the parent of this work). Before assets were built from that
commit in a temporary detached worktree. Both captures use the same local D1
seed, real `/ticket?id=1` route, loaded bundled fonts, dark theme, and
1440×900 / 390×900 viewports. These are real API-backed screens, not fixtures.

| Viewport | Before | After |
| --- | --- | --- |
| Desktop | [before](before-desktop.png) | [after](after-desktop.png) |
| Mobile | [before](before-mobile.png) | [after](after-mobile.png) |

Reproduce using `CHROME_BIN=<headless-chromium> cargo xtask mirror-check`.
For before captures, set `TICKETS_BEFORE_ASSETS` to the base worktree's built
`crates/web/dist`. The check creates only isolated local D1 state. It verifies
signed issue events are ignored without altering the imported ticket, native
API edits, enabled board/list/detail controls, and original GitHub author display.

In this shared agent environment the checks ran in a private network namespace
to avoid another worktree's port 8795, with the host's managed proxy policy hidden
inside that process only. No system policy or production resource was changed.
