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

Visit http://localhost:8787 for the Leptos CSR workspace. Ticket CRUD arrives separately. Static assets use SPA fallback; `/api` and `/api/*` always
reach the Worker and unknown API routes return 404 rather than HTML.

```sh
curl --fail http://localhost:8787/api/health
# {"ok":true,"version":"0.1.0"}
```

The all-zero D1 identifier is a local placeholder. The first migration is a
no-op; subsequent migrations create domain and auth tables. Local state lives in ignored `.wrangler/`.
Repeated migration runs skip applied files. Add sequential SQL files rather than
editing applied migrations; see [ADR 0002](adr/0002-migrations.md).
Do not provision a remote database or deploy without explicit authorization.
Production configuration, one-time setup, and manual deployment steps are in
[DEPLOY.md](DEPLOY.md).

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

## Local authentication

Create a gitignored `.dev.vars` at the repository root. Use placeholder values
below only as a guide; never commit real credentials or generated tokens:

```dotenv
BOOTSTRAP_USERNAME="<workspace-username>"
BOOTSTRAP_PASSWORD_HASH="<generated-pbkdf2-hash>"
```

Run `cargo xtask auth-hash` and supply your password on standard input (at least
16 characters; trailing newlines are removed). It prints a salted WebCrypto-compatible
hash. Use a generated passphrase; length alone does not ensure entropy. Avoid putting
passwords in command arguments or shell history. Redirect the
hash into your local configuration securely. The format is
`pbkdf2-sha256$100000$<16-byte-salt-hex>$<32-byte-digest-hex>`.
The Worker secrets are authoritative: changing them changes future login credentials;
existing sessions remain valid until logout or their seven-day expiry. User id 1 is
created on the first successful login. There is no signup or invitation endpoint.

Visit `/login`, then **Settings → API tokens**. Tokens appear once after creation;
copy them before dismissing the message. All auth responses use `Cache-Control:
no-store`. Use `Authorization: Bearer <personal-token>` for private API requests.
Never put a token in a URL. `GET /api/v1/me` checks authentication;
`POST /api/v1/logout` deletes the current cookie session. Token endpoints are
`GET/POST /api/v1/tokens` (POST JSON `{ "name": "<agent-name>" }`) and
`DELETE /api/v1/tokens/<id>`. Lists include metadata, never token hashes or raw tokens.
Invalid bearer credentials return 401 even if a valid session cookie is present.

Cookie mutations, including login, require an `Origin` header matching the request
URL's origin. For local curl use `Origin: http://localhost:8787`; browsers set this
automatically. Secure cookies work on localhost in supported browsers. Production
requires HTTPS. Login permits five attempts per hashed source IP in a 15-minute
window (including successful attempts); stale counters are pruned on login.
`/api/health` is public; all other `/api/*` routes pass through the auth gate.

Reproduce the native password comparison with
`cargo run --release -p xtask -- auth-bench`. See ADR 0003 for its limits.

For the local integration check, create ignored `target/login.local` with
`{"username":"<workspace-username>","password":"<local-password>"}`, start
Wrangler on port 8787, and run `cargo xtask auth-verify`. It uses curl, stores
transient cookies under target, and consumes the login rate-limit window.
Before rerunning, clear only the local fixture counter with
`wrangler d1 execute DB --local --command 'DELETE FROM login_attempts'`.
Do not run this check against production. See [ADR 0003](adr/0003-auth.md) for
the accepted 100,000-iteration policy and residual risk.

`cargo xtask auth-screenshots` uses Chromium DevTools (host only) to capture login
and authenticated token metadata and exercise create/dismiss/revoke in Leptos.
Set `TICKETS_CHROME` to your Chromium executable, or put `chromium` on PATH.
Run auth-verify first to prepare its ignored browser storage. Screenshot captures
never include the one-time secret. The before screenshot for #4 reuses the actual
scaffold capture from the base branch, whose UI was unchanged by #3.

`cargo xtask skill-check` builds the app and runs every curl example in the
[agent skill](../.agents/skills/tickets/SKILL.md) against a fresh local Worker/D1.
It checks HTTP status, JSON response subsets, pagination, filters, dependency
conflicts, project moves and repeated external upserts. It shares api-smoke's
port 8795 and generated `target/api-smoke-*` state, and stops the Worker on exit.
Use it whenever the skill or API changes; CI also runs it. It supplies generated
fixture credentials, never credentials from your environment.
