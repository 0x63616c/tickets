# Board drag and drop (TIK-51)

The board keeps its Leptos state, keyboard controls and sequential, compensating
PATCH workflow. SortableJS owns pointer/touch sensors, hit testing, the insertion
placeholder, preview movement and nested autoscroll. It never owns saved order.

## Research and choice

Reviewed upstream documentation/source on 2026-10-31:

- [leptos_dnd 0.1.4](https://docs.rs/leptos_dnd/0.1.4/leptos_dnd/):
  draggable/droppable primitives. Its published manifest requires Leptos 0.7
  with nightly, whereas Tickets uses stable Leptos 0.8. Sorting, scrolling and
  keyboard behaviour would still need application work.
- [leptos_drag_reorder](https://docs.rs/leptos_drag_reorder): native HTML drag
  events and reactive panel ordering. Its source does not supply the fallback
  touch sensor and nested edge scrolling needed here.
- [Pragmatic drag and drop](https://atlassian.design/components/pragmatic-drag-and-drop/core-package/):
  framework independent, native HTML drag primitives with separate optional
  packages. Its [accessibility guidance](https://atlassian.design/components/pragmatic-drag-and-drop/accessibility-guidelines/)
  still requires an alternative interaction. A strong general toolkit, but more
  assembly for the requested touch sorting and existing keyboard workflow.
- [dnd-kit DOM](https://github.com/clauderic/dnd-kit/tree/main/packages/dom):
  now has a framework independent DOM layer as well as React adapters. Sensors,
  sortable primitives and plugins could fit, but introduce a broader manager /
  adapter integration than this board needs. The React adapter is unnecessary.
- [SortableJS](https://github.com/SortableJS/Sortable#options): purpose-built
  connected sortable lists; fallback pointer/touch dragging, insertion ghosts,
  transform animation, touch activation delay and a bundled
  [AutoScroll plugin](https://github.com/SortableJS/Sortable/tree/master/plugins/AutoScroll).
  Chosen for its narrow, mature API and standalone ESM distribution. Pin the npm
  dependency and load it locally through wasm-bindgen; no runtime CDN.

## Ownership

During a pointer gesture, SortableJS may temporarily reposition a card. Before
calling Rust on drop, the adapter restores the original DOM position, including
Leptos' list markers. Rust then updates the canonical state once. Hover movement
never emits network writes or rebuilds Leptos cards. Cleanup destroys sensors,
listeners and observers. Project switching waits until a move is dropped or
canceled, and in-flight live responses defer card replacement during a move.
Keyboard moves remain immediate.

A board-level capture listener suppresses trailing clicks, including clicks on
nested title links and replacement card nodes. A new pointer gesture clears the
latch. Touch uses a hold delay so ordinary scrolling and taps remain available.
Escape and interrupted gestures restore the original order without saving.

## TIK-49 contract

TIK-49 still needs Calum's decision on sort keys, scope and persistence. TIK-51
adds no sort setting. BoardState.manual_order is the shared policy boundary; TIK-49 must wire
its selected ordering into it and use the same policy for rendering columns.
Derive/reset this policy when changing projects; sort preferences belong to TIK-49.

Manual columns permit insertion and rank writes. Sorted columns reject same-column
reordering and accept cross-column status moves without changing manual ranks.
Their pointer target is the column, not an apparent manual insertion position.
The policy must govern pointer, keyboard and status-menu moves alike. Do not
silently switch a sorted column back to manual order when dropping a card.

## Pointer tracking and origin slot (TIK-92)

The fallback preview is appended to `body`. Disable its transform transition at
that scope: the ordinary card press transition otherwise interpolates Sortable's
matrix updates. Sortable reads the computed matrix and adds the latest pointer
delta, so interpolation loses accumulated travel rather than merely lagging.
The centred page layout, fallback tolerance and pointer offsets are unrelated.

The real card remains Sortable's destination indicator. A separate inert,
`aria-hidden`, grayscale copy retains the source slot until drop/cancel. Exclude
that copy from Sortable's draggable selector and remove duplicate IDs and motion
keys. Remove the copy before restoring the real card and calling Rust. A release
outside a board column cancels even after the pointer visited a valid column.

`board-check` drives real Chromium mouse/touch input at 1440, 2560 and 390 pixels,
asserts viewport-relative preview travel within 3px at each step, checks the
source copy's dimensions/dimming/inertness, and checks tracking after nested
vertical/horizontal autoscroll. It retains the existing keyboard, ordering,
click-latch, persistence, rollback and cancellation checks. Review frames and
geometry live in `docs/screenshots/TIK-92/`.

## Safari-safe re-land (TIK-92)

PR #120 reverted #113 and #118 after production Safari reported `SyntaxError`
and `LinkError: import install_board_drag must be callable`. The original module
parses and exports a function in current real WebKit. The evidence points to a
cached old `/snippets/*/src/components/dom.js` served with a newer wasm import
surface, rather than a demonstrated Safari parser defect. The wasm-bindgen
snippet directory identifier must **not** be assumed to hash module contents.

The re-land keeps the existing `dom.js` byte-for-byte identical to main, including
its popup/motion exports. Drag installation is a plain exported function
declaration in
`dom_drag_c97c58dae072cc25.js`. The basename includes the first 16 hex digits of
SHA-256 of the exact file bytes. Any edit to this adapter must change that digest
and update both Rust module imports and the source-digest regression test together.
The verification harness and a Worker unit test check the filename against the
contents and verify its public URL. The Worker permits only the
`dom_drag_<16 hex>.js` family inside a valid build namespace. Trunk can retain
retired versions in its wasm-bindgen cache and preload them even when the new glue
never imports them; permitting these versioned assets avoids redirect/SRI errors.
Malformed digests, suffixes and nested paths remain private. The versioned file also exports the drag-aware motion bridge. New wasm
imports motion there; popup imports keep using the unchanged legacy module. Its
old motion export remains available for older wasm bundles. Preserving the legacy
bytes also preserves Trunk modulepreload integrity: Chromium correctly rejects a
stale response if a new HTML integrity digest expects changed legacy contents.
A browser
with any previous `dom.js` cached cannot satisfy this new import from that old
URL: the new wasm glue requests a distinct basename. Existing dependencies keep
their pinned versions/import surfaces. This fix does not depend on a cache-header
change, a hard refresh, or Safari cache eviction.

Both bridge modules and SortableJS 1.15.6's complete ESM distribution pass an
ECMAScript 2020 syntax parse. They contain no logical assignment, class static
blocks, top-level await, import attributes, regex lookbehind or private-brand
checks. Optional chaining/nullish coalescing are supported by Safari 15. This is
a syntax floor check; current Playwright WebKit is runtime evidence, not a claim
that Safari 15 itself was run.

`docs/screenshots/TIK-92/reland/verify.mjs` runs the release app against isolated
local Wrangler/D1 state, logs in via the UI, and tests real mouse round trips in
WebKit and Chromium. It substitutes main `b9db5b0`'s old `dom.js` at the requested
snippet URL alongside the new wasm; the unchanged pinned ascii-reveal module is
also served as a retained previous module. New digest/Sortable URLs are fetched
normally because previous main had neither. The substitution remains enabled
through drag, saves and reloads. Every preview step allows at most 2px axis
error; the source copy must be grayscale, opacity .35, inert, aria-hidden and
fixed in its original slot. See that directory's README, logs, geometry and PNGs.
