# TIK-75 — exclusive Board/List initial loading

Baseline: `b8e85d1` (main before this fix). Actual API-backed routes, isolated
local Wrangler Worker and D1, fixture-only projects/tickets seeded by board-check.
No production data or API credentials appear in captures.

## Reproduction and cause

Home loads project metadata for the sidebar but deliberately does not load the
board's ticket collection. Navigating from Home starts that first ticket fetch
with a project already selected. Board rendered its skeleton while independently
rendering columns whenever the project was nonzero. That made a second, empty
board appear below the skeleton. List independently rendered its filters and
list container alongside its skeleton too. Its real implementation lives in
`pages/list.rs`; `pages/tickets.rs` contains design fixtures.

Cold Board with no project selected did not overlap. Neither repeat refresh nor
focus-triggered live catch-up reintroduced a skeleton: `BoardState.loaded`
already keeps previous content visible. Retain that behavior.

## Capture method

Run `CHROME_BIN=/workspace/tickets-polish/chrome-noproxy TICKETS_SKIP_BUILD=1
cargo xtask board-loading-check` after building. Before mode additionally sets
`TICKETS_BEFORE_ASSETS` to the preserved baseline distribution.

Each path samples DOM visibility every 50ms for about four seconds and captures
PNG frames with a 50ms target cadence (actual screenshot throughput varies).
Ticket responses are held for the first 1.8 seconds, then released; the slow
case also applies 150ms CDP network latency. The check uses real navigation,
fetches and live focus events, without setting app signals. Cold hard navigation
uses a fresh app state, retained login cookie and disabled browser cache; capture
starts when the initial ticket request is held, so it focuses on API loading
rather than measuring Wasm download time.

`*-held.png` and `*-settled.png` show the endpoints. `*-frames.png` contact sheets
show eight frames across each sequence. The complete PNG sequences are retained
under ignored `target/TIK-75/` and `/workspace/tickets-polish/TIK-75/frames/`.
`*.json` contains the 50ms visibility/geometry samples; `transitions.txt` contains
per-path overlap counts. Visibility checks target `.loading-skeleton`, avoiding
unrelated live/refresh status announcements. They detect columns below the
skeleton even when the duplicate content extends beyond the viewport.

Refresh assertions require the same board/list DOM root, card count and board
position throughout the hold/release cycle. After-mode also requires `aria-busy`
feedback during refresh. Empty-project and mobile Home navigation are included.

## Review

One Impeccable critique/polish pass: independent visual and detector assessments.
Baseline visual review identified contradictory loading/empty states and duplicate
height on mobile; the mechanical scan returned zero findings. Preserve the
existing dark tokens, typography, skeleton component and published animation
wrapper. The fix changes state composition, with a small shared refresh indicator.

## Result

Before: **227 / 892** overlapping visibility samples across 11 paths. After:
**0 / 905** across the identical paths, including desktop/mobile Home navigation,
empty project, cold/slow loading, foreground refresh and live catch-up.

Both Board refresh cases retained the same root, six visible cards and board
position (`y=132`) throughout. List catch-up retained its root and 101 fixture
rows. All three reported busy while pending and never restored a full skeleton.
The initial skeleton/content boundary does not remount during later refreshes.

Rendered after PNGs confirm skeleton-only pending states and the original dense
content when settled. No CSS or design-token values changed. The source/detector
assessment found no additional loading defect; the one polish pass is complete.
