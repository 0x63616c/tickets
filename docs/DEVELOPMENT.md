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

For a faster frontend-only loop, run `trunk serve --port 8086` from `crates/web`
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

## GitHub issue import

With a local `wrangler dev` and local D1, create a local personal token through
Settings → API tokens and set `TICKETS_TOKEN` in your secure environment.
GitHub authentication uses `GITHUB_TOKEN`, falling back to `gh auth token`.
Neither credential is printed. The User-Agent is `tickets-import`.

```sh
cargo xtask import-github --repo 0x63616c/voxelite --project QUA --dry-run
cargo xtask import-github --repo 0x63616c/voxelite --project QUA
# A second local instance can use --url http://127.0.0.1:8790
cargo xtask import-check
```

The default Tickets URL is `http://127.0.0.1:8787`; without `--allow-remote`
only HTTP loopback URLs are accepted. With the flag, HTTPS base URLs are also
accepted. Nonlocal plain HTTP, URL credentials, paths, queries and fragments are
rejected. Tickets redirects remain disabled; GitHub follows up to five redirects
for renamed repositories.

Calum has approved the production import. The coordinator runs these commands
**after merge**, with `TICKETS_TOKEN` already set in the secure environment (never
pass it as an argument or print it). This PR does not perform the import:

```sh
cargo xtask import-github --repo 0x63616c/voxelite --project QUA --url https://tickets.worldwidewebb.co --allow-remote --dry-run
cargo xtask import-github --repo 0x63616c/voxelite --project QUA --url https://tickets.worldwidewebb.co --allow-remote
```

Both `--repo 0x63616c/voxelite` and `--repo 0x63616c/quantum` use the original
`github:0x63616c/voxelite:N` external IDs and comment markers. This explicit alias
keeps reruns stable before, during and after the rename. Repository metadata is
checked before destination writes; other renames fail loudly and require an
explicit identity alias before importing. Keep the original namespace; do not
start a new namespace under a renamed repository. Do not deploy or migrate remote
D1 as part of the import.

All issues, open and closed, are read with pagination; PRs are excluded.
`QUA` creates **Quantum** for `0x63616c/voxelite`; other repositories use their
repository name. An existing project is selected by key, including archived
projects. Existing tickets are upserted by `github:<owner>/<repo>:<number>`;
external URLs link back to GitHub. Closed issues become Done, other issues Todo.
GitHub Projects columns are deferred until the GitHub token has project scope.
Tickets has no stored Blocked status: `status:blocked` is retained as a label,
and `Blocked by #N` creates blocker → blocked edges after all ticket IDs exist.
The API marks a ticket blocked while an incoming blocker is not Done/Canceled.
A stale blocked label with only completed blockers therefore does not override
the API's derived state. Unresolved references fail the import.

The importer owns imported title, body, state, URL and label memberships. Labels
are created by name and colors updated to GitHub's colors (labels are global).
Dependencies between imported tickets are reconciled; unrelated edges survive.
Comments use a hidden Markdown marker containing the repository and GitHub
comment ID, so equal bodies remain distinct and edited comments update in place.
Keep those markers when editing imported comments locally. Comments removed
from GitHub are retained as history. Authors remain the API's single-user owner;
GitHub usernames and credential information are not added to comment bodies.
Rerun after a partial failure; source updates overwrite local edits to owned
fields. Do not run two imports concurrently.

Dry run reads source issues/comments and destination projects/labels, prints
issue numbers, states and planned counts, and makes no writes. The final write
report compares source open/closed counts with read-back Tickets counts, fields,
label names/colors, dependency edges and comment bodies; gaps exit nonzero.
API reads, external-ID PUT upserts and fixed-value PATCH updates retry 429, 5xx
and transient transport failures up to three times (four attempts total), with
1/2/4-second exponential backoff. Retry-After seconds and HTTP dates are honored;
GitHub rate-limit reset headers/403 are also supported. Each wait is bounded to
five minutes; a longer server delay fails for a later rerun rather than retrying
early. Response bodies and credentials are never echoed.

Project/label/comment POST creation is not retried: a lost response may follow
a committed write. Relationship POST/DELETE operations also stop on failure,
since duplicates/missing links return 409/404. Rerun the importer to reconcile
these writes through project keys, label names, comment markers and current
relationships. Never run concurrent imports.

`import-check` reuses api-smoke's isolated local Worker/D1 lifecycle, generated
credentials, port 8795 and ignored `target/api-smoke-*` state. A Rust HTTP server
provides paginated GitHub fixtures and a rate-limit response. It checks dry-run
absence of writes, PR exclusion, open/closed state, Markdown, label colors,
dependency direction and three comments (including equal bodies with distinct
IDs). It imports repeatedly, deliberately introducing drift before a rerun,
and checks stable ticket/comment identities and no duplicate projects/labels.
It also reverses a local dependency and adds an incorrect label before the
rerun, then confirms an unexpected imported ticket makes reconciliation fail.
Do not run it concurrently with api-smoke, skill-check or browser checks.

The check also proxies Tickets locally: it returns 429 once for a read and 503
once after committed PUT and PATCH writes, checking stable identities after retry.
It also loses a committed comment-create response, verifies safe recovery on
rerun, follows GitHub 301/307 redirects, rejects an unrecognized rename, and
imports under both voxelite/quantum names without duplicate tickets or comments.
