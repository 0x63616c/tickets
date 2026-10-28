# Tickets design system

This is the visual and interaction reference for #7 (board), #8 (list), #9
(ticket editing), #10 (projects), and #16 (live updates). Framework/styling choices
are recorded in [ADR 0001](adr/0001-frontend.md).

## Reference routes

| Route | Reference |
| --- | --- |
| `/` and `/design/board` | Four-column Kanban; status menu and drag alternatives; counts, labels, blockers |
| `/design/list` | 36px rows grouped by status; aligned ID, label, comment, project, and date fields |
| `/design/ticket` | TKT-7; sanitized Markdown, task list, comments, properties, both dependency directions |
| `/design/ticket?id=6` | A specific fixture ticket; all ticket links retain their ID |
| `/design/empty` | Empty project and direct creation actions |
| `/design/projects` | Project list and New project flow |
| `/design` | Buttons, inputs, selects, status/priority icons, labels, avatars, connection states, overlays, tokens |

These are local in-memory fixtures. Creation, status changes, labels, comments,
projects, and dependency selection demonstrate behavior; reload resets them.
The sidebar identifies the preview as Sample data. Ticket API integration, GitHub sync,
and live transport remain separate. The existing `/login` and `/settings/tokens`
screens retain their API behavior and now use these design tokens. Do not treat the fixture statuses
as a statement about the GitHub issues' actual progress.

## Component contract

| Module | Components and responsibility |
| --- | --- |
| `components/shell.rs` | AppShell, TopBar, ProjectHeader; sidebar, breadcrumbs, visible Board/List tabs |
| `components/controls.rs` | Button, IconButton, TextInput, Select, Menu, LabelChip, Avatar, Shortcut, EmptyState, Markdown, ConnectionIndicator |
| `components/icons.rs` | One 16px/1.5px-stroke SVG family; status shapes and priority bars |
| `components/overlays.rs` | Native Modal, command palette, creation/dependency dialogs, toast |
| `style/tokens.css` | Color/type/spacing/radius/shadow/motion roles; self-hosted Inter Variable and Geist Mono |
| `crates/core/src/markdown.rs` | The shared Markdown-to-HTML boundary; pulldown-cmark then ammonia |

Use the existing role tokens; do not introduce a new palette per page. Use
`--text-secondary` for body content and `--text-muted` for secondary metadata.
`--text-faint` is reserved for nonessential section cues and separators. Do not
use color as the only status signal. Board cards and list rows share the same
status names and icons. No animated page entrance; hover uses short color changes.

A sidebar item is 28px high, a top/view bar 44px, a list/group row 36px. Body UI
is 13px Inter at 510 for rows; detail titles are 20px/590. Controls use 6px radii,
cards/menus 8px, dialogs 12px. Desktop sidebar width is 240px. Smaller screens hide
optional list properties first; boards scroll horizontally inside the content
area. The sidebar collapses behind a labelled toggle. Detail properties follow
the article on narrow screens. Focus remains visible and reduced motion is honored.

All spacing sits on a 4px grid: buttons and fields are 32px high with 12px
horizontal padding, keyboard hints 20px, label chips 24px, swatches 32px.

**Selects** keep the native element for keyboard, touch and assistive technology,
with `appearance: none` and the `--chevron-down` token: a 14px chevron in
`#8a8f98`, 12px from the right edge to match the 12px left padding, and 36px of
right padding so text never runs under it. Inline selects (board cards, detail
properties) use the same chevron with a transparent fill and a hover tint; card
selects size to their content so the chevron sits beside the value.

**Sidebar** is `#0f1011` against the `#08090a` canvas with a 1px `#23252a`
divider. A workspace switcher (20px mark, name, chevron, avatar) sits on top.
Items are `#d0d6e0` with 16px icons; hover is a faint tint; the active item is a
`#191a1b` pill with white text and an accent icon. Section headers are 11px
uppercase `#62666d`. Projects show an 8px colored dot.

## Flows to preserve

1. **New project:** visible in the sidebar and on Projects; one required name.
   The new project opens with a direct New ticket action.
2. **New ticket:** always visible at top right; C also opens the composer.
   Title is required, description optional, project inherited from the view,
   status defaults to Todo (or the column where New ticket was clicked).
3. **Move:** drag a card to a status column or use its status select. The latter
   supports keyboard/touch. Counts update and the toast names the ticket and
   new status. Restore keyboard focus after the card moves.
4. **Labels:** Add label in the properties rail opens neutral rows with colored
   dots and selected checkmarks. Selecting again removes the additional label.
5. **Dependencies:** Add dependency opens a direction select, an explanation,
   and search by ID/title. “Is blocked by” means the selected ticket comes first;
   “Blocks” reverses the relation. Existing and circular links are disabled in
   the preview; the API must enforce the invariant independently. Each relation
   links to the related ticket. Resolved prerequisites remain visible.
6. **Comments:** the same sanitized Markdown renderer as descriptions; submit
   is disabled for whitespace-only text. The timeline distinguishes You/Agent.
7. **Command palette:** Cmd/Ctrl+K, initial search focus, Arrow Up/Down or Tab,
   Enter activation, Escape close. C does not intercept typing in fields.
8. **Connection:** connected / reconnecting / offline are text + different SVG
   icons in a polite live region. Keep the last received content visible while
   reconnecting. #16 supplies transport state and retry timing. These indicators
   do not imply successful writes, queued mutations, or offline persistence.

The preview uses the shared core status/priority enums, Markdown renderer, and
`DependencyGraph` validator. In review and Canceled appear as board/list groups
when populated; all six statuses are available in the status select and gallery.

## Handoff boundaries

Replace `DesignState` with API-backed resources. Keep the visual structure and
native semantics; do not copy the fixtures as persistence/domain code. Add actual
loading/error/retry states at the resource boundary, optimistic rollback for
moves, server validation, session-aware ticket access, GitHub project linking, and live transport
in their implementation issues. The source URL shown on the project is a repository
reference, not an implemented GitHub Projects integration. The native select is a
conscious accessibility baseline, not a promise of identical browser popover chrome.

## Reproduce and review

Run `cargo xtask dev` for release assets through the local Worker, then:

```sh
TICKETS_DESIGN_URL=http://127.0.0.1:8787 cargo xtask design-check
```

The Rust xtask drives Chromium over its debugging protocol, checks real rendered
routes and interactions, and captures PNGs under `docs/screenshots/design/`.
`CHROME_BIN` can point to an installed Chromium/headless-shell executable if the
system Chrome has managed proxy settings. `TICKETS_SCREENSHOT_DIR` can select a
review output directory. Only local URLs are accepted. No browser dependencies
are included in the WASM app and no Node/Python tooling is vendored.

Default captures are 1440×900, dark, DPR 1; the mobile gallery is 390×844.
Fonts are loaded before capture. The before image is the actual #2 scaffold
(`screenshots/02-after-dark.png`), copied as `00-before-scaffold.png` here.

| Screenshot | Review focus |
| --- | --- |
| [Shell/gallery](screenshots/design/01-sidebar-shell.png) | Token consistency and component vocabulary |
| [Board](screenshots/design/02-kanban.png) | Card density, four status columns, visible move/create controls |
| [List](screenshots/design/03-list.png) | Aligned properties, compact groups, readable titles |
| [Detail](screenshots/design/04-ticket-detail.png) | Markdown, both dependency directions, comments and composer |
| [Empty](screenshots/design/05-empty-states.png) | Direct first action, no illustration or marketing copy |
| [Projects](screenshots/design/06-projects.png) | Simple rows, explicit New project |
| [Commands](screenshots/design/07-command-palette.png) | Search focus and keyboard hints |
| [Composer](screenshots/design/08-new-ticket.png) | Clear labels, inherited project, one submit action |
| [Dependency picker](screenshots/design/09-add-dependency.png) | Direction and search are explicit |
| [Mobile](screenshots/design/10-mobile-gallery.png) | Narrow-screen controls and typography |
| [Login](screenshots/design/12-login.png) | Existing authentication form using shared tokens |
| [Token access](screenshots/design/13-token-access.png) | Existing API-token page, signed out; no credentials in captures |

The screenshots were reviewed against the supplied Linear and Vercel pack. The
second pass removed the tall project introduction, tightened cards/rows, fixed
multiple selected sidebar items, removed redundant single-user avatars from rows,
shortened the fixture description to keep the composer visible, and changed the
empty project to a left-aligned action with visible status columns.

## Release measurement

Measured locally with the pinned toolchain, `cargo xtask build` (Trunk release,
LTO, size optimization, wasm-opt), including the existing authentication screens:

| Asset | Bytes |
| --- | ---: |
| Frontend WASM | 1,749,566 |
| WASM compressed with `gzip -9` | 689,087 |
| Generated JavaScript loader | 53,150 |
| Component CSS + tokens | 33,738 |
| Inter Variable WOFF2 | 352,240 |
| Geist Mono WOFF2 | 71,596 |

Gzip is a reproducible local estimate, not a measured Cloudflare transfer size.
This is the complete preview including Markdown parsing/sanitization and routing.
Track this baseline as the API-backed screens replace fixtures; do not compare it
to unoptimized debug builds.
