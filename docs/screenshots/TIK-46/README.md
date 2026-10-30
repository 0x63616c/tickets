# TIK-46 compact Dots cards

Fixture-only evidence; no production data or tokens. Baseline: 8bc5d99bacba936de87d2531c49993decfc4ec3f.

`cargo xtask card-shots` serves four API-shaped sample tickets through the local
fixture server. Before uses that base commit's built assets via
`TICKETS_BEFORE_ASSETS`; after uses this branch's assets. Both runs use identical
sample data, routes, dark theme, loaded bundled fonts and viewports.

- `before/after-{long-title,many-labels,urgent,blocked}.png`: tightly cropped 2× cards.
- `before/after-board-desktop.png`: 1440×900 full board.
- `before/after-board-mobile.png`: 390×900 board viewport (columns scroll within board).
- `before/after-list-{desktop,mobile}.png`: unchanged list layout, whole-row interaction.

The long-title sample also has many labels, including a hidden priority label.
The blocked sample depends on the urgent sample. Full titles and full visible
label names remain available in native hover titles. The option-2.png mock and
function B in the supplied gen.py are authoritative; options-sheet.png currently
shows a different set of variants and is not used to override the chosen mock.
