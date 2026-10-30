# Local development

The workspace pins stable Rust 1.99.1 with rustfmt, clippy and the
`wasm32-unknown-unknown` target. Run `rustup show` to install it. Use Node.js 22
or newer for Wrangler and the pinned ascii-reveal web dependency. Install the build tools:

```sh
cargo install worker-build --version 0.8.7 --locked
cargo install trunk --version 0.21.14 --locked
npm install --global wrangler@4.154.0
```

From the repository root:

```sh
npm ci --prefix crates/web # Required before direct Trunk/Cargo wasm builds
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

Visit http://localhost:8787 for the Leptos CSR design preview. `/design` is the
component gallery; board, list, ticket, project and empty-state routes are listed
in [the design system](design-system.md). Changes use in-memory fixtures and reset
on reload. API-backed ticket CRUD arrives separately. The existing `/login` and
`/settings/tokens` routes remain API-backed; see Local authentication below.
Static assets use SPA fallback; `/api` and `/api/*` always
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
dark baseline. Later UI changes use the real previous UI as their before image.

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

For a faster frontend-only loop, first run `npm ci --prefix crates/web` from the
repository root, then `trunk serve --port 8086` from `crates/web`
with `NO_COLOR=true` and `CARGO_TARGET_DIR` pointing to this worktree's `target/`.
Run `cargo xtask design-check` from the repository root to capture and check those
routes (default server: `http://127.0.0.1:8086`). To check the Worker instead, set
`TICKETS_DESIGN_URL=http://127.0.0.1:8787`. Use `CHROME_BIN` for an installed
Chromium executable if `/usr/bin/google-chrome` is unavailable or managed by a
proxy policy. The task writes screenshots under `docs/screenshots/design/` and
browser logs/profile data under ignored `target/`.

`cargo xtask skill-check` builds the app and runs every curl example in the
[agent skill](../.agents/skills/tickets/SKILL.md) against a fresh local Worker/D1.
It checks HTTP status, JSON response subsets, pagination, filters, dependency
conflicts, project moves and repeated external upserts. It shares api-smoke's
port 8795 and generated `target/api-smoke-*` state, and stops the Worker on exit.
Use it whenever the skill or API changes; CI also runs it. It supplies generated
fixture credentials, never credentials from your environment.

## GitHub import

Tickets is the sole system of record as of 2026-10-29. Run the importer once per
repository into a chosen existing project, then disable GitHub Issues after
reviewing the migration. It never creates projects. Keys remain QUA/TIK/APP/WWW/BOX.

```sh
cargo xtask import-github --repo 0x63616c/world-wide-webb --project WWW --dry-run
cargo xtask import-github --repo 0x63616c/world-wide-webb --project WWW --state all --since 2026-01-01 --exclude 7,12
```

Set TICKETS_TOKEN and GITHUB_TOKEN securely (GitHub defaults to `gh auth token`).
Tickets URL defaults to loopback; `--url https://... --allow-remote` is reserved
for an explicitly approved production migration by the coordinator after merge.
Never deploy or run remote D1 migrations as part of importing.

Default `--state open`; `all` and `closed` include closed issues. `--since
YYYY-MM-DD` filters closed issues by closed_at while retaining all selected open
issues. PRs and `--exclude N,M` are skipped. Title/body, labels (including model
labels), assignee, URL, comments with original author/timestamp and hidden marker,
state and priority are preserved. Open maps to Todo; closed completed to Done;
closed not_planned to Canceled. priority:p0..p3 maps to urgent..low.

After upserts allocate native keys, local #N body references become imported keys;
unknown references become full GitHub URLs. Code and existing URLs are retained.
Blocked-by API data, sub-issues (child blocks parent), `Blocked by #N` and `Depends
on #N` create edges only within this selected repository import. Invalid dependency
cycles stop before writes. Unrelated native dependencies are preserved.

Identity is `github:<lowercase owner>/<lowercase repo>:N`. The explicit quantum
alias retains `github:0x63616c/voxelite:N`, updating existing QUA tickets. Other
metadata/name mismatches require an explicit alias. Comments deduplicate by marker,
never body. Reruns update imported fields; retain markers when editing comments.
This is a migration tool: rerunning deliberately overwrites native field edits.

Dry run prints create/update plans without writes. Actual runs read back fields,
labels, comments and edges; gaps fail the command. `--close-github` waits for that
verification, posts `Moved to Tickets: <ticket URL> (KEY-N). Tickets is now the
system of record; this issue is closed.` once, then closes as completed (or
not_planned for Canceled). A failed ambiguous comment write stops; rerun reads
comments before trying again. Movement comments are excluded from imported discussion.
Reruns recognize matching movement comments and preserve the native Tickets status;
archiving an open GitHub issue must not complete its migrated task.
Final output includes per-repo imported/updated/skipped/closed counts and TSV
issue → key mapping. Do not run concurrent imports; replay-safe GET/PUT/PATCH
requests retry, creation/link writes stop for reconciliation on rerun.

Run `cargo xtask import-check`, `skill-check`, and `mirror-check` sequentially;
they share isolated local state/port 8795 and never access production.

## Retired GitHub issue ingestion

The signed endpoint remains for PR integration, while issue/comment/label events
return 202 ignored. There is no GITHUB_MIRROR var or reconciliation cron.
`cargo xtask mirror-check` verifies this contract against isolated local D1 and
checks native editing and imported author attribution with dark screenshots.
