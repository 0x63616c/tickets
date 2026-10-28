# Contributing

## Issues are the source of truth
- Every piece of work has a GitHub issue with a **Done when** checklist.
- Every open issue carries exactly one model label:
  - `model:astra` – design work only (design system, visual design/mockups, UI ADRs).
    Run: `codex exec -m gpt-6-astra -c model_reasoning_effort=xhigh`.
  - `model:sol` – all implementation work. Run: `codex exec -m gpt-6.1-sol`.
- Dependencies are written as `Blocked by #N` in the issue body; the issue carries
  `status:blocked` until every blocker is closed.
- Type labels: `type:feat`, `type:fix`, `type:chore`.

## Pull requests
- One focused PR per issue. Branch name: `<type>/<issue#>-<slug>`.
- PR title in Conventional Commits form (`feat:`, `fix:`, `refactor:`, `chore:`, `docs:`,
  `test:`, `ci:`, `perf:`).
- PR body contains `Closes #N`, a summary, and test evidence.
- **UI PRs must include before and after screenshots** (dark mode).
- CI must be green: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test`.
- Squash-merge; delete the branch after merge.

## Commits
- Author: `Calum <6991398+0x63616c@users.noreply.github.com>`.
- Never put personal info (emails, names, addresses) in commits, code, headers or payloads.

## Deploys
- Every merge to `main` deploys to production (`tickets.worldwidewebb.co`) via the CI
  `deploy` job, after all checks pass (approved by Calum). Only merge work that is ready to
  ship, and keep migrations backwards compatible. Never deploy manually or touch production
  resources outside CI without Calum's explicit OK. Local dev uses `wrangler dev` with a
  local D1. See [docs/DEPLOY.md](docs/DEPLOY.md).

## Rust workspace conventions
- Pin the stable toolchain and wasm target in `rust-toolchain.toml`; commit `Cargo.lock`.
- Keep `crates/core` free of wasm dependencies. Gate runtime-specific Worker and web
  code/dependencies on wasm32 so workspace host tests remain runnable.
- Run clippy on wasm32 as well as the host to cover runtime-specific implementations.
- Use `cargo xtask build`, `cargo xtask dev`, and `cargo xtask ci`; see `docs/DEVELOPMENT.md`.
- For the initial scaffold only, with no pre-existing runnable UI, the before
  screenshot is an explicitly labelled empty dark baseline.

## Data model conventions
- Store timestamps as UTC ISO 8601 text (`YYYY-MM-DDTHH:MM:SS.sssZ`). Repository
  updates set `updated_at`; creation timestamps are immutable. Explicit API project
  moves allocate a fresh destination number atomically and advance its counter;
  an unchanged project preserves its number. Old keys stop resolving after a move.
- Status values in SQL/JSON are `backlog`, `todo`, `in_progress`, `in_review`,
  `done`, `canceled`. Priority is 0 (none), 1 (urgent), 2 (high), 3 (medium), 4 (low).
- Project prefixes are 1–10 uppercase ASCII letters; colors are `#RRGGBB`.
  Ticket numbers are allocated atomically per project and never reused after deletion.
- Dependency edges point from blocker to blocked. Core validates additions; the SQL
  trigger also rejects cycles atomically. Remove/reinsert edges rather than updating them.
- Auth tables accept hashes only; hashing/verification and bootstrap belong to auth (#4).
  External IDs must be namespaced by source/repository (for example `github:org/repo:123`).
- Keep D1 transport in the Worker Store adapter. Test shared parameterized repository
  queries using SQLite with foreign keys enabled and the complete migration chain.
- Render descriptions/comments only through `tickets_core::markdown::render`; the
  returned HTML is sanitized, including raw HTML and forced-disabled task checkboxes.
- Domain lists use ascending ID cursors (`data`, `next_cursor`, default limit 50,
  maximum 100). Full-text search treats q as a literal FTS5 phrase. API mutations
  reject unknown fields; PATCH null clears nullable fields. Comments use the
  single-user author `owner`. Errors use `error.code` and `error.message`.
- After successful domain writes return internal change metadata for the Worker
  to notify project rooms; notification failure must never fail the write. Validate
  domain documents in core, keep transport in the authenticated Worker adapter,
  and exercise shared dispatch with SQLite.
- Keep each API request within six D1 calls including authentication, independent
  of page size, and each SQL statement within 100 bound parameters. The Workers
  Free plan allows only 50 queries per invocation. Ticket reads use one SELECT
  with ordered JSON aggregates for labels/dependencies; never query per result row.
  Counting Store regressions cover 100-ticket pages and mutation responses with
  the shared bearer/session auth statements (list/get: 2 calls; create: 4; upsert: 5;
  PATCH with a project field: 6; dependency creation: 5).
- Run `cargo xtask api-smoke` for an isolated local Worker/D1 end-to-end check;
  it builds the app and uses port 8795 and generated state under `target/`.

## Authentication conventions
- All `/api/*` handlers must pass through the Worker auth gate, except health, login, and the independently HMAC-authenticated GitHub webhook. Invalid Authorization headers must not fall back to a cookie.
- Cookie-auth mutations require a matching Origin; bearer-auth mutations do not.
- Never return credential hashes in JSON or log request bodies/authorization.
- Auth timestamps follow the domain UTC format. Sessions expire after seven days;
  personal tokens remain active until explicitly revoked.
- Bootstrap password hashes live only in Worker secrets; D1 has no password column
  (migration 0004 drops it). Never persist the bootstrap hash in D1.
- Password hashes store canonical positive PBKDF2-SHA256 iteration counts up to
  the production WebCrypto maximum (100,000); new hashes use that maximum.
  Generate a passphrase of at least 16 characters with `cargo xtask auth-hash`.

## Frontend design conventions
- Leptos CSR is locked; see `docs/adr/0001-frontend.md`. Use plain CSS with
  `crates/web/style/tokens.css` roles, semantic controls and the shared inline SVG
  family. The visual/interaction reference is `docs/design-system.md` and `/design`.
- The `/design` routes are fixture previews. Label them as sample data and keep
  API/auth/live-transport integration in its implementation issue.
- Descriptions and comments must use `tickets_core::markdown::render` (pulldown-cmark followed
  by ammonia). Never feed raw Markdown or user HTML directly to `inner_html`.
- Keep tool logic in Rust `xtask`. `cargo xtask design-check` checks a local
  running UI through Chromium and captures 1440×900 dark screenshots. See
  `docs/design-system.md` for browser, server, and output overrides.
- UI screenshots must show actual rendered routes with fonts loaded. Include
  the previous real UI as the before image; the scaffold exception no longer
  applies. Font OFL notices live alongside the bundled fonts, not as a repo license.

## Agent skill verification

- When changing the agent skill or API, run `cargo xtask skill-check`. It extracts
  every fenced `sh` curl example from `.agents/skills/tickets/SKILL.md`, executes
  it directly (no shell), and checks the adjacent `# expect` HTTP status and JSON
  subset. Keep examples as curl argv with only `$TICKETS_URL`/`$TICKETS_TOKEN`
  substitutions and backslash continuations; every command needs an expectation.
  Array assertions match length and order; object assertions allow extra fields.
  The task reuses api-smoke's isolated local D1 lifecycle and port 8795, so do not
  run both concurrently. CI runs the skill check; no production access is used.

## Kanban ordering and verification

- The API board lives at `/`; `/design*` retains its fixture-only state. Projects
  are selected in the sidebar. Hide Canceled by default; `?canceled=true` reveals it.
  Match the fixture's four default columns; show In review when populated.
  Status selects always offer all six statuses. Use equal flex
  columns with a 240px minimum and 16px gaps. The sidebar stays 240px; extra
  columns scroll inside the board. The footer counts tickets in visible statuses.
- Sort a project/status column by `sort_order`, then numeric ticket ID to resolve
  ties (including the API default of zero). A move assigns gapped integer ranks
  `1024, 2048, …` to the destination column and PATCHes only changed rows. Include
  filtered-out cards in that order so filters do not discard their positions.
  These per-ticket writes are sequential, not atomic. On failure, compensate
  completed writes and restore the local snapshot; if compensation fails, ask
  the user to reload and reconcile with the server. #16 should defer incoming
  events during a local move, then refetch authoritative state.
- Cards: Tab to focus; arrows navigate. Space picks up, arrows change column or
  position, Space saves, Escape cancels. Status selects remain the touch-friendly
  alternative. Mouse drops on a card insert before it; drops on a column append.
- `CHROME_BIN=<headless-chromium> cargo xtask board-check` builds the app, starts
  an isolated local Worker/D1, seeds through the API, and verifies keyboard/mouse
  moves, ordering after reload, filters, pagination, rollback, focus, creation and
  project isolation. Screenshots are written to `docs/screenshots/kanban/` at
  1440×900 and 390×900. `board-before` captures the previous real `/` route on a
  running local Worker at port 8787 before changing frontend assets.

## List view conventions

- `/list` shares API ticket loading and creation with the board; `/design/list`
  remains the fixture reference. Group by the six domain statuses, omitting empty
  groups. Show all projects unless `project` is supplied.
- List query parameters are `project` (ID), `label` (ID), `assignee` (exact name
  or `__unassigned`), `status` (display name), `q` (key/title substring), and
  `sort` (`priority`, `updated`, `created`). Priority defaults to urgent first,
  none last; dates are newest first, with numeric ID breaking ties.
- J/K focuses the next/previous visible row; Enter opens the ticket
  detail editor. Native inline selects edit status and priority. Failed
  writes keep saved state and restore the select; errors remain visible.
- `CHROME_BIN=<headless-chromium> cargo xtask list-check` verifies the list
  against isolated local Worker/D1 state and captures 1440×900 / 390×900 images.

## Ticket detail conventions

- `/ticket?id=<numeric ID>` is the API-backed detail route. Board titles and list
  titles/Enter open it; `/design/ticket` remains the fixture. Numeric links survive
  project moves, which change the displayed key. Detail inherits its ticket's project
  for creation; the shared composer permits selecting any project and defaults to Todo.
- Save ticket title/description explicitly with Edit ticket. Property selects save
  immediately; assignee saves on change/Enter (blank clears it). Saved state
  stays visible on failure. The dependency dialog displays the API's exact error
  inline and remains open after a rejected addition.
- Description/comment previews and saved bodies use the shared sanitized Markdown
  component. Comments offer Write/Preview for both creation and editing; whitespace
  cannot submit. No activity events are fabricated from ticket timestamps.
- `CHROME_BIN=<headless-chromium> cargo xtask detail-check` starts isolated local
  Worker/D1 state, seeds through the API, verifies editing/creation/comments/XSS and
  dependency cycles, and captures dark 1440×900 / 390×900 images in
  `docs/screenshots/detail/`. For before captures only, set `TICKETS_BEFORE_ASSETS`
  to the absolute `crates/web/dist` directory built from origin/main in a temporary
  worktree. This captures the previous real detail route and list preview, plus
  the fixture target separately. Remove the temporary worktree after capturing.

## Polish audit conventions

- `CHROME_BIN=<headless-chromium> cargo xtask polish-check` seeds isolated local
  Worker/D1 state and audits the real board, list, detail, composer, commands,
  login, tokens, and empty states at 1440×900 and 390×900. It checks keyboard
  focus, overflow, compact row/control geometry, long text, and token lifecycle.
  Token secrets must be dismissed before screenshots; capture metadata only.
- Capture `before` from assets built in a temporary worktree of the base commit,
  using `TICKETS_BEFORE_ASSETS`. Capture `/design*` targets separately. Each pair
  uses the same seed, route, viewport, dark scheme, and loaded fonts. Keep the base
  SHA and any fixture-only boundaries in the screenshot directory's README.
- Search results open the selected ticket. Empty results have explicit copy;
  dialogs restore opener focus, and mobile navigation contains keyboard focus.
  Preserve native selects and the shared chevron rather than drawing new controls.

## Projects conventions

- `CHROME_BIN=<headless-chromium> cargo xtask first-run-check` builds and starts
  isolated local Wrangler/D1 with the complete migration chain and zero data.
  It logs in through `/login`, then uses visible UI controls only to create a
  project, two tickets, a Kanban move, a new label and a dependency verified on
  both tickets. No API seeding or token creation is used. CI runs it sequentially
  with the other port-8795 checks; six dark 1440×900 screenshots live in
  `docs/screenshots/1/first-run-*.png`.

- `/projects` uses the `/design/projects` table and shared Modal, fields and icon.
  Project color tints its initial badge; show its key prefix in muted mono when no description exists. Keep row actions at the right edge, revealed on hover/focus (always visible on touch). The header contains the sole project count. Archive hides sidebar entries only;
  Show archived permits editing/restoring them. Tickets remain accessible.
- Project dialogs use compact swatches from the shared color tokens. Use one GitHub Project field for a URL or owner/number; show owner type only for short input and the prefix-change hint only while editing.
- GitHub linking makes no network requests. Core parses owner/number or an HTTPS
  `github.com/users|orgs/<owner>/projects/<number>` URL; persist the owner type in
  `github_project_kind` (default `users`). Repository is a name under that owner.
- `CHROME_BIN=<headless-chromium> cargo xtask projects-check` verifies UI project
  lifecycle against isolated local D1 and captures desktop/mobile dark screenshots.
  Set `TICKETS_BEFORE_ASSETS` to origin/main assets for the real before route and
  separate fixture target, following the detail/polish baseline conventions.

## GitHub import conventions

- Keep GitHub migration tooling in native Rust xtask. `import-github` accepts only
  local HTTP loopback Tickets URLs by default; approved production imports require
  `--allow-remote` and HTTPS. The coordinator performs the production run after merge.
- Import identity is `github:<lowercase owner>/<lowercase repo>:<issue number>`.
  The explicit quantum → voxelite alias preserves the original namespace across
  the rename; reject other metadata/name mismatches until an alias is defined.
  Retry only replay-safe GET/PUT/fixed-value PATCH requests; creation and
  relationship mutations stop for reconciliation on rerun.
  Comments use repository/comment-ID Markdown markers until the API has native
  comment external IDs. Preserve markers on reruns; do not deduplicate by body.
- Blocked is derived from dependencies, not a seventh stored status. Preserve
  `status:blocked` labels and reconcile blocker → blocked edges after upserts.
- Run `cargo xtask import-check` for importer changes; it reuses isolated local
  api-smoke state/port 8795 and must run sequentially with other local checks.
- Importer PRs use `Refs #11` while production import remains pending;
  do not close the issue or mark the production migration complete prematurely.

## Live updates conventions

- `/api/v1/live/{project_id}` upgrades only after the common session/bearer gate;
  session upgrades require matching Origin. Each project uses one `BoardRoom` keyed
  by its numeric ID, with SQLite migrations and hibernating WebSockets. Never poll
  from an alarm or retain socket lists in object memory.
- Change events are typed invalidations, not ordered patches. D1 remains authoritative.
  Reconcile duplicate/own events and refetch after every reconnect. Defer incoming
  reconciliation during local board moves and detail writes/edits.
- `CHROME_BIN=<headless-chromium> cargo xtask live-check` verifies two-client
  broadcasts and two isolated browser windows, capturing `docs/screenshots/16/`.
  Use `TICKETS_BEFORE_ASSETS` for the real main baseline and separate fixture target.

## Login navigation conventions

- The Worker runs before assets. Only fonts, favicon and Trunk hashed JS/CSS/Wasm
  paths bypass session lookup; missing public assets return 404. Asset SPA fallback
  stays disabled: the Worker serves `index.html` only after the navigation gate.
  Missing cookies cost zero D1 queries; a well-formed cookie costs one session SELECT.
- `/login` mounts outside AppShell. Shared core `navigation::safe_next` validates
  decoded relative destinations on both Worker and client; API 401s navigate to
  login with the current path and query. Never redirect API responses themselves.
- `cargo xtask login-check` uses isolated local D1 and generated local credentials
  to verify navigation, assets, form errors/loading, next and desktop/mobile layout.
  `cargo xtask login-before` captures logged-out `/`, `/login` and `/design` from
  an origin/main Worker already running on port 8787. Captures live in
  `docs/screenshots/34/` at 1440×900 and 375×812; no production changes are needed.
- Capture the emulated viewport explicitly with Chromium's screenshot clip and
  `captureBeyondViewport`; the headless window can otherwise truncate desktop
  images to 1024×640. Verify the saved PNG dimensions before publishing evidence.

## GitHub mirror conventions

- Shared issue/comment mapping lives in core; importer and Worker use identical
  external IDs and hidden comment markers. GitHub is authoritative; never write back.
- Mirror ingestion uses D1 batches with JSON label lists and bounded cron pages.
  Advance stored cursors only after applied pages. Deletions require webhooks.
- Mirrored tickets (`external_id` starts `github:`) are read-only in the UI and
  for agents. Keep API writes available for importer compatibility.
- Run `cargo xtask mirror-check` for ingestion changes; its local lifecycle shares
  port 8795 with import/skill checks. Screenshots live under docs/screenshots/39/.


## Live catch-up conventions

- Browser visibility, focus, online and a 60-second visible-tab interval share the
  live invalidation debounce. Hidden tabs make no catch-up requests. Refreshes keep
  saved content visible and defer while moving tickets, editing fields or holding
  comment drafts. Recheck before applying responses so an edit begun during a read
  is preserved. Existing paginated REST reads retain the per-invocation D1 budget;
  there is no server-side polling or replay storage.
- Shared count labels use the web count helper (singular only at one), including
  fixture views and accessibility labels. Bare numeric badges retain their numbers.
- Issue #36 evidence is captured by `cargo xtask live-check` in
  `docs/screenshots/36/`; use origin/main assets via `TICKETS_BEFORE_ASSETS`
  for the real before route and a separate design target.

## Label creation conventions

- The ticket label menu filters names case-insensitively. A trimmed nonblank
  name without an exact match offers creation; Enter in the input creates it,
  while arrows focus existing rows for selection. Escape and successful writes
  restore the trigger focus. Enter reuses an exact name and keeps an already
  attached match; clicking an existing row toggles it.
- Before creating, refresh the complete label list and reuse a case-insensitive
  match. Default colors use a stable lowercase-name hash over the four label
  hues in `tokens.css`. Use shared Menu, TextInput and LabelChip controls.
- `cargo xtask projects-check` covers creation from zero labels, keyboard focus,
  case-insensitive reuse and persistence. Issue #45 captures use
  `docs/screenshots/45/` at 1440×900 and 390×844; `TICKETS_BEFORE_ASSETS`
  selects origin/main assets for the same real ticket route and a fixture target.
