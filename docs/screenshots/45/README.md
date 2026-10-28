# Issue #45: create labels from ticket detail

Dark mode, DPR 1, local Worker/D1, fonts loaded. Desktop is 1440×900;
mobile is 390×844 and scrolled to the properties rail. Captures use the
actual `/ticket?id=2` route with the same three projects, ticket title,
status and empty labels.

- `before-*.png`: origin/main `908fad8c31a860e7ced16a177e9c05246bbcf492`,
  built in `/tmp/tickets-45-before`; Add label opens an empty menu.
- `after-*.png`: name entered with no existing labels; Create label is available.
- `attached-*.png`: Feature (Enter) and Bug (click) created and attached.
- `target-desktop.png`: `/design/ticket` fixture reference, sample data only.

Reproduce with `CHROME_BIN=<headless-chromium> cargo xtask projects-check`.
Set `TICKETS_BEFORE_ASSETS` to the absolute origin/main `crates/web/dist`
from a temporary worktree for baseline captures; remove that worktree afterward.
The browser check also verifies filtering, blank-name suppression,
case-insensitive reuse without duplicates, arrow keys, Enter, Escape, trigger
focus restoration and persistence after reload. No API label seed is used.

The shared screenshot helper uses the current scroll coordinates so mobile
captures show the actual viewport. System Chrome on this box has a managed
SOCKS proxy; captures used a temporary Playwright headless Chromium executable
under `/tmp`, via `CHROME_BIN`. No browser tooling is committed.
