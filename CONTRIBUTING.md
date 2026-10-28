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
