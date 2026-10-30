# TIK-60 ticket change log

All captures use disposable local D1 fixtures or `/design/c/history`. No production
data, personal account details or API credentials appear in the PNGs.

- `before-*`: actual main assets at `3ab9a59`, before this feature, using the same
  saved fixture edits as the after run. Later unrelated main changes to the sidebar
  may be visible in the after page; use the Activity crops to compare
  this feature directly.
- `after-desktop*` / `after-mobile*`: initial ticket state with saved priority/status
  edits, creation, label and dependency history. Full variants include the entire
  document; viewport variants use 1440×900 and 390×900 CSS px.
- `before-activity.png` / `after-activity.png`: tight 2× Activity section crops.
- `after-history.png`: isolated shared component, dark, 2×, loaded Inter font.
- `verified-*` and `create-dialog.png`: intermediate acceptance fixtures after
  exercising edits, comments, labels, project changes and dependencies.

Run from the issue worktree after sourcing `.job.env`:

```sh
CHROME_BIN=/workspace/tickets-polish/chrome-noproxy \
  TICKETS_BEFORE_ASSETS=/workspace/tickets-polish/TIK-60-baseline/crates/web/dist \
  cargo xtask detail-check
CHROME_BIN=/workspace/tickets-polish/chrome-noproxy cargo xtask detail-check
CHROME_BIN=/workspace/tickets-polish/chrome-noproxy \
  cargo xtask component-shot history --output docs/screenshots/TIK-60
```

`detail-check` verifies authenticated UI edits and history fields, failed-save
absence, escaped raw description history, descending pagination without duplicates,
exact timestamp activation, and retention of loaded/expanded history after a save.
It also retains the existing comment, dependency, project and navigation checks.

One Impeccable critique/polish pass: independent design review scored 28/40 before
polish. Fixes preserve loaded history on live refresh, share continuously refreshed
comment/event times with keyboard/touch access to exact UTC, and enlarge disclosure
touch targets. The independent rendered check verified no desktop/mobile overflow,
long-text containment, native Enter/Space disclosure, normal text contrast above
6.2:1 and a visible focus indicator at 4.47:1. One CLI detector scan returned no
findings; Rust-source detector coverage is limited and was supplemented by CDP.
No human-visible browser overlay was available; the headless overlay ran cleanly.

Local verification uses the required shared Cargo target. Concurrent jobs exposed
shared executable and Trunk bindgen artifact collisions; a temporary unique web
binary name and compiler metadata flag isolated local build outputs. The binary
name is restored before committing. API/import/webhook/first-run checks then reused
the completed build with source and output hashes checked before each run; each
check still used fresh local D1 and real HTTP/browser flows. CI builds the normal
manifest in isolation. Capture-only runs support `TICKETS_DETAIL_CAPTURE_ONLY=1`
and use the supported reduced-motion preference for deterministic settled text;
full detail acceptance retains normal motion and its reveal assertions.
