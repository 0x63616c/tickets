# Contributing

## Tickets is the source of truth
- Every piece of work has a Tickets ticket with a **Done when** checklist.
- Every open ticket carries exactly one model label:
  - `model:astra` – design work only (design system, visual design/mockups, UI ADRs).
    Run: `codex exec -m gpt-6-astra -c model_reasoning_effort=xhigh`.
  - `model:sol` – all implementation work. Run: `codex exec -m gpt-6.1-sol`.
- Dependencies use native blocker → blocked edges; blocked state is derived until
  every blocker is done or canceled.
- Type labels: `type:feat`, `type:fix`, `type:chore`.

## Pull requests
- One focused PR per ticket. Branch name: `<key-lowercase>-<slug>`.
- PR title in Conventional Commits form (`feat:`, `fix:`, `refactor:`, `chore:`, `docs:`,
  `test:`, `ci:`, `perf:`).
- PR titles end with `[KEY-N]`; bodies contain `Closes [KEY-N](https://tickets.worldwidewebb.co/t/KEY-N)`
  (or `Refs [KEY-N](https://tickets.worldwidewebb.co/t/KEY-N)` for references),
  a summary, and test evidence.
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

## Design-token lint

Run `npm ci --prefix crates/web`, then:

```sh
cargo xtask check design-tokens
npm run lint:css --prefix crates/web
npm run test:lint --prefix crates/web
```

The xtask command scans Rust in `crates/web` and runs pinned Stylelint over its
CSS. The npm CSS command invokes the same xtask/Stylelint pipeline with `--css`;
there is one policy and one baseline, including for inline Rust declarations.
`cargo xtask ci` runs both source checks and the policy tests before Rust builds.
`crates/web/stylelint.config.mjs` and its local rule enforce the contract in
DESIGN.md. Token definitions are exempt only at `crates/web/style/tokens.css`.
Stylelint disable comments cannot suppress findings. Keep the Stylelint version
exact in package.json and commit the generated npm lockfile.

Quantum's `xtask/lint-allowlist.json` pattern applies here: each key is a file,
rule and exact declaration/literal with an occurrence count. Every count must
match remaining debt, including decreases; remove stale entries in the same
change that fixes them. Counts cannot increase or move to a new key. Rust comments
are ignored; fixture copy containing literal values remains explicitly recorded.
Dynamic style bindings use property/expression-head counts, so their existing
API/geometry uses remain reviewable. New static styles belong in CSS.

The first inventory was **399 findings (354 CSS, 45 Rust)**. Four exact-equivalent
substitutions (skip-link offsets/padding and chart text size) leave **395**
allowlisted findings (350 CSS, 45 Rust), preserving the then-current UI. TIK-64
removes light appearance; Tickets now uses only the dark token values.
Broad cleanup is intentionally deferred while other UI tickets edit these files.
`cargo xtask check design-tokens --report` prints observed counts for inspection;
it never rewrites the baseline. Do not copy a larger report into the allowlist.

The no-growth comparison uses `LINT_BASE_REF` (default `HEAD`). CI fetches full
history and uses the PR base SHA or the previous main SHA, preventing growth
hidden in an earlier branch commit. An invalid/missing Git revision is an error;
only an absent baseline file at a valid revision bootstraps the initial rollout.
For a local branch-wide check, use
`LINT_BASE_REF=origin/main cargo xtask check design-tokens`. A file rename requires
retiring its old debt first; budgets are not transferable.

## Git hooks

Fast local hooks live in `.githooks/` (plain POSIX shell, following Quantum).
Enable them once per clone/worktree:

```sh
git config core.hooksPath .githooks
```

- **pre-commit**: token checks when web CSS/Rust, lint configuration or the baseline
  is staged; `cargo fmt --all --check` when Rust is staged; then
  `gitleaks git --pre-commit --staged --redact` on the staged diff. Token checks
  read all relevant **index contents**, including the staged baseline, so partial
  staging cannot hide a violation behind an unstaged fix. The complete source
  scan also detects stale entries on deletion. Rustfmt follows Quantum and checks
  the working tree. Expect a few seconds with a warm xtask build; the first run
  compiles xtask. Hooks never install dependencies or access the network.
- **pre-push**: `cargo clippy --all-targets --locked -- -D warnings` on changed
  Rust crates. Shared Cargo/toolchain configuration changes check the workspace.
  New branches compare against their merge base with origin/main; deletions and
  pushes without Rust changes skip clippy. Timing depends on changed crates and
  cache warmth. Full host/wasm checks and tests remain in CI.

Missing Cargo, Node or Stylelint blocks required checks with an installation
notice. Install web tools with `npm ci --prefix crates/web`. Missing gitleaks
skips only secret scanning with a notice (`brew install gitleaks`); CI scans
history. Job worktrees source their existing `.job.env` before Cargo commands.

Skip once with `git commit --no-verify` or `git push --no-verify`. Disable with
`git config --unset core.hooksPath`. Skipping local hooks does not skip CI.

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
  Checks using that port must be serialized across worktrees too, or run inside
  separate local network namespaces. Browser policies must permit loopback access;
  isolate any test-only policy override to the child process, never shared settings.

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

## Accessibility verification

- `CHROME_BIN=<headless-chromium> cargo xtask contrast-check` uses isolated local
  Worker/D1 state to check rendered WCAG text contrast (including composited
  label backgrounds and primary rest/hover/active/focus/disabled states), badge
  size, viewport overflow and sequential headings on every route. It captures
  dark 1440×900 / 390×844 evidence in `docs/screenshots/54/` and writes computed
  foreground/background pairs to `contrast-pairs.json`. Use
  `TICKETS_BEFORE_ASSETS` for origin/main assets built in a temporary worktree.
- Markdown headings beneath page titles compress skipped source levels while
  preserving nesting. Style the shared `markdown-heading` class independently
  of semantic heading level; board/list page titles use `sr-only` to retain the
  compact chrome.

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

- The API board lives at `/board`; `/design*` retains its fixture-only state. Projects
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
- Cards use the compact Dots layout: key, priority and icon-only status select;
  two-line title; label dots and up to three short names, with full labels in the
  hover title. Hide priority metadata labels without changing stored data.
  Card bodies and list row bodies open the ticket; nested controls and links
  retain their own actions. Enter opens a focused card/row; a drag never opens it.
- `CHROME_BIN=<headless-chromium> cargo xtask card-shots` captures fixture-only
  card variants and board/list desktop/mobile evidence in `docs/screenshots/TIK-46/`.
  Set `TICKETS_BEFORE_ASSETS` to base-commit built assets for matching before PNGs.
- Cards: Tab to focus; arrows navigate. Space picks up, arrows change column or
  position, Space saves, Escape cancels. Status selects remain the touch-friendly
  alternative. Mouse drops on a card insert before it; drops on a column append.
- `CHROME_BIN=<headless-chromium> cargo xtask board-check` builds the app, starts
  an isolated local Worker/D1, seeds through the API, and verifies keyboard/mouse
  moves, ordering after reload, filters, pagination, rollback, focus, creation and
  project isolation. Screenshots are written to `docs/screenshots/kanban/` at
  1440×900 and 390×900. `board-before` captures the previous real `/` route on a
  running local Worker at port 8787 before changing frontend assets.

### Board topbar creation

The board topbar shows connection state only; create tickets with a column's
labelled + button. The C shortcut defaults to Todo. Other views keep their
New ticket action. `cargo xtask component-shot board-topbar` captures fixture-only
44px board chrome and full-page desktop/mobile evidence; `--before-assets`
uses baseline assets. `board-check` verifies all six column creation defaults.

## List view conventions

- `/list` shares API ticket loading and creation with the board; `/design/list`
  remains the fixture reference. Group by the six domain statuses, omitting empty
  groups. Show all projects unless `project` is supplied.
- List query parameters are `project` (ID), `label` (ID), `assignee` (exact name
  or `__unassigned`), `status` (display name), `q` (key/title substring), and
  `sort` (`priority`, `updated`, `created`). Priority defaults to urgent first,
  none last; dates are newest first, with numeric ID breaking ties.
- J/K focuses the next/previous visible row; Enter opens the ticket
  detail editor. Shared SelectField controls edit status and priority. Failed
  writes keep saved state and restore the select; errors remain visible.
- `CHROME_BIN=<headless-chromium> cargo xtask list-check` verifies the list
  against isolated local Worker/D1 state and captures 1440×900 / 390×900 images.

## Ticket detail conventions

- `/t/KEY-N` is the case-insensitive API-backed detail route;
  `/ticket?id=<numeric ID>` remains supported. Board titles and list
  titles/Enter open the key route; `/design/ticket` remains the fixture. Numeric links survive
  project moves, which change the displayed key. Detail inherits its ticket's project
  for creation; the shared composer permits selecting any project and defaults to Todo.
- Scope detail async tasks to the keyed ticket page and cancel them on cleanup.
  Editor callbacks must not access child signals after an authoritative reload
  remounts those children. Browser checks print panic-hook console errors and fail
  on exceptions; first-run-check holds live-read/save responses across dependency
  navigation to exercise disposal without retries.
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
  Use the shared SelectField/Combobox/DropdownMenu and chevron; never reintroduce native selects.

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
- Projects show repository links from `github_owner` + `github_repo`, independently
  of `github_project_number`. Counts come from the project API across all tickets;
  done excludes canceled, and last activity is the latest ticket update (all statuses).
  `CHROME_BIN=<headless-chromium> cargo xtask project-repos-check` verifies immutable
  repo-only and linked-PR fixtures and captures desktop/mobile evidence under
  `docs/screenshots/TIK-42/`; `TICKETS_BEFORE_ASSETS` captures the same fixtures
  against base assets.
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

- The Worker runs before assets. Only fonts, favicon and app icons (exact paths in
  `is_public_asset`, incl. `/site.webmanifest`), Trunk hashed JS/CSS/Wasm
  and the exact hashed DOM-bridge/ascii-reveal core snippet paths bypass session lookup; missing public assets return 404. Asset SPA fallback
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

## Retired GitHub issues

- Since 2026-10-29 Tickets is the sole system of record. GitHub issues are imported,
  then GitHub Issues is disabled per repository. Imported tickets are fully editable;
  external IDs/URLs and comment markers retain provenance, never edit restrictions.
- Keys remain per-project: QUA, TIK, APP (apps), WWW (world-wide-webb), BOX
  (grok-box-config). Use native keys in PR references and dependency discussions.
- Keep the HMAC-authenticated webhook endpoint for pull-request events. Issue,
  issue_comment and label events return 202 ignored without D1 calls; no issue cron
  or GITHUB_MIRROR configuration remains. Historical cursor tables are unused.
- `cargo xtask mirror-check` now checks retirement, signatures and native imported
  ticket editing, with before/after dark screenshots in `docs/screenshots/tik-33/`.
- One-shot migration: `cargo xtask import-github --repo 0x63616c/world-wide-webb
  --project WWW --dry-run`. The project must already exist. Default state is open;
  use `--state all|open|closed`, `--since YYYY-MM-DD` (closed_at cutoff for closed
  issues only), `--exclude N,M` and optionally `--close-github` after reviewing the
  dry run. See docs/DEVELOPMENT.md for reconciliation and operational safeguards.

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
  hues in `tokens.css`. Use shared Combobox, TextInput and LabelChip controls.
- `cargo xtask projects-check` covers creation from zero labels, keyboard focus,
  case-insensitive reuse and persistence. Issue #45 captures use
  `docs/screenshots/45/` at 1440×900 and 390×844; `TICKETS_BEFORE_ASSETS`
  selects origin/main assets for the same real ticket route and a fixture target.

## Login recovery conventions

- Keep typed transport failures inside the web API adapter; existing domain callers
  receive their display strings. Login clears its password only on success or HTTP
  401. Network, rate-limit and server failures keep it available for retry and do
  not mark credentials invalid. Keep the decision host-testable in the web login module.
- `cargo xtask login-check` also blocks `/api/v1/login` through Chromium's network
  layer and captures dark desktop/mobile network failures in `docs/screenshots/56/`.
  With `TICKETS_BEFORE_ASSETS` pointing at origin/main assets built in a temporary
  worktree, it captures only the baseline network state without asserting new behavior.

## Impeccable design review

Impeccable **4.8.0** is installed project-locally for Codex, Claude and Cursor.
Before requesting review of a UI PR, run `/impeccable critique <target>` and
`/impeccable polish <target>` in the agent chat, resolve findings within scope,
then run the existing UI checks and attach before/after dark screenshots.
Read `DESIGN.md` first; its pinned taste and tokens win over Impeccable defaults.
Inter and near-pure-black surfaces are intentional. Tooling-only PRs need no UI
screenshots. `PRODUCT.md` supplies the product context expected by this release.

Always run Impeccable with `IMPECCABLE_NO_TELEMETRY=1 DO_NOT_TRACK=1`, including
its bundled launchers. Keep `"telemetry": false` in `.impeccable/config.json`.
From the worktree root, reproduce the install with:

```sh
IMPECCABLE_NO_TELEMETRY=1 DO_NOT_TRACK=1 npx -y impeccable@4.8.0 install --providers=codex,claude,cursor --scope=project --yes --no-hooks
IMPECCABLE_NO_TELEMETRY=1 DO_NOT_TRACK=1 npx -y impeccable@4.8.0 telemetry status
```

Use skill-only installation: automatic edit hooks are unnecessary for explicit
review and would execute third-party tooling on edits. Enabling any provider
hooks, including `.codex/hooks.json`, requires Calum's approval. The installer
places Codex skills in `.agents/skills/impeccable`; it does not need a Codex hook.
Downloaded engines are ignored; the version/checksum-pinned launchers fetch them
on first use. When invoking a bundled launcher, also set
`IMPECCABLE_HOME="$PWD/.impeccable"` to keep its fallback engine cache in this
worktree rather than the default home-directory cache. Do not commit runtime questions, seed keys, roll records, caches,
downloaded binaries, local overrides or `node_modules`.

To update, check the upstream release notes and installer help, choose an exact
version, then rerun the install above with that version and `--force`. Review the
third-party diff, retain `--no-hooks`, update the pins here and in `AGENTS.md`,
and verify telemetry status from the project root. Confirm the config alone
reports off as well (temporarily unset both opt-out environment variables for
that read-only status check). Recheck ignored runtime paths, inspect the staged
diff for secrets/binaries, and run the required Rust checks before opening a PR.

Keep installer-provided skill files verbatim, including upstream whitespace and
Windows line endings; apply repository formatting checks to our own files.

## Mobile ticket layout verification

- Keep blocked indicators visible at narrow widths. Ticket properties follow the
  title in DOM/keyboard order and precede description, dependencies and activity;
  desktop retains its right-hand properties column.
- `first-run-check` asserts the 390×844 layout with its UI-created unresolved
  dependency, including marker geometry, row containment and properties order.
- `TICKETS_MOBILE_LAYOUT_ONLY=1 cargo xtask contrast-check` captures the same
  blocked list/detail routes at 1440×900 and 390×844 in `docs/screenshots/55/`.
  Set `TICKETS_BEFORE_ASSETS` to origin/main assets from a temporary worktree for
  the before images; after captures also assert visibility and content order.

## Chromium startup diagnostics

- Browser checks use a fresh ignored profile and OS-assigned DevTools port for
  each launch attempt. Retry only launch/connect (three attempts, 30 seconds
  each); never retry assertions. Every owned child is killed and reaped on exit.
- Per-attempt stderr is retained in `target/*chromium*-attempt-*.log`, including
  successful launches. Startup errors include the final 30 log lines. CI uploads
  `target/*chromium*.log` when the check job fails; use these logs to distinguish
  profile/process failures from slow startup before changing launch policy.


Browser readiness predicates may retry CDP document/context replacement errors
across hard navigations, within the existing timeout. Browser actions and JavaScript
exceptions must still fail immediately; never replay writes to recover navigation.

## Pull request ingestion conventions

- PR ingestion uses an owner allowlist in
  `GITHUB_PR_OWNERS`, shared signed webhook, new PR modules and migration triggers.
- Latest per-PR snapshots persist even with no links, preventing stale restoration.
  GitHub timestamp ties prefer terminal states; same-state ties retain first delivery.
- Atomic snapshot writes resolve all keys with JSON joins; ticket read aggregates
  include ordered PR metadata without extra D1 calls. No GitHub network writes.
- `cargo xtask mirror-check` also runs signed PR acceptance and captures before/after
  dark desktop/mobile evidence under `docs/screenshots/32`. Set
  `TICKETS_BEFORE_ASSETS` to base-commit assets for before captures.

## Component review (TIK-38)

Default to isolated component PNGs before full-page captures. Run `cargo xtask build`,
then `CHROME_BIN=/usr/bin/google-chrome cargo xtask component-shot all --output docs/screenshots/<TICKET>` (or a single
name, e.g. `select`, `modal`, `icons`, `busy-button`). The fixture-only
`/design/c/<name>?variant=...` route excludes auth and API transport. Skeleton
variants are board/list/detail/projects. The harness uses an OS-assigned local
port, loaded fonts, dark background, tight component bounds and 2× DPR. It shuts
down its own server and browser; it does not share port 8795 with API checks.
Use `--output docs/screenshots/<TICKET>` and copy the PNGs to
`/workspace/tickets-polish/<TICKET>/`. `--before-assets <dist>` captures a previous
build's actual components/loading states; record that build's SHA in the evidence
README. Native OS select popups are not captured by Chrome's page screenshot;
show the baseline focused trigger alongside the new open listbox. Full-page
screenshots remain required when changing a page layout.

See docs/design-system.md for shared component signatures, controlled async values,
presence ownership, motion/reduced-motion tokens and fixture variants. Keep all
project icon schema changes additive and validate values in core.

## Sidebar and settings (TIK-40)

- `/` and `/home` render Home; `/board` renders Board.
  G then H/B/L navigates; `?` opens shortcuts.
- Account identity comes from `/api/v1/me`, with nullable `display_name` falling
  back to the authenticated username. Never substitute a literal You/Y avatar.
- `/settings` redirects to `/settings/account`; sections are `account`,
  `api-tokens`, `projects`, `appearance`. `/settings/tokens` redirects to
  `/settings/api-tokens`. Keep the token lifecycle UI for TIK-41 to redesign.
- Project counts use `open_ticket_count` from the API (exclude done/canceled).
  Tickets is dark only; ignore OS colour preferences and retire the old
  `tickets.theme` key on startup. Section disclosure and reduced motion use
  browser localStorage; unavailable storage must not block navigation.
- `cargo xtask settings-check` verifies account persistence, token lifecycle and
  redirects, shortcuts, preferences and mobile navigation in isolated local D1.
  It shares port 8795 and must run sequentially with other Worker checks.
- `cargo xtask settings-shot` captures fixture-only Settings sections and full
  pages in `docs/screenshots/TIK-40`; `TICKETS_BEFORE_ASSETS` selects the actual
  prior settings route. `component-shot sidebar` and `component-shot user-menu`
  provide 2× crops. Never capture a live account or revealed API secret.

## New-ticket composer verification

- The shared TicketComposer owns draft controls and keyboard behavior; BoardComposer
  owns API persistence. Keep fixture component previews on that same surface.
- Label attachment happens after creation. Retain the saved ticket on partial
  failure and retry only missing label links, never the successful create POST.
- `component-shot modal` checks preview geometry, empty state, six status icons,
  keyboard selection/preview, discard protection, reduced motion and mobile fit.
  `board-check` verifies all six starting statuses against local D1, context defaults,
  Tab/submit shortcuts, busy locking and partial label failure without duplicates.

## Post-import UI presentation

- Hide `priority:*`, `status:blocked`, and `later` label chips and picker options;
  retain labels and ticket relationships in storage. Priority comes from the field,
  Blocked from unresolved dependencies. Todo tickets tagged `later` appear in Backlog;
  explicit active/resolved statuses take precedence. Lists hide Canceled by default,
  with `canceled=true` or the explicit Canceled status filter showing them.
- Bare `#N` in ticket descriptions/comments links within the current project through
  the sanitized Markdown renderer. Code and existing links retain their content.
- Mobile ticket properties start collapsed; desktop properties remain visible.
  Created/Updated include UTC time. GitHub provenance appears in Activity without
  inventing an import timestamp; repeated leading model metadata is presentation only.
- `polish-check` also verifies TIK-45 fixture cases and writes matching before/after
  list/detail captures to `docs/screenshots/TIK-45/`.
- Shared Cargo artifacts can retain another worktree's compiled-in directory.
  Resolve xtask's workspace from its runtime working directory and inherit the
  caller's `CARGO_TARGET_DIR` (source `.job.env`); do not override it in subprocesses.
## API token UI conventions

- `/settings/api-tokens` leads with active token metadata. Token IDs distinguish
  duplicate names; Show revoked preserves access to history. Last used means the
  last successful authentication, not agent health. Tokens have full workspace
  access and no expiration; do not expose unsupported scope/expiry controls.
- Creation keeps the returned secret in memory in a locked shared Modal until
  the user explicitly acknowledges saving it. Never put it in storage or a toast.
  Copy failure permits manual selection. A browser unload warns while creating
  or displaying the secret. Revoke requires the token's name and ID in a confirmation.
- `CHROME_BIN=<headless-chromium> cargo xtask tokens-shot` captures fixture-only
  Settings pages and cropped 2× component states at desktop/mobile sizes in
  `docs/screenshots/TIK-41/`. The fixture secret is deliberately invalid. Use
  `settings-check` for the real local token lifecycle and `polish-check` for regression.

## Home analytics conventions

- `/` and `/home` show Home; `/board` is the API Kanban route. Board links,
  shortcuts and checks must use `/board`. Home only fetches aggregate analytics
  and project metadata until the user invokes the ticket composer.
- V1 flow uses eight explicit UTC Monday weeks. Count observed status transitions,
  never current done + updated_at. Null means unavailable. History must remain
  additive and atomic with domain writes; subject IDs survive deletion/ID reuse.
  See docs/design/home-analytics.md for the committed metric/deferred-scope contract.
- `cargo xtask home-shot` uses immutable fixture HTTP data on an OS-assigned port,
  checks Home interaction/states and saves docs/screenshots/TIK-43. Use
  `TICKETS_BEFORE_ASSETS` for the same fixture's previous real landing page.
  CI runs the fixture check after building the web assets.
  `cargo test -p tickets-worker --test analytics` exercises full-migration history,
  exact cohorts and one-read query budgets. Keep first-run-check's empty-D1 flow
  through Home and UI project creation, then verify current counts and a completion
  transition through its metric drilldown.
## Command palette (TIK-44)

- Cmd/Ctrl+K uses `/api/v1/search`, independent of loaded board pages. Exact keys
  rank first; ordered-character fuzzy title/body matching combines relevance with
  open status, update age and up to 20 locally viewed numeric IDs. Recent IDs are
  stored in `tickets.recently-viewed`; unavailable storage is harmless. Search is
  debounced and obsolete responses are canceled. Domain list `q` remains literal FTS.
- `status:todo` and `p:QUA` combine with words. Search input retains keyboard focus;
  arrows change the active descendant, Enter opens, Cmd/Ctrl+Enter opens ticket
  actions. Escape/empty Backspace returns from action/value levels; Escape at the
  root closes and restores the opener. Changes use existing PATCH routes and retain
  the action level on errors. Project moves preserve GitHub provenance.
- `CHROME_BIN=<headless-chromium> cargo xtask palette-check` exercises search,
  actions, clipboard, mutation retry and stale responses against isolated local D1.
  CI runs it sequentially with the other port-8795 checks. `component-shot palette` captures
  fixture-only states, 2× dialog crops, mobile and a full-page dark view. Pass
  `--before-assets <base crates/web/dist> --output docs/screenshots/TIK-44` for the
  previous real component, or omit `--before-assets` for the current implementation.

- `cargo xtask tokens-shot` checks fixture-only token states and captures desktop/mobile
  PNGs. Set `TICKETS_SCREENSHOT_DIR` to keep ticket-specific evidence separate
  from the default `docs/screenshots/TIK-41/` baseline.

## Loading feedback (TIK-47)

- Run `npm ci --prefix crates/web` before compiling the web crate, including
  direct Cargo wasm checks and Trunk builds. `cargo xtask build`, `dev`, `ci`
  and checks that call the build helper do this automatically. CI installs Node 22
  before those commands, including the merge-to-main deploy build.
- Keep `ascii-reveal` exactly pinned to `0.3.0` in `crates/web/package.json` and
  commit its npm lockfile. Import `/node_modules/ascii-reveal/dist/core.js` through
  wasm-bindgen; never port, copy or fork its animation code. `node_modules` is ignored.
- Use `AsciiSkeleton`/`LoadingIndicator` while pending and `AsciiReveal` or
  `use_ascii_reveal` at the data-ready boundary. BusyButton owns both phases for
  submit controls. Destroy controllers and stop skeletons on cleanup; honor OS
  reduced motion and the in-app override. Keep refresh data visible.
- `/design/c/skeletons?variant=board|list|detail|projects` and `/design/c/reveal`
  are fixture-only. `cargo xtask component-shot reveal` captures the latter.
  `CHROME_BIN=<headless-chromium> cargo xtask loading-check` holds fixture API
  responses on real routes, asserts moving sweeps, decode, cleanup and reduced
  motion. Original evidence and reproduction notes live in
  `docs/screenshots/TIK-47/`; current cold-load comparisons are described below.
- Serialize Trunk builds within a worktree and keep source assets stable until
  they finish; concurrent staging or mid-build CSS edits can mismatch hashed
  stylesheet URLs and files. Loading checks require every stylesheet to load.
- Loading/reveal hosts must stay within their container: constrain skeleton glyph
  hosts and clip the decorative overlay, with overlay positioning shipped in the
  initial stylesheet. Never suppress scrolling on html, body or the app shell;
  preserve the board's horizontal scroll and long pages' vertical scroll.
- `loading-check` also opens fresh Chromium profiles at 1280×800 and 390×844,
  disables cache, applies 150ms latency / 200KB/s download / 75KB/s upload and
  4× CPU slowdown, and samples document/main extents every animation frame from
  navigation through held skeletons, decode and settlement on Board, List,
  ticket detail, Projects and Home. Loading must never exceed the settled scroll
  extents (1px rounding allowance). Four paired Home PNGs and JSON geometry
  diagnostics live in `docs/screenshots/TIK-62/`; red edge markers visualize
  document overflow when headless Chromium hides native scrollbars. Use
  `TICKETS_BEFORE_ASSETS=<base-dist>` to record the same baseline without enforcing
  the regression assertion. Keep fixture/API and real animation boundaries clear.

## Ticket history (TIK-60)

- Route all ticket-affecting SQL through `mutation::execute`, including typed
  repository writes, relationship/deletion cascades and PR snapshots. The Store
  `batch` contract is atomic; never emulate it with independent D1 calls.
- Core owns snapshot field/set diffs. Compare inside the batch, never against an
  earlier HTTP read. Preserve the analytics triggers and `ticket_events` separately.
- History actor identity comes from authentication or signed webhook transport;
  use safe aliases, never account/token names or credential material. Future MCP
  adapters must pass their authenticated actor to the shared dispatch.
- History pagination alone descends by ID. Retain durable subject identity after
  deletion; never resolve old history using only a potentially reused ticket ID.
- `detail-check` now verifies history fields, pagination, raw-text disclosure and
  failed-save absence, and captures TIK-60 fixtures. Its before mode uses the same
  fixture on actual base assets. `component-shot history` captures isolated 2×
  rows. `import-check` verifies history attribution and replay idempotence.

## Pickup leases (TIK-61)

- Core owns pickup validation, pure eligibility and total ordering; shared SQLite
  dispatch tests keep the SQL adapter aligned. The claim selector and mutation
  scope run inside the TIK-60 write batch, never from a prior preview.
- Owner identity comes from auth. Require the per-acquisition claim ID on heartbeat
  and release; never renew an expired lease. Cron expires 25 leases per minute,
  preserving statuses changed by a human/webhook. Keep notifications bounded.
- `waiting-on-calum` is seeded by migration. Empty-workspace checks must allow
  this label; choose newly created label IDs from API responses where possible.
- `cargo xtask pickup-check` verifies local D1 claims, contention, heartbeat,
  release, cron and history. `settings-check` covers rule editing and overrides.
  `component-shot pickup-rules` captures fixture-only dark desktop/mobile and 2×
  crops; `--before-assets <base-dist>` captures the previous Settings appearance.
  See docs/design/claim-next.md for the lease contract and research.
