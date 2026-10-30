# TIK-62: cold-load scrollbar regression

Baseline: `b4c3e11df55d7a66e83a192fbfcc50fd017c1984` (PR #79, including
ascii-reveal from PR #77). Before assets were built from this worktree before
source changes and preserved under ignored `target/TIK-62-before-dist`.

The loading harness renders real routes with immutable local HTTP API fixtures
from `docs/fixtures/home-api.json` and `home.json`; it does not use production
accounts or data. The package's actual skeleton sweep and decode run unchanged.
Each route/size gets a fresh Chromium profile, cache disabled, 150ms network
latency, 200,000 bytes/s download, 75,000 bytes/s upload, and 4× CPU throttling.
The matrix is Board, List, `/t/TKT-1`, Projects and Home at 1280×800 and 390×844.
An init script samples document and main scroll/client extents on every animation
frame from navigation, through held API responses, release, decode and settlement.

## Reproduction

```sh
source .job.env
cargo xtask build
TICKETS_BEFORE_ASSETS="$PWD/target/TIK-62-before-dist" \
  CHROME_BIN=/workspace/tickets-polish/chrome-noproxy cargo xtask loading-check
CHROME_BIN=/workspace/tickets-polish/chrome-noproxy cargo xtask loading-check
```

The browser wrapper uses a child-only mount namespace to override the machine's
managed proxy policy for loopback. It does not change shared Chrome settings.
Shared Cargo outputs may be replaced by another worktree: touch `xtask/src/main.rs`
before rebuilding if Cargo reuses a stale binary, and preserve the resulting
executable under `target/` when running long checks alongside other jobs.

## Evidence

Four dark PNGs show the same held Home skeleton, after fonts load and before API
release. A fixed canvas annotates the document/client dimensions and draws red
edge markers for the overflow present at capture time; it contributes no layout
size. Headless Chromium normally hides the actual scrollbar track. Horizontal
escape can occur earlier than the captured frame; the JSON records those peaks.
The JSON includes sample/phase counts, settled and peak extents, violating frame
counts, and offending element rectangles. Rectangle scans run when extents grow,
not every frame, to avoid distorting the CPU-throttled timing.

The observed failures are skeleton-phase canvas escape, rather than a demonstrated
decode failure: default 300×150 absolutely positioned sweep canvases extend past
short/narrow hosts. Desktop detail reaches 1333px width. Home reaches 1324×974
on desktop and 511×1752 on mobile. Its placeholder also has a taller intrinsic
footprint than its eventual queues. Seven other route/size cases do not exceed
settled extents in the baseline. Legitimate long-page and board scrolling remain
allowed; regression assertions reject growth beyond settled extents, with 1px
rounding tolerance.

The fix clips and bounds skeleton hosts, gives Home's loading placeholder only
its available page box, and marks reveal hosts before controller creation so the
initial stylesheet can position/clip overlays before any lazy library CSS. No
package files, global page overflow rules or animation settings were changed.

## Results

| Route | Viewport | Before violating frames | After |
| --- | --- | ---: | ---: |
| `/board` | 1280×800 | 0 | 0 |
| `/list` | 1280×800 | 0 | 0 |
| `/t/TKT-1` | 1280×800 | 4 | 0 |
| `/projects` | 1280×800 | 0 | 0 |
| `/` | 1280×800 | 135 | 0 |
| `/board` | 390×844 | 0 | 0 |
| `/list` | 390×844 | 0 | 0 |
| `/t/TKT-1` | 390×844 | 0 | 0 |
| `/projects` | 390×844 | 0 | 0 |
| `/` | 390×844 | 13 | 0 |

Every after case sampled both skeleton and decode. Loaded Home remains 870px
high on desktop and 1672px on mobile, matching baseline. Its loading captures
are exactly 1280×800 and 390×844, with no synthetic overflow edge markers.

Passed: workspace formatting, host and wasm32 workspace Clippy with warnings
denied, workspace tests, `loading-check` (including its original sweep/decode,
search, busy-button, reduced-motion and disposal assertions), and isolated
`skeletons --variant detail` / `reveal` component shots. The final loading run
used a freshly built executable copied to ignored `target/TIK-62-current-check`
to avoid another worktree replacing the shared Cargo executable mid-check.
All six stylesheet URLs and SHA-384 integrity hashes were verified against the
published assets; Home/foundation CSS also matches its source byte for byte.

`first-run-check` was run against migrated, empty local D1, but stopped at
bootstrap login with “Incorrect username or password” before creating any domain
data. This acceptance check did not pass; no authentication behavior was changed
in this ticket. A prior overlapping Trunk run produced inconsistent staging;
those after captures were discarded, and the final evidence uses a serialized,
source-stable build with all stylesheets loaded.

## Impeccable critique and polish

Method: independent assessments A (`/root/critique_design`) and B
(`/root/critique_evidence`), followed by a scoped polish pass. Both inspected the
four final PNGs and geometry reports and found no remaining in-scope defects.
The existing dense dark shell, populated sweep, animation configuration and
reduced-motion behavior remain intact. The scoped loader heuristic score is
27/32 (recovery/help not applicable to this passive status surface).

The detector ran once with telemetry disabled: zero primary/advisory findings
on the Rust wrapper. Its regex scan cannot validate generated Leptos markup or
canvas timing; runtime evidence supplies that proof. No browser detector overlay,
hooks or review-only servers were created. No ignore list was present. Context
reported legacy PRODUCT.md metadata; it was left unchanged (`impeccable init`
can update it separately). Questions skipped: fully specified bug fix.
