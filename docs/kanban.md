# API-backed Kanban (#7)

The real board is `/board`. Pick a project in the sidebar. Five statuses are visible by default, including empty In review; only Canceled is hidden. The Columns checkbox menu remembers visible statuses per project in localStorage (`tickets.board.columns.project.<id>`), with at least one column always visible.
Columns share space equally with a 240px minimum and 16px gaps. The sidebar
contracts at 1440px to fit all five; narrower boards scroll horizontally. `/design/board`
remains the in-memory visual reference.

Cards use the shared status/priority icons and label chips; they show the API key,
title, assignee initials when assigned, optional API comment counts, and a Blocked by key indicator when a prerequisite is
neither Done nor Canceled. The board loads every API page, including prerequisites
in other projects, before calculating that indicator.

Drag onto a card to insert before it, or onto a column to append. Tab focuses a
card, arrows navigate, Space picks it up, arrows change its column/position,
Space saves, and Escape cancels. The native status select is an alternative for
keyboard and touch. Focus returns to the moved card. Label, assignee, and text
filters combine; reordering preserves cards hidden by the filters.

New ticket is available in the header and each column, and with C outside input
fields. The composer inherits project/status; title is required, description is
optional. Creation and moves use the authenticated REST API. Loading, sign-in,
request failure and reload states are explicit. Writes update the board
optimistically; failures restore its snapshot and compensate completed writes.
Ordering uses gapped ranks, documented in [CONTRIBUTING](../CONTRIBUTING.md#kanban-ordering-and-verification).

`pages::board::BoardState::apply_change_event(ticket, deleted)` is the #16 hook
for an authoritative ticket upsert/delete. It returns false during an active
move, so transport can defer the event and refetch when editing settles. It does
not establish a connection or claim live synchronization.

## Verification

Run `CHROME_BIN=<headless-chromium> cargo xtask board-check`. The Rust CDP harness
builds release assets, starts local `wrangler dev`, applies tracked migrations
to isolated D1 under `target/`, signs in through the actual UI, and seeds via API.
It checks:

- Five default columns, empty In review, and per-project column choices persisted across reload, real metadata, active blockers, project isolation and empty
  projects, including a dataset crossing the API's 100-ticket page boundary.
- Native mouse drag between/within columns; keyboard move/reorder, cancellation,
  persisted ordering after reload, and restored focus.
- Combined label/assignee filters and the native status select.
- Rejected writes and compensation after a partially completed reorder.
- Ticket creation and signed-out loading errors.

Dark screenshots are in [screenshots/kanban](screenshots/kanban): before is the
actual root route independently rebuilt on main at `d446c24` in a temporary
worktree (main itself maps `/board` to the fixture Board; no `/design` URL was used); after is the API-backed route on this
branch, at the same 1440×900 and 390×900 viewports with fonts loaded. The after
dataset is deliberately smaller and includes the additional status columns.

## Clarity check

This view's part is visible without instructions: choose a project, click New
ticket, enter a title, then drag or choose a status on the card. Labels and
assignees have named filters; blockers use ticket keys. Keyboard
move instructions appear in the desktop footer and pickup announcement.
Project creation (#10), editing labels/dependencies and opening ticket details
(#9), and the list view (#8) remain their own implementation issues. The real
board does not send users into fixture dialogs to perform those operations.
