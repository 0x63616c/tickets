# Issue #36 — catch-up and count plurals

Captured by `cargo xtask live-check` against isolated `wrangler dev` + local D1,
using installed Playwright Chromium, dark mode and loaded bundled fonts.
Desktop PNGs are 1440×900; mobile PNGs are 390×900. The shared sidebar stays 240px.

- `board-before-desktop.png` and `list-before-desktop.png`: real `/` and `/list`
  from origin/main `5b7e9a0`, release-built in a temporary worktree and served via
  `TICKETS_BEFORE_ASSETS`. Both show the original **1 tickets** footer.
- `board-after-desktop.png` and `list-after-desktop.png`: same seed, routes and
  viewport with **1 ticket**. Mobile board captures are supplementary.
- `board-target-desktop.png`: separate `/design/board` fixture reference from main;
  it is sample data, not the real-route baseline.
- `demo-01-window-{a,b}.png` / `demo-02-window-{a,b}.png`: independent authenticated
  Chromium sessions before/after a live status move.
- `visibility-catchup-desktop.png`: session B returns from a real background tab
  after A changes status. The harness suppresses received WebSocket messages,
  verifies zero REST reads during a full 61-second hidden interval, then verifies
  one three-read refresh for the visibility/focus/online burst, with no new socket.
- `periodic-catchup-desktop.png`: notifications remain suppressed; a later write
  appears through the normal 60-second visible interval, again without reconnect.

The same automated check covers each visibility/focus/online trigger independently,
keyboard-drag deferral, unsaved title preservation while a modal is open, subsequent
reconciliation, list/detail live events, upgrade authentication and offline reconnect.
No production data, credentials or tokens are included in these images.
