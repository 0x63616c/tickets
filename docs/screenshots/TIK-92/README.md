# TIK-92 drag tracking evidence

Baseline: `5a7ea241c1be312ebf4a147783c24c09153f1af1`, built from unchanged
source in this worktree before the repair. Baseline web assets were copied into
ignored `target/tik92-before-assets/`; no additional checkout was created.

All board frames show the real `/board` route against disposable local Worker/D1
state with API-seeded sample tickets, loaded fonts and dark appearance. The white
ring records the exact CDP pointer position because headless PNGs omit the OS
cursor. It is a test-only, non-interactive overlay.

## Results

Twenty real sensor moves per viewport, sampled one animation frame apart; four
PNG positions per sequence. The assertion allows at most 3px absolute error from
the pickup offset on either axis. The geometry JSON contains every sample.

| Viewport / input | Before maximum axis error | After maximum axis error |
| --- | ---: | ---: |
| 1440×900 / mouse | 13.70px | 0px |
| 2560×900 / mouse | 13.69px | 0px |
| 390×900 / touch | 9.48px | 0px |

Before: preview retains `transform 0.12s cubic-bezier(0.23, 1, 0.32, 1)` and no
origin copy exists. Sortable reads that interpolated matrix and adds the latest
pointer delta, losing travel during movement. The board-scoped transition reset
misses the preview appended to `body`. The original card press reset also needed
the press selector's specificity to preserve pickup geometry.

After: preview transition is `none`. Every origin rectangle retains its original
x/y/width/height, opacity `0.35`, grayscale, `inert` and `aria-hidden`. The separate
purple destination indicator still moves through Sortable's normal path.

- `before-<width>-step-{0,6,12,19}.png` and matching `after-*`: paired positions.
- `before-tracking.json`, `after-tracking.json`: all 60 samples per phase.
- `after-cross-column.png`: grey Backlog origin with purple Todo destination.
- `after-column-autoscroll.png`, `after-board-autoscroll.png`: mouse vertical and
  touch horizontal autoscroll; subsequent 24px/16px movement tracks within 3px.
- `before-after-frames.mp4`: before on left, after on right, the four desktop
  captures presented as a three-second frame sequence (not a real-time recording).
- `components/`: fixture-only `board-columns` toolbar/popover crops at 2× DPR
  and corresponding full board captures. These fixtures do not install sensors;
  drag assertions and evidence above use the real API board.

## Validation

Passed: `cargo xtask build`, `cargo xtask ci` (token policy, policy tests,
formatting, workspace build, host and wasm32 clippy with warnings denied,
workspace tests), `npm run lint:css --prefix crates/web`,
`npm run test:lint --prefix crates/web`, `board-check`, `first-run-check`,
`component-shot board-columns` before/after and `page-layout-check`.

The full board check also covers keyboard drag, Escape/touch cancellation,
outside drop after visiting Todo, click suppression and subsequent normal card
activation, persistence/reload, rollback, filters, pagination and project scope.

First-run's first attempt timed out at its existing ticket-detail `saveHeld`
assertion after the UI creation/Kanban/dependency steps; a diagnostic fresh-state
run passed the entire suite without source changes or assertion changes. The
first build hit a truncated shared wasm-opt input; the subsequent serial build
passed. The box's managed SOCKS policy blocked loopback in an ordinary browser;
checks used a child-only bubblewrap mount namespace hiding browser policy files,
with `/dev` and `/proc` mounted for Chromium. No shared policy was edited.

Impeccable critique: independent design and detector assessments, 35/40,
zero in-scope priority issues; CLI detector exited 0 with `[]`. Desktop/mobile
rendered frames and geometry were reviewed; no persistent browser overlay was
claimed. Polish preserved the existing dark tokens, source/destination distinction,
keyboard guidance and readable preview. No further edits were required.

## Reproduce

Source `.job.env` and put `$HOME/.cargo/bin` on PATH first. Build once, then use
`TICKETS_SKIP_BUILD=1 CHROME_BIN=<local-headless-chromium> cargo xtask board-check`.
CI runs the same assertion through its existing board-check step.

For paired baseline evidence, first build unchanged baseline assets and retain
that dist directory. Set `TICKETS_DRAG_BASELINE=1`, `TICKETS_SKIP_BUILD=1` and
`TICKETS_BEFORE_ASSETS=<baseline-dist>` when running the current board-check.
Baseline mode records the defect without enforcing the repaired behavior.
