# TIK-135 project color picker

Fixture-only dark captures of the real project editor, opened from the sidebar.
Before shows the prior unlabeled buttons and always-visible hex input. After
shows hue names, Graphite selected with a checkmark and keyboard focus, plus
Custom revealing the existing hex input. Desktop is 1440×1000; mobile is 390×844.
Fonts are loaded before capture. No live account data is used.

`cargo xtask project-colors-shot` uses the shared fixture server and Chromium
screenshot harness. `TICKETS_BEFORE_ASSETS` selects pre-change built assets.
The harness checks arrow keys, visible row focus, Custom save/reopen and
case-insensitive matching of a saved palette hex. Its Graphite measurement is
foreground `rgb(138, 143, 152)` against the actual 12% tinted dark tile,
`color(srgb 0.089098 0.091451 0.0991373)`: **5.50:1**, exceeding the 4.5:1 check.

WebKit uses the existing Playwright 1.64.1 installation and WebKit build 2370.
The smoke opens `/design/settings/projects`, `/design/projects` and
`/design/board`, then exercises the same editor's native radio arrow keys,
Graphite and Custom save/reopen, plus 44px rows with a coarse touch pointer.
It fails on any console or page error.
Local fixture servers and browsers are stopped on exit.

The final browser checks use a normal debug Trunk frontend build, stripped and
optimized with `wasm-opt` to fit Workers’ asset limit (8,240,621 bytes), and the
local release Worker. Full local CI checks the final source. The release picker was
also built and checked before the one-line archive focus fallback correction.
The final release rebuild was canceled while queued behind other jobs in the
shared Cargo cache.

Passed (exit 0): full `cargo xtask ci`, CSS lint, final frontend build/optimization,
`project-colors-shot`, `projects-check`, `first-run-check`, and the WebKit smoke.
The Chromium checks use a test-process-only direct proxy policy override.

Impeccable critique and polish were performed in one context, following the
instruction not to launch another Codex process. The source detector returned
`[]`; rendered desktop/mobile review, keyboard checks, measured contrast and
WebKit touch checks found no remaining picker issues. The required visible list
adds form height on mobile; the existing dialog scroll keeps every field reachable.
