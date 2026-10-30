# TIK-57 blocked indicators

Dark fixture-only captures from `cargo xtask card-shots`, fonts loaded.
Before assets: b4c3e11df55d7a66e83a192fbfcc50fd017c1984 (TIK-46 option B).
Local builds used distinct temporary binary names to avoid cross-worktree artifact
collisions in the shared Cargo target; UI source and styles match their stated revisions.
The base Wasm was packaged directly with wasm-bindgen 0.2.129 and the base
Trunk CSS/font stage to avoid its queued duplicate Cargo metadata call; no
application code was changed. The after assets use the normal Trunk build.
Both phases use the same five sample tickets, routes and viewports; no production data.
Card crops are 2×; desktop pages are 1440×900 and mobile pages 390×900.

- `before/after-blocked.png`: one unresolved dependency.
- `before/after-blocked-multiple.png`: two unresolved dependencies.
- `after-blocked-tooltip.png`: shared tooltip with both blocker keys and titles.
- `before/after-blocked-board-mobile.png`: mobile board scrolled to the blocked cards.
- `before/after-board-*`, `before/after-list-*`: full-page context at both sizes.
- Other card crops retain long title, many labels and urgent-priority coverage.

`board-check` exercises the real local API/live transport: normal status grouping,
single/multiple blocker labels and keyboard tooltip, partial resolution, live clearing
on done/canceled, list parity, and existing whole-card click/Enter/drag behavior.
`contrast-check` includes the rendered blocked marker in its foreground/background
measurements; `--blocked` is the shared red-orange semantic token.
