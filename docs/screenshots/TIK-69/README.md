# TIK-69 · shared page container

Before build: `2e5cdd0b33d0e9a1fb9855d2ec193d50ed1bbde4` (unchanged assets copied
before editing). After build: the implementation in this PR.

All captures are dark, with local deterministic fixture data, loaded bundled
fonts and reduced motion. No production data or tokens are used.

For Home, list, ticket detail, Settings Account and Board, `before-<page>-<width>.png`
and `after-<page>-<width>.png` are matching 900px-high viewports. Widths are 1440,
1920, 2560, 1024 and 390. The corresponding `*-full.png` files capture the complete
document at the same width, including content below the fold. Full-document
captures expand viewport height to the document height. The visible compositor
path avoids Chromium dropping cached text during offscreen rasterization.

Run after building:

```sh
CHROME_BIN=/workspace/tickets-polish/chrome-noproxy cargo xtask page-layout-check
TICKETS_BEFORE_ASSETS=/absolute/path/to/base-dist CHROME_BIN=/workspace/tickets-polish/chrome-noproxy cargo xtask page-layout-check
```

The fixture server and browser use their own ephemeral loopback ports and shut
down on exit. `measurements.json` records the after run's container bounds.
Assertions cover equal margins in the pane beside the sidebar, the shared
1240px cap, stable horizontal bounds while content loads, the unchanged 240px
desktop sidebar, document overflow and Board's full-width canvas/shared gutters.
The same check covers API-backed Home/list/detail/Board using immutable HTTP
fixtures, plus Analytics, Projects, gallery, Mission Control, all Settings
sections, empty/not-found states and standalone login at 2560, 1024 and 390.
First-run acceptance separately exercises the UI against a fresh empty local D1.

`components/` contains the after Board topbar crop and Board captures from
`cargo xtask component-shot board-topbar` (dark, fixture-only, 2× DPR).
