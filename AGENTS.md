# Agent guide

Tickets is the sole system of record (2026-10-29); GitHub Issues is retired.
Use native project keys (QUA, TIK, APP, WWW, BOX), and PR bodies must contain
`Closes [KEY-N](https://tickets.worldwidewebb.co/t/KEY-N)` (or
`Refs [KEY-N](https://tickets.worldwidewebb.co/t/KEY-N)` for reference-only links).
Imported tickets are editable; preserve GitHub provenance and comment markers.
One-shot migration: `cargo xtask import-github --repo owner/repo --project PREFIX
--dry-run`; see CONTRIBUTING.md. Never run production imports without explicit OK.

Read `CONTRIBUTING.md` first; everything there applies.

- Pick the model from the issue's label: `model:astra` (design only) uses
  `-m gpt-6-astra -c model_reasoning_effort=xhigh`; `model:sol` (implementation) uses
  `-m gpt-6.1-sol`.
- Work in a git worktree per issue. Never delete folders you did not create.
- Merging to `main` deploys to production automatically via CI. Never run `wrangler deploy`, remote migrations or other production changes by hand without Calum's explicit OK.
- Never commit secrets, API tokens, or personal info. Tokens are stored hashed only.
- Run `npm ci --prefix crates/web` before direct web/wasm or Trunk builds.
  `cargo xtask build`, `dev`, and `ci` install the locked dependencies automatically.
  Loading uses published `ascii-reveal` 0.3.0; keep wrappers thin, never vendor or port it.
- Tickets is dark only. Keep dark tokens and the black Stub; do not add theme
  settings, OS colour-scheme switching or alternate theme assets.
- Use design-system tokens for UI spacing, radius, colour, font size and shadow.
  Run `cargo xtask check design-tokens` and `npm run lint:css --prefix crates/web`;
  remove fixed debt from `xtask/lint-allowlist.json`, never grow it. See DESIGN.md.
- Before opening a PR run: `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- UI changes: default to `cargo xtask component-shot <name>` after building;
  `/design/c/<name>` renders fixture-only, tightly cropped 2× dark PNGs.
  Attach before/after dark-mode PNGs to the PR; add full-page captures for layout
  changes. See CONTRIBUTING.md and docs/design-system.md for variants and baselines.
- First-run acceptance: run `CHROME_BIN=<headless-chromium> cargo xtask first-run-check`;
  see CONTRIBUTING.md for its empty-D1, UI-only contract and screenshot outputs.
- If a convention is missing, add it to `CONTRIBUTING.md` / this file in the same PR.
- The agent skill for driving Tickets lives in `.agents/skills/tickets/SKILL.md`; keep it
  current when the API changes.

## Impeccable

- Project-local Impeccable is pinned to **4.8.0** for Codex (`.agents/skills`),
  Claude and Cursor. Always run it with `IMPECCABLE_NO_TELEMETRY=1 DO_NOT_TRACK=1`;
  `.impeccable/config.json` also disables telemetry for everyone.
- On UI PRs, run `/impeccable critique` and `/impeccable polish` before review.
  Read root `DESIGN.md` first: Calum's taste and its pinned tokens override
  Impeccable defaults, including warnings about pure black or Inter.
- Skill-only installation: no provider hooks are installed. Do not enable hooks
  (including `.codex/hooks.json`) without Calum's approval.
- Set `IMPECCABLE_HOME="$PWD/.impeccable"` for bundled launchers so fallback
  engine downloads stay in the ignored project cache.
- Installation, verification and version updates: see CONTRIBUTING.md.

## Ticket classification and PR titles

Ticket titles are plain (for example `Grid home`, never `feat(ui): Grid home`).
Ticket `type` is nullable and must be exactly `feat`, `fix`, `chore`, `refactor`,
`docs`, `perf`, `test`, `ci`, or `build`. Optional nullable `scope` is trimmed and
lowercased and must match `[a-z0-9][a-z0-9._/-]{0,31}`; an empty string clears it.
The shared `tickets_core::ticket_type::pr_title` helper uses the title verbatim:
`<type>(<scope>): <title> [KEY-N]` with scope, or `<type>: <title> [KEY-N]`
without scope. An unset type falls back to `chore`, including when scope is set.
