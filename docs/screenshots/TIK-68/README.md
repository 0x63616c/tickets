# TIK-68 project row actions

Dark fixture data only. Baseline: `07343d7f7ff35e051d3d312e55b8f737c0619f71`.

- `before-project-menu.png` / `after-project-menu.png`: hovered project row in
  the isolated sidebar; the after image includes the shared menu. There was no
  project-row menu in the baseline.
- `before-project-edit.png` / `after-project-edit.png`: existing shared editor,
  reached through Projects before and directly through the sidebar after.
  The baseline Projects page uses the component server's sample project; the
  after editor uses the sidebar sample project with its glyph and linked repo.
- `before-sidebar.png` / `after-sidebar.png`: resting sidebar at 2×.
- `before-desktop.png` / `after-desktop.png`, `before-mobile.png` /
  `after-mobile.png`: full shell fixtures at desktop and mobile widths.

Capture with `CHROME_BIN=/workspace/tickets-polish/chrome-noproxy cargo xtask
component-shot sidebar --output docs/screenshots/TIK-68`. Supply
`--before-assets <baseline-dist>` for the baseline. Menu/edit captures use 2×
component crops; full-page captures use the browser check's standard viewport.

Local concurrent Trunk builds produced an empty WASM distribution. The baseline
capture used the matching release WASM and wasm-bindgen glue in a private asset
snapshot; no production resources or fixture values were changed.
