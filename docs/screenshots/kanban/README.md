# Kanban screenshot provenance

- `before-desktop.png`, `before-mobile.png`: recaptured from the real `/` route at
  origin/main `d446c24` (1440×900 and 390×900). Main was checked out in
  `/tmp/tickets-main-before` using `git worktree add`, built with `cargo xtask build`,
  and served with local `wrangler dev --port 8787`. `cargo xtask board-before`
  navigated to `http://127.0.0.1:8787/`, waited for fonts, and captured both sizes.
  The server was stopped and the temporary worktree removed afterwards.
- Main maps `/` to `pages::Board`, the same component as `/design/board`. Thus the
  independently recaptured files are identical to the previous baseline; they
  really are main's root route. This is a frontend baseline, not an API board.
- `target-desktop.png`: unchanged visual target from `design/02-kanban.png`.
- `after-desktop.png`, `after-mobile.png`: real authenticated `/` route on this
  branch, seeded through the API and captured by `cargo xtask board-check` with
  Canceled hidden. The dataset includes every status; the checkbox reveals the
  sixth column and persists in `?canceled=true`. Cards have API data, not fixture
  comments; comment counts render when supplied by the API.

Five 240px columns, four 16px gaps and 40px outer padding require 1304px. At a
1440px viewport the sidebar contracts to 136px to preserve those requested
minimums. At larger widths the sidebar returns to 240px. The four-column target
has wider cards; the after image includes In review as the fifth default status.
