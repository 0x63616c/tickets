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
- Production (`tickets.worldwidewebb.co`) is configured but **never deployed without Calum's
  explicit OK**. Local dev uses `wrangler dev` with a local D1.

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
- After successful domain writes call the no-op `api::change_event` with entity,
  ID and action, ready for #16. Validate domain documents in core, keep transport
  in the authenticated Worker adapter, and exercise shared dispatch with SQLite.
- Keep each API request within six D1 calls including authentication, independent
  of page size, and each SQL statement within 100 bound parameters. The Workers
  Free plan allows only 50 queries per invocation. Ticket reads use one SELECT
  with ordered JSON aggregates for labels/dependencies; never query per result row.
  Counting Store regressions cover 100-ticket pages and mutation responses with
  the shared bearer/session auth statements (list/get: 2 calls; create/upsert: 4;
  PATCH with a project field and dependency creation: 5).
- Run `cargo xtask api-smoke` for an isolated local Worker/D1 end-to-end check;
  it builds the app and uses port 8795 and generated state under `target/`.

## Authentication conventions
- All `/api/*` handlers must pass through the Worker auth gate, except health and
  login. Invalid Authorization headers must not fall back to a cookie.
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
  Use equal flex columns with a 240px minimum and 16px gaps. At 1440px the
  sidebar contracts to leave 1304px for five columns; narrower boards scroll.
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
- J/K focuses the next/previous visible row; Enter opens a read-only ticket
  preview pending #9. Native inline selects edit status and priority. Failed
  writes keep saved state and restore the select; errors remain visible.
- `CHROME_BIN=<headless-chromium> cargo xtask list-check` verifies the list
  against isolated local Worker/D1 state and captures 1440×900 / 390×900 images.
