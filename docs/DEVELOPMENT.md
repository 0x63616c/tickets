# Local development

The workspace pins stable Rust 1.99.1 with rustfmt, clippy and the
`wasm32-unknown-unknown` target. Run `rustup show` to install it. Use Node.js 22
or newer for Wrangler. Install the build tools:

```sh
cargo install worker-build --version 0.8.7 --locked
cargo install trunk --version 0.21.14 --locked
npm install --global wrangler@4.154.0
```

From the repository root:

```sh
cargo xtask build    # Trunk release assets, then workers-rs release bundle
cargo xtask dev      # Build both, then wrangler dev --local (port 8787)
cargo xtask migrate  # Apply pending versioned SQL to local D1 only
cargo xtask ci       # fmt, native build, host + wasm clippy, host tests
```

All Cargo build output stays in this worktree's `target/`. Wrangler rebuilds the
Worker when its source changes using `[build]` in `wrangler.toml`. After frontend
changes stop the dev server and run `cargo xtask dev` again, then refresh the
browser. This rebuilds the assets and refreshes Wrangler's asset manifest. Run each command from
the workspace root so Cargo can resolve the xtask alias.

Visit http://localhost:8787 for the Leptos CSR placeholder. There is no ticket CRUD
or authentication yet. Static assets use SPA fallback; `/api` and `/api/*` always
reach the Worker and unknown API routes return 404 rather than HTML.

```sh
curl --fail http://localhost:8787/api/health
# {"ok":true,"version":"0.1.0"}
```

The all-zero D1 identifier is a local placeholder. The initial migration is a
no-op; domain tables arrive in #3. Local state lives in ignored `.wrangler/`.
Repeated migration runs skip applied files. Add sequential SQL files rather than
editing applied migrations; see [ADR 0002](adr/0002-migrations.md).
Do not provision a remote database or deploy without explicit authorization.

## Workspace boundaries and CI

- `crates/core`: runtime-independent, host-testable types; no wasm dependencies.
- `crates/worker`: wasm32 Cloudflare entry point, D1 binding `DB`, assets `ASSETS`.
- `crates/web`: wasm32 Leptos CSR app, router and Trunk entry point.
- `xtask`: host CLI for the development commands above; subprocess failures propagate.

Browser and Worker code/dependencies are gated on wasm32 so whole-workspace host
checks work. Wasm clippy checks their real runtime implementations. Wasm tests
are compile-checked, not executed as native tests.

CI runs on every PR and push to main on GitHub-hosted Ubuntu. It runs
`cargo xtask ci`, both release wasm builds, gitleaks, and migrations against a
fresh local D1 in a separate job. Actions are pinned to full commit SHAs; Rust
artifacts are cached with Swatinem/rust-cache. There are no deploy steps.

For UI changes capture dark-mode before/after screenshots under
`docs/screenshots/` and reference raw branch URLs in the PR. There was no runnable
UI before the scaffold: its before screenshot is an explicitly labelled empty
dark baseline. Design and frontend ADR 0001 are reserved for #6.
