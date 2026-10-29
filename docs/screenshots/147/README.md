# TIK-36 stable ticket links

Before assets: `ba34327` (the base commit), built in a temporary detached worktree.
After assets: this branch. Both captures use the same empty-D1, UI-only first-run
flow: create FIRST, create two tickets, move the first to In progress, label the
second Release, and make the first block the second.

The before detail route is `/ticket?id=2`; the after route is `/t/FIRST-2`.
The detail editor, data, dark scheme, loaded fonts, and viewport are otherwise
identical. Desktop is 1440×900; mobile is 390×844. No fixture-only screen is used.

Reproduce with `CHROME_BIN=<headless-chromium> cargo xtask first-run-check`.
For before captures, also set `TICKETS_BEFORE_ASSETS` to the base commit's absolute
`crates/web/dist` directory. That mode skips only the newly introduced key-route
assertions. Normal runs check lowercase lookup, unknown-key recovery, dependency
navigation during held reads/saves, and the logged-out `/t/FIRST-2` login round-trip.
