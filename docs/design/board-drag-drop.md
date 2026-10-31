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
