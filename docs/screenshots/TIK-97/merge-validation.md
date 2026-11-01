# TIK-97 merge validation

Merged `origin/main` at `f9c4999dfee828e239c9a48ab40bd69a0ee7b0c9`
into `TIK-97-openapi` (previous head `4f7720ebea8b1ae9731b0a26a30bf4d9545127f4`).

Resolved the two documentation append conflicts by retaining both sections:
OpenAPI and ticket classification in AGENTS.md, and API reference and project
icon colors in docs/design-system.md. Preserved main's application changes and
the branch's route gates, static spec endpoint and native API reader.
Updated OpenAPI create/patch/read, search and Mission Control schemas for main's
nullable ticket type and normalized scope, plus the generated ticket PR title.
Refreshed this ticket's after screenshots against the merged build; retained the
existing before captures. Unrelated check-generated captures were restored.

## Environment and verification

All Cargo shells sourced `.job.env`, prepended `$HOME/.cargo/bin` to PATH and set
`CARGO_BUILD_JOBS=4`. The final check suite set `LINT_BASE_REF=origin/main`,
`TICKETS_SKIP_BUILD=1` and
`CHROME_BIN=/workspace/tickets-polish/chrome-noproxy` after a successful
`cargo xtask build`. That existing read-only wrapper supplies a private Chromium
policy for loopback access; shared browser policies and build caches were unchanged.
Every Worker/runtime check used isolated local D1. No remote migrations or
production changes were performed.

| Exact command | Exit |
| --- | --- |
| `cargo xtask ci` (initial run) | 0 |
| `npm run lint:css --prefix crates/web` (initial run) | 0 |
| `cargo xtask build` | 0 |
| `cargo xtask ci` | 0 |
| `cargo xtask check design-tokens` | 0 |
| `npm run lint:css --prefix crates/web` | 0 |
| `cargo xtask pickup-check` | 0 |
| `cargo xtask perf-check` | 0 |
| `cargo xtask perf-browser-check` | 0 |
| `cargo xtask settings-check` | 0 |
| `cargo xtask navigation-check` | 0 |
| `cargo xtask api-smoke` | 0 |
| `cargo xtask font-check` | 0 |
| `cargo xtask skill-check` | 0 |
| `node --test crates/worker/tests/github_app.mjs` | 0 |
| `cargo xtask import-check` | 0 |
| `cargo xtask mirror-check` | 0 |
| `cargo xtask first-run-check` | 0 |
| `cargo xtask board-check` | 0 |
| `cargo xtask mission-check` | 0 |
| `cargo xtask detail-check` | 0 |
| `cargo xtask palette-check` | 0 |
| `cargo xtask project-repos-check` | 0 |
| `cargo xtask board-loading-check` | 0 |
| `cargo xtask loading-check` | 0 |
| `cargo xtask home-loading-check` | 0 |
| `cargo xtask component-shot all` | 0 |
| `cargo xtask home-shot` | 0 |
| `cargo xtask page-layout-check` | 0 |
| `cargo xtask api-docs-check` | 0 |
| `cargo xtask migrate` | 0 |
| `gitleaks git --redact` | 0 |

The final `cargo xtask ci` ran locked npm installation, autofill and raw-color
checks, design-token lint, Node policy tests, `cargo fmt --all --check`,
`cargo build --workspace --locked`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo clippy --workspace --all-targets --locked --target wasm32-unknown-unknown -- -D warnings`,
and `cargo test --workspace --locked`; all exited 0. No tests or checks were
skipped or loosened, and no failure retries were needed. GitHub Actions itself
and production deployment were not run locally.

The API reference check verified public static JSON and headers, all 57 operations,
filter/empty/reset behavior, expanded schemas, session GET requests and dark
desktop/mobile containment. The compiled Worker tests verified zero binding/D1
accesses for the spec and auth-route dispatch. Host tests checked spec/route parity,
local refs, document structure and actual domain dispatch handlers.
