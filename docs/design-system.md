# Tickets design system

This is the visual and interaction reference for #7 (board), #8 (list), #9
(ticket editing), #10 (projects), and #16 (live updates). Framework/styling choices
are recorded in [ADR 0001](adr/0001-frontend.md).

## Reference routes

| Route | Reference |
| --- | --- |
| `/design/home` | Current work, eight UTC weeks, review/blocked queues and chart data tables |
| `/design/board` | Four-column Kanban; status menu and drag alternatives; counts, labels, blockers |
| `/design/list` | 36px rows grouped by status; aligned ID, label, comment, project, and date fields |
| `/design/ticket` | TKT-7; sanitized Markdown, task list, comments, properties, both dependency directions |
| `/design/ticket?id=6` | A specific fixture ticket; all ticket links retain their ID |
| `/design/empty` | Empty project and direct creation actions |
| `/design/projects` | Project list and New project flow |
| `/design` | Buttons, inputs, selects, status/priority icons, labels, avatars, connection states, overlays, tokens |

Home is available at `/` and `/home`. The real API-backed board is `/board`;
see [Kanban](kanban.md) for its persistence,
keyboard controls, local verification and #16 change-event hook.
The API-backed list is `/list`: it reuses the grouped fixture rows and inline
status/priority icons, with project/label/assignee/status filters and sort in the
URL query. Enter opens the persistent detail editor at `/ticket?id=<numeric ID>`.
Run `cargo xtask list-check` with `CHROME_BIN` for the isolated local verification;
see the list conventions in `CONTRIBUTING.md`.

The API-backed detail uses the same article, properties rail, dependency rows,
Markdown, comments and modal controls as the fixture. Edit ticket saves title and
description; property selects save immediately. The shared create dialog supports
project selection. Descriptions and comments provide Write/Preview using the core
sanitizer. Dependency rejections remain inline in the open picker. Run
`cargo xtask detail-check` with `CHROME_BIN` for isolated local verification and
desktop/mobile screenshots; see the detail conventions in `CONTRIBUTING.md`.

The `/design*` routes are local in-memory fixtures. Creation, status changes, labels, comments,
projects, and dependency selection demonstrate behavior; reload resets them.
The sidebar identifies the preview as Sample data. Ticket API integration, GitHub sync,
and live transport remain separate. The existing `/login` and `/settings/api-tokens`
screens retain their API behavior and now use these design tokens. Do not treat the fixture statuses
as a statement about the GitHub issues' actual progress.

## Component contract

| Module | Components and responsibility |
| --- | --- |
| `components/shell.rs` | AppShell, TopBar, ProjectHeader; sidebar, breadcrumbs, visible Board/List tabs |
| `components/controls.rs` | Button, IconButton, TextInput, LabelChip, Avatar, Shortcut, EmptyState, Markdown, ConnectionIndicator |
| `components/icons.rs` | One 16px/1.5px-stroke SVG family; status shapes and priority bars |
| `components/overlays.rs` | Native Modal, command palette, creation/dependency dialogs, toast |
| `style/tokens.css` | Color/type/spacing/radius/shadow/motion roles; self-hosted Inter Variable and Geist Mono |
| `crates/core/src/markdown.rs` | The shared Markdown-to-HTML boundary; pulldown-cmark then ammonia |

Use the existing role tokens; do not introduce a new palette per page. Use
`--text-secondary` for body content and `--text-muted` for secondary metadata.
`--text-faint` is reserved for nonessential section cues and separators. Do not
use color as the only status signal. Board cards and list rows share the same
status names and icons. Route snapshots crossfade briefly; hover uses short color changes. See the TIK-38 interaction contract below.

A sidebar item is 28px high, a top/view bar 44px, a list/group row 36px. Body UI
is 13px Inter at 510 for rows; detail titles are 20px/590. Controls use 6px radii,
cards/menus 8px, dialogs 12px. Desktop sidebar width is 240px. Smaller screens hide
optional list properties first; boards scroll horizontally inside the content
area. The sidebar collapses behind a labelled toggle. Detail properties follow
the article on narrow screens. Focus remains visible and reduced motion is honored.

All spacing sits on a 4px grid: buttons and fields are 32px high with 12px
horizontal padding, keyboard hints 20px, label chips 24px, swatches 32px.

The #15 audit uses the same rules on real API routes. Board columns retain a
240px minimum; In review appears when populated. Shared status selects offer all
six statuses without shifting columns on pickup. Extra columns scroll without
shrinking the 240px sidebar. List keys
occupy a fixed 56px slot at both viewport sizes; titles truncate and expose their
full text on hover. Mobile filters wrap into two readable columns.

Authentication uses the shared `.field` and `.button` rules. Token settings use
an inventory with active/revoked filtering, compact metadata rows, and the shared
Modal for creation and named revocation. The one-time secret is a readonly
selectable field with Copy, an explicit saved acknowledgement, and deliberate
discard confirmation. Full timestamps remain in `time` attributes and accessible
labels/tooltips; visible dates use UTC calendar days. Blank assignees use an empty Avatar instead
of placeholder property text. Comment editors keep their accessible label and
Write/Preview controls while fitting the desktop article's compact rhythm.

Command rows are 36px and scroll within the dialog; search and footer stay
visible. Results open the ticket, no matches have an explicit message, Escape
restores the opener, and shortcuts never stack another dialog over a composer.
The mobile sidebar has an expanded state, outside dismissal, Tab containment,
and Escape focus restoration. These changes use only existing tokens.

**Selects** use the shared `SelectField` controlled listbox (or the small static
`Select` adapter). The trigger is 32px with a 14px chevron, 12px horizontal
padding, a visible focus ring, and ellipsis for long values. The popup enters
from its trigger, flips above it when necessary and scrolls within the viewport.
Arrow keys, Home/End, typeahead, Enter/Space, Escape and Tab work in all instances.
Selection returns focus to the trigger. Failed writes keep the saved value.

**Sidebar** is `#030303` against the pure `#000000` canvas with a 1px `#141416`
divider. A shared account dropdown (product glyph, Tickets, chevron) sits on top; the
menu shows the saved display name or authenticated username. No placeholder avatar
or workspace footer. Home and search precede collapsible Projects and Views.
Items are `#d0d6e0` with 16px icons; hover is a faint tint; the active item is a
`#0a0a0b` pill with white text and an accent icon. Section headers are 11px
uppercase `#797d86`. Projects use the shared 20px ProjectIcon tile.

**Palette** is near-pure black (#32): canvas `#000000`, sidebar and properties
`#030303`, surfaces `#070708` (cards, inputs) and `#0a0a0b` (raised, active),
borders `#141416` and `#1f2024`. Depth comes from borders more than fills. All
body text meets WCAG AA on every surface; the faint tier `#797d86` keeps 4.8:1
even on `#0a0a0b`.

**Tinted chips.** A label chip takes its own hue (`--c`): fill at 14%
(`--tint-fill`), border at 26% (`--tint-border`), text mixed 55% toward white
(`--tint-text`), dot at full colour. Board column counts use the same recipe
with their status colour. Status and priority stay icon-first elsewhere, so a
row never carries more than one tinted element.

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
semantic controls; do not copy the fixtures as persistence/domain code. Add actual
loading/error/retry states at the resource boundary, optimistic rollback for
moves, server validation, session-aware ticket access, GitHub project linking, and live transport
in their implementation issues. The source URL shown on the project is a repository
reference, not an implemented GitHub Projects integration. Select behavior is provided by the shared controlled listbox, not browser menu chrome.

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

The subsequent real-route audit is in [screenshots/15/README.md](screenshots/15/README.md),
with paired 1440×900 and 390×900 captures from `origin/main` and the #15 branch.
Run `cargo xtask polish-check` with `CHROME_BIN` to reproduce it; see
`CONTRIBUTING.md` for baseline assets and local state conventions. Projects is
explicitly captured as a design preview until #10; no project API flow or live
connection state is fabricated by this polish pass.

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

The API-backed Projects view is `/projects`; `/design/projects` remains the
fixture. It reuses the fixture table, shared project icon, Modal and TextInput.
New project and Edit save name, key prefix, glyph or emoji, icon color and optional GitHub link.
Show archived reveals projects for restoration; archiving preserves tickets.
GitHub Project and repository links appear in the selected project header.
Run `cargo xtask projects-check` for the isolated local UI lifecycle check.

## TIK-38 interaction foundations

These components are the extension points for TIK-39/40/42/44/45. Keep page layout
work in those tickets; reuse these APIs rather than copying popup or timer logic.
All are exported from `crate::components`.

| API | Contract |
| --- | --- |
| `SelectField { label, options: Signal<Vec<SelectOption>>, value: Signal<String>, on_change: Callback<String>, disabled }` | Controlled single selection. `SelectOption::new(value, label)`; `SelectOption::project(project)` includes its icon; `::status(status)` and `::priority(priority)` include their semantic icons. Callers own async saving and rollback. |
| `Select { label, options: Vec<&str>, value: RwSignal<String>, on_change? }` | Small adapter for static choices and fixture/local state. |
| `DropdownMenu { label, children, trigger_content? }` | Roving focus, typeahead, Escape/Tab/outside dismissal and origin-aware positioning. An optional trigger content slot supports the account avatar/name while retaining the labelled button. Compose `MenuItem { on_select, checked?: Signal<bool>, shortcut? }` and `MenuSeparator`. Optional submenus are not implemented. |
| `Combobox { label, input_id?, options, selected: Signal<Vec<String>>, on_toggle, on_create?, busy }` | Searchable multi-select. Search stays editable; arrows enter options. Enter toggles an exact match in selection-only mode. With `on_create`, Enter submits the trimmed name to the caller’s create-or-attach flow (an already attached label stays attached); option selection always toggles. Opening focuses search; Arrow Up from the first option returns there. Selection is authoritative and async errors remain on the owning page. |
| `Tooltip { text, children }` | 500ms initial pointer delay, immediate keyboard display, 700ms skip-delay window after a tooltip. Escape/blur/leave dismiss. Never use as the only accessible label. |
| `SegmentedControl { label, options, value: RwSignal<String>, instant?: Signal<bool> }` | Equal-width mutually exclusive buttons with `aria-pressed`, roving tabindex, Left/Right/Home/End and a transform-only sliding indicator. Keyboard changes skip motion; `instant` also suppresses movement for external shortcuts. Markdown Write/Preview uses it. |
| `LoadingIndicator { label }` | Compact library scan skeleton; one polite status label and decorative glyphs hidden from AT. |
| `BusyButton { busy, disabled, variant, kind, class?, aria_label?, status_label?, on_click?, children }` | Async submit default; disables repeat input and sets aria-busy. The original content keeps its grid footprint; the library skeleton overlays it; the label decodes after completion. Keep children constant while busy. |
| `AsciiSkeleton { variant, lines, rows?, class? }`, `LoadingSkeleton { layout }` | Package-generated markup and sweep, with board/list/detail/projects layout composition. First loads reserve space; reconciliation keeps saved content visible. |
| `AsciiReveal { children, class? }`, `use_ascii_reveal(root, pending, on_mount)` | Package decode preset, fast duration, 1.25× speed, top-left origin, no lazy delay. Mount or pending-to-ready reveals; destroys controllers on rerun/unmount. |
| `ProjectIcon { name, icon, color, size }` | Decorative SVG/emoji or initial fallback next to the project name. `size="large"` is 32px; default 20px. Never display unvalidated markup. |
| `ProjectIconPicker { value, color, name }` | Shared glyph selector, live tile preview and optional emoji input for the existing editor. |

### Presence and movement

`Presence { show: Signal<bool>, children }` keeps a child owner mounted during
its exit and provides the open state to Modal. Use it instead of an immediate
`Show` around a boolean-driven overlay. For optional payloads, use
`present_value(source: Signal<Option<T>>)` to retain the last payload, and pass
the original source's open state to `Modal { open }`. `use_presence(open, ms)`
is the low-level helper for anchored surfaces. All pending timeouts are canceled
on cleanup and rapid reopen. The 140ms retention includes the 120ms exit and a
frame margin. Native dialog continues to contain focus and keep the background
inert throughout exit; cleanup restores the connected opener.

`dom.js` is a small platform bridge, not a parallel state system. Popover API
puts anchored surfaces in the top layer; fixed positioning is the fallback.
Resize and nested scroll reposition from the trigger. No positioning library
is bundled. Motion snapshots keyed by `data-motion-key` provide transform-only
FLIP for card/row reorders and short insertion/removal fades. Removal snapshots
are inert, aria-hidden, stripped of IDs, noninteractive and removed after 120ms.
Keyboard moves bypass animation. Route clicks progressively use native View
Transitions to crossfade old/new snapshots; unsupported browsers navigate at once.
The update callback yields a task, never an animation frame: the [View Transitions
spec](https://www.w3.org/TR/css-view-transitions-1/#setup-view-transition) suppresses
rendering during that callback. Snapshot overlays do not intercept input; skipped
snapshots do not reject the navigation. Isolated preview links use a document
navigation so they mount without the application shell.

Loading is **ascii-reveal by Quan Hoang, MIT License**, installed from the exact
[npm 0.3.0 package](https://www.npmjs.com/package/ascii-reveal/v/0.3.0).
[Source](https://github.com/quanhoangindex/ascii-reveal) · [Demo](https://quanhoangindex.github.io/ascii-reveal/).
The framework-agnostic core is imported directly by wasm-bindgen; there is no
React runtime, vendored implementation or custom glyph animation. Full notice:
[THIRD_PARTY.md](../THIRD_PARTY.md). Library colors/fonts reference Tickets tokens.
OS reduced motion and the in-app override stop sweeps and skip decode entirely,
including mid-animation. Controllers and skeleton stop functions are disposed on
cleanup. Real data never waits on a fixture timer. CSS transitions still make the
segmented indicator interruptible.

### Component review and PNGs

`/design/c/<name>?variant=...` is isolated from auth and API transport, with sample
data only. `/design` links every specimen. Names: `select`, `menu`, `combobox`,
`modal`, `loader`, `skeletons`, `reveal`, `icons`, `busy-button`, `tabs`, `tooltip`.
Skeleton layouts: `board` (default), `list`, `detail`, `projects`. `reveal` holds
a sample response for 1.2 seconds, then decodes it; Replay loading repeats the cycle.

```sh
cargo xtask build
CHROME_BIN=/usr/bin/google-chrome cargo xtask component-shot all
CHROME_BIN=/usr/bin/google-chrome cargo xtask component-shot skeletons --variant detail
```

The xtask serves built assets on an OS-assigned loopback port, opens menu/select
states, waits for fonts, then captures tight bounds at 2× DPR on the dark canvas.
It includes keyboard selection/typeahead/focus/collision checks. `--output <dir>`
changes the destination (default `target/component-shots`).
`--before-assets <absolute-dist-path>` captures the real previous UI using the
same browser and crop mechanism; held local requests reveal actual loading and
submitting states. Native OS select menus are not part of Chrome page screenshots;
the baseline shows the focused native trigger, while the after image includes the
new rendered listbox. Add full-page captures when a page's layout changes.


TIK-40 adds `/design/settings/{account,api-tokens,projects,appearance}` and isolated
`/design/c/sidebar` and `/design/c/user-menu` fixtures. All use Sample User and
example repository metadata. Settings uses a compact sub-navigation with the
Design system link in its footer. Tickets is dark only. Reduced motion overrides
persist per browser; the retired theme preference is removed on startup. Density
remains the pinned compact scale.

### Ticket composer (TIK-39)

`TicketComposer` is the shared creation surface on the board, list and detail.
Board column actions inherit that status; general actions start at Todo. Project
selection inherits the current view. Project, all six statuses and priority use
`SelectField` (with ProjectIcon, StatusIcon or PriorityIcon); existing labels use
multi-select `Combobox`. The isolated modal specimen uses the same component.

Tab follows title, Write/Preview, description, project, status, priority, labels,
then footer actions. Cmd/Ctrl+Enter submits; Cmd/Ctrl+Shift+P toggles preview.
The segmented indicator moves with a 180ms strong ease-out transition; keyboard
changes are immediate. Both editor modes share a fixed, scrollable container;
empty preview has explicit muted copy. Escape, Cancel and close protect a typed
title/body with Keep editing / Discard draft. Escape from a popup closes only the
popup; Escape from discard confirmation returns to the draft.

Creation locks fields and shows the shared BusyButton. Labels attach after the
POST succeeds. If an attachment fails, the saved ticket stays on the board and
Retry labels retries only missing links; it never repeats the create POST.
The shared keyed card/list motion handles successful insertion. Keyboard actions
and reduced-motion preferences retain the foundation's quieter behavior.

`component-shot modal` captures the closed-menu composition, both segmented
states, preview, the complete open status menu, desktop page and mobile view.
`--before-assets <dist>` uses the same real composer route and fixture HTTP data;
the old locked status is captured as `before-status-static.png`.


TIK-41 captures `/design/settings/api-tokens` with deliberately invalid fixture
credentials using `cargo xtask tokens-shot`. It produces dark desktop/mobile
viewport captures and cropped 2× states in `docs/screenshots/TIK-41/`. The real
local lifecycle, including failed create/revoke and clipboard denial, is covered
by `settings-check`; no real secret is included in its published screenshots.

### Home (TIK-43)

`/` and `/home` use a compact metric strip, status-stacked project counts,
weekly created/completed bars, and review/blocked queues. `/design/home` is the
sample-data specimen. Shared controls and ascii-reveal skeletons reserve loading space;
refresh retains same-scope data. SVG charts have a labelled data-table disclosure
with exact cohort links; unavailable weeks remain gaps. Counts use tabular figures
and status tokens. See [Home metric contract](design/home-analytics.md).
## Command palette (TIK-44)

Search and commands share a 640px dialog with a fixed search field and keyboard
footer. Rows are 36px (44px on narrow screens); keys use Geist Mono and titles
truncate. Recently viewed and in-progress/assigned tickets precede commands in the
empty query. Typed results are relevance ordered, with exact ticket keys first.
Project and status filters use `p:QUA` and `status:todo`. No result, loading and
retry states live inside the same surface.

Input focus stays put while arrows update `aria-activedescendant`. Enter opens
and Cmd/Ctrl+Enter or Actions opens the highlighted ticket's commands. Each ticket
also has a touch-accessible action button. Results use a combobox/grid pattern;
the scroll region is omitted from the tab order. Nested
status, priority and project choices show the saved value. Escape returns a level,
then restores opener focus on close. This frequently invoked surface uses an
opacity-only transition; shared modal containment and reduced-motion preferences
still apply. `/design/c/palette` is a fixture-only preview of this same component.

### Compact board cards (TIK-46)

Cards use option B (Dots): Geist Mono key left, priority and shared icon-only
status select right, title clamped at two lines with its full hover title, then
label colour dots and comma-separated short names (up to three) with ellipsis.
The label summary hover title retains full visible label names. Priority labels
stay stored and are hidden on cards. Comment counts sit at the footer right;
blocked glyphs sit beside priority in the header. The same layout serves fixture and API
cards. Whole-card and whole-list-row clicks open the ticket while nested
controls/links retain their own actions. Enter opens the focused surface.
Card press scales to .97, except during drag or reduced motion.

Blocked board/list tickets keep their normal status. A 14px blocked glyph uses
`--blocked` (`#f07856`) in a focusable 16px slot; the shared tooltip names each
unresolved blocker by key and title. Resolved dependencies clear live.

`cargo xtask card-shots` captures fixture-only 2× cropped long-title, many-label,
urgent and blocked cards plus desktop/mobile board and list PNGs.
`TICKETS_BEFORE_ASSETS=/absolute/base/crates/web/dist` captures matching baselines.

### Menu spacing (TIK-53)

All DropdownMenu, SelectField and Combobox options use the shared `.menu-item`
style: `--menu-item-gap: 2px`, `--menu-inset: 4px` and
`--menu-item-padding: 6px 8px`. Rounded highlights stay inset from the popup
and separate from adjacent items. Separators use the same gap and inset.
The sidebar account trigger shows the display name (falling back to username)
and initial at the bottom; its shared popup opens above with a bottom-left origin.
## Ticket activity (TIK-60)

`HistoryEvent` is shared by API detail, `/design/ticket` and `/design/c/history`.
Compact rows align a 16px shared glyph with actor/action/value text and a relative
`time` (click/tap or keyboard activation toggles exact UTC). Title and description changes expand to escaped
before/after text, keeping large bodies out of the scanning path. The real Activity
feed merges comments and history newest first; older history loads explicitly.
Live invalidations merge new entries through the loaded boundary, retaining older
pages and open diffs. Relative time refreshes each minute. Failed reads preserve
saved content.
Use `component-shot history` and `detail-check` for dark desktop/mobile evidence.

### Pickup rules (TIK-61)

Settings → Pickup rules extends the existing form with a shared scope SelectField,
compact labeled checkbox rows, numeric capacity/lease fields, and explicit Save.
Workspace inheritance stays visible; projects can save a complete override or use
workspace rules again. Unsaved edits lock scope until saved/discarded. Errors keep
inputs intact. `/design/c/pickup-rules` and `/design/settings/pickup-rules` use sample
data only; `component-shot pickup-rules` captures the rendered editor and pages.
