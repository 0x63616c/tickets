# TIK-51 board drag evidence

Dark-only, loaded-font captures from the actual `/board` route against isolated
local Worker/D1 fixtures. No production data or production writes.

Baseline: `04fa46d183bb6a58f5680250ded418e5f7101f76` (see `base-sha.txt`). Built
in this existing job worktree before editing; assets retained under ignored
`target/tik51-before-assets`. No separate clone or Cargo target directory.

- `before-board-{desktop,mobile}.png`: unchanged baseline, same initial seeded board.
- `after-board-{desktop,mobile}.png`: same seed and 1440×900 / 390×900 viewports.
- `after-pointer-placeholder.png`, `after-touch-placeholder.png`: real Chromium
  mouse/touch gestures; the accent slot shows insertion and the raised card is
  the moving preview. These are actual API board cards, not design fixtures.
- `after-column-autoscroll.png`: scrolling a long column while dragging.
- `after-board-autoscroll.png`: touch dragging at the narrow board's right edge.
- `components/`: fixture-only `/design/board` chrome/component captures (2× DPR
  tight topbar crops plus full desktop/mobile pages), kept separate from API proof.

Reproduce with `.job.env` sourced and single-job Cargo builds:

```sh
source .job.env
CARGO_BUILD_JOBS=1 cargo xtask build
CHROME_BIN=<headless-chromium> TICKETS_SKIP_BUILD=1 cargo xtask board-check
CHROME_BIN=<headless-chromium> cargo xtask component-shot board-topbar --output docs/screenshots/TIK-51/components
CHROME_BIN=<headless-chromium> TICKETS_SKIP_BUILD=1 cargo xtask first-run-check
```

For the baseline initial capture, supply `TICKETS_BEFORE_ASSETS=<baseline-dist>`
to board-check. That mode captures the previous actual board and skips the new
sensor assertions. The full existing baseline board-check also passed before
editing. The component topbar's historical `--before-assets` mode expects the
pre-TIK-59 New ticket button; use the API baseline above for this ticket.

On this box, the managed browser proxy blocked loopback. Tests used a wrapper
that bind-mounted an empty policy directory only inside the browser child's
private mount namespace. Shared/browser policy files were never edited.
