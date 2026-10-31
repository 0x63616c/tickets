# Self-hosting Tickets

This guide provisions **your own** Cloudflare account and GitHub fork. It does not
change the maintainer's instance. Tickets is currently a single-login workspace:
all valid sessions and API tokens can access all projects. There is no signup or
invitation flow. Review the [fork checklist](#what-you-must-change-when-forking)
before deploying; some branding and ticket links still require source edits in
your fork.

## 1. Install the tools

You need a Cloudflare account with Workers and D1 available, Git, rustup, and
Node.js 22 (22.15+ for the compiled Worker callback test). The repository pins
Rust and the wasm target in `rust-toolchain.toml`.

```sh
git clone https://github.com/YOUR-OWNER/tickets.git
cd tickets
rustup show
cargo install worker-build --version 0.8.7 --locked
cargo install trunk --version 0.21.14 --locked
npm install --global wrangler@4.154.0
npm ci --prefix crates/web
wrangler login
```

Workers Free supports SQLite Durable Objects, but has small CPU and daily query
budgets. The maintainer's deployment uses Workers Paid. For planning, the
[deployment notes](DEPLOY.md#free-tier-limits-worth-knowing) record these Free
allowances: Workers 100,000 dynamic requests/day and 10 ms CPU/invocation; D1
500 MB/database, 5 GB/account, 10 databases, 5 million rows read/day and 100,000
written/day; SQLite Durable Objects 100,000 requests/day, 13,000 GB-s/day,
5 million SQL rows read/day, 100,000 written/day, 5 GB SQL storage and 100 classes.
These are shared account allowances, not a capacity guarantee for Tickets.
Check the Cloudflare limit/pricing links there before choosing a plan; watch
login CPU, D1 usage and live connection traffic on your own workload. Paid billing
is an account-owner choice, not a setting this guide changes.

## 2. Configure your Worker and hostname

Edit your fork's `wrangler.toml` before any remote command:

- Top-level `name = "tickets"` identifies the default Worker configuration used
  for local development. `[env.production]` uses `name = "tickets-production"`;
  choose a unique name for your account. Keep the environment name `production`:
  xtask's remote migration command and CI use it explicitly.
- For a custom domain, change the production `routes` pattern to your hostname,
  keeping `custom_domain = true`. Its zone must be on Cloudflare. The checked-in
  `workers_dev = false` and `preview_urls = false` disable alternate endpoints.
- To use workers.dev instead, remove the custom-domain `routes` entry in your
  fork and set `workers_dev = true`. Decide separately whether to enable
  `preview_urls`; keeping it false avoids version preview URLs. Use the assigned
  workers.dev origin throughout GitHub App setup and CI smoke tests.
- Set `GITHUB_PR_OWNERS` in **both** `[vars]` and `[env.production.vars]` to your
  comma-separated GitHub user/organization owners. Named environment vars do not
  inherit the top-level values. Empty/unset means no PR owner is allowed.

Keep `DB`, `ASSETS`, `BOARD_ROOMS` and the `BoardRoom` class names: the code uses
these bindings. Keep the existing build and assets configuration.

## 3. Create D1 and apply migrations

```sh
wrangler d1 create tickets
```

Copy the returned database ID into `database_id` under
`[[env.production.d1_databases]]`, **replacing the maintainer's ID**. Keep
`binding = "DB"`, set `database_name` to the name you created, and keep
`migrations_dir = "migrations"`. If `tickets` already exists in your account,
choose another database name and update that field accordingly. The top-level
all-zero ID is a local-development placeholder; do not copy it into production.
Named-environment D1 bindings are configured separately.

After checking that the production binding points to your database:

```sh
cargo xtask migrate --remote
```

This runs `wrangler d1 migrations apply DB --remote --env production`. It needs
your authenticated account or CI credentials and changes the remote database.
For local development use `cargo xtask migrate` instead. Keep applied SQL files
immutable and add new migrations; migrations run before Worker deployment and
must remain compatible with the previous Worker.

## 4. Durable Objects, cron and storage

There is no Durable Object resource to create manually. `wrangler.toml` already
binds `BOARD_ROOMS` to the exported `BoardRoom` class locally and in production.
The top-level migration tagged `v1-board-room` uses
`new_sqlite_classes = ["BoardRoom"]` and applies when deploying the named
environment too. Keep that class migration; it is separate from D1 migrations.
Project rooms are addressed by project ID and created on demand. They broadcast
invalidation messages over hibernating WebSockets; D1 remains authoritative and
there is no replay log or duplicate ticket database in a room.

Both configurations include `crons = ["* * * * *"]`. This runs the lease sweep
every minute, clearing up to 25 expired claims per tick and returning tickets
still In Progress to Todo. Larger backlogs drain over subsequent ticks. It does
not reconcile GitHub Issues.

Tickets uses **neither KV nor R2**: there are no KV/R2 bindings and nothing to
create. Profile images are stored as D1 BLOBs. Static web assets use `ASSETS`.

## 5. Set secrets and optional vars

For local development, copy [`.dev.vars.example`](../.dev.vars.example) to
`.dev.vars` and fill it in. Wrangler reads that file; it is gitignored.
[`.env.example`](../.env.example) lists CI/xtask environment names as a reference;
xtask does not automatically load it. Neither example contains usable credentials.

| Worker secret | Required? | Purpose |
| --- | --- | --- |
| `BOOTSTRAP_USERNAME` | Yes, for login | Single workspace login name |
| `BOOTSTRAP_PASSWORD_HASH` | Yes, for login | Salted PBKDF2-SHA256 hash, authoritative in the Worker secret, never D1 |
| `GITHUB_WEBHOOK_SECRET` | Only for legacy hooks | HMAC key for `/api/v1/github/webhook`; independent of App credentials |
| `GITHUB_APP_STORE_KEY` | For GitHub App registration and operation | Standard base64 encoding of 32 random bytes; encrypts/decrypts saved App credentials with AES-256-GCM |
| `GITHUB_APP_SETUP_STATE` | During App registration only | Unguessable state authorizing manifest conversion; remove after registration to disable the callback |

Generate the password hash with:

```sh
cargo xtask auth-hash
```

Supply a generated passphrase of at least 16 characters on stdin, then EOF.
The command emits a randomly salted hash with 100,000 iterations. Avoid passwords
in command arguments or shell history. Paste the output literally at the secret
prompt, preserving its `$` characters. Set the required production secrets:

```sh
wrangler secret put BOOTSTRAP_USERNAME --env production
wrangler secret put BOOTSTRAP_PASSWORD_HASH --env production
```

If using the App, generate the store key and state with a cryptographic generator
(for example `openssl rand -base64 32` for each), keep them securely, and set:

```sh
wrangler secret put GITHUB_APP_STORE_KEY --env production
wrangler secret put GITHUB_APP_SETUP_STATE --env production
```

Only if you also use a legacy per-repository webhook:

```sh
wrangler secret put GITHUB_WEBHOOK_SECRET --env production
```

Never rotate the App store key without re-encrypting the stored row: otherwise
App credentials become unreadable. After registration, disable the callback with:

```sh
wrangler secret delete GITHUB_APP_SETUP_STATE --env production
```

Also remove its source CI secret so a later deploy cannot restore it. Password
rotation changes future logins; existing sessions last until logout or seven-day
expiry. Personal API tokens last until revoked.

`GITHUB_PR_OWNERS` is a non-secret var for the owner allowlist.
`GITHUB_API_BASE_URL` is an optional non-secret var used for local mock GitHub API
servers. Leave it **unset** in production to use `https://api.github.com`; an
empty override is not the same as leaving it unset.

## 6. Build and deploy your fork

Once your configuration points only to your resources:

```sh
cargo xtask build
wrangler deploy --env production
```

The build installs locked npm dependencies, builds Leptos assets with Trunk, and
bundles the Rust Worker with worker-build. Open your HTTPS origin, sign in and
create your projects. Check `/api/health`; unauthenticated `/api/v1/tickets`
should return 401. For subsequent releases prefer CI below. Maintainer-instance
manual deploys and migrations remain subject to [DEPLOY.md](DEPLOY.md).

## 7. Register your own GitHub App (optional)

Follow the [manifest flow](API.md#github-app-registration-phase-1) on **your**
instance: sign in, open Settings → Account → GitHub, and select Connect GitHub.
The manifest derives URLs from the request origin, including:

- Webhook: `https://YOUR-HOST/api/v1/github/app/webhook`.
- Callback: `https://YOUR-HOST/api/v1/github/app/manifest-callback`.
- Post-install setup: `https://YOUR-HOST/settings`.

The default display name is **Tickets by World Wide Webb**; `?name=alt` selects
**Tickets WWW**. Other name parameters are ignored. Change the source in your
fork for your own branding/name availability. Register an App you own rather
than reusing the maintainer's App or credentials.

The generated manifest currently requests Pull requests read, Issues read,
Checks write, Commit statuses write, Metadata read and Contents read. It
subscribes to `pull_request`, `check_suite`, `check_run` and `push`; push and issue
events are not ingested as tickets. Retain the permissions required for PR reads
and Tickets checks when reviewing the App configuration. GitHub returns a
one-use conversion code; Tickets stores the resulting credentials encrypted in D1.

Follow the installation link and select **All repositories** on your intended
account; repository access and `GITHUB_PR_OWNERS` must both permit ingestion.
`GET /api/v1/github/app` shows registration and installation metadata. Verify PR
linking, status sync and the Tickets check before removing old repository hooks.
The App webhook uses the secret returned by GitHub and encrypted in D1, **not**
`GITHUB_WEBHOOK_SECRET`. Remove the setup state when done.

## 8. Configure GitHub Actions deployment

Create a `production` GitHub environment in your fork and restrict deployments
to your intended branch. The checked-in workflow deploys every push to `main`
after `check`, `migrations` and `secrets` pass. PRs do not deploy. Set these in
that environment:

| Name | GitHub kind | Mapping |
| --- | --- | --- |
| `CLOUDFLARE_API_TOKEN` | Secret | Account-scoped deployment token |
| `CLOUDFLARE_ACCOUNT_ID` | Variable | Your Cloudflare account ID |
| `TICKETS_BOOTSTRAP_USERNAME` | Secret | `BOOTSTRAP_USERNAME` |
| `TICKETS_BOOTSTRAP_PASSWORD_HASH` | Secret | `BOOTSTRAP_PASSWORD_HASH` |
| `TICKETS_GITHUB_WEBHOOK_SECRET` | Secret, legacy integration only | `GITHUB_WEBHOOK_SECRET` |
| `TICKETS_GITHUB_APP_STORE_KEY` | Secret, App integration | `GITHUB_APP_STORE_KEY` |
| `TICKETS_GITHUB_APP_SETUP_STATE` | Secret, App registration only | `GITHUB_APP_SETUP_STATE` |

For the custom-domain deployment, the documented token scopes are Workers
Scripts Write, D1 Write and Account Settings Read on **your account**; Workers
Routes Write, Zone Read and DNS Write on **your zone only**. Adjust zone access
to match your domain setup; workers.dev does not need a custom-domain zone.
Do not reuse the maintainer's account or token.

CI uploads all five Worker secret names through a temporary secrets file; unused
optional GitHub secrets resolve to empty values. Copy any manually configured
secrets into the corresponding GitHub environment secrets before enabling CI,
and keep them consistent. CI applies remote D1 migrations, builds, deploys and
smoke-tests the configured hostname. Update its environment URL and both smoke
URLs to your origin before the first push to `main`. The separate D1 usage
workflow also needs review; see the table below.

## 9. Cloudflare Access (optional)

Tickets has its own login and bearer API tokens; Cloudflare Access is not
required. You may put Access in front of UI navigation, but bypass
`/api/v1/github/*` for webhooks and the manifest callback, and bypass bearer-token
API paths so agents can reach the Worker. An Access login challenge or Access-only
authentication will break those clients. A straightforward path arrangement is
to bypass `/api/*` at Access and let Tickets enforce its own API authentication.
This also leaves `/api/health` public as designed. Keep HTTPS and the Worker's
own authentication; Access does not replace it.

## What you must change when forking

This is a checklist for your fork, not changes applied by this documentation PR.
Some values are deploy settings; others need a follow-up source change if you
want fully independent branding and links. Historical fixtures can stay as demo
examples; do not mistake their repository names for runtime defaults.

| Value | File(s) | What to do |
| --- | --- | --- |
| Production route `tickets.worldwidewebb.co` | `wrangler.toml` | Set your hostname, or remove routes and enable workers.dev |
| Production D1 `database_id` | `wrangler.toml` | Replace with your newly created database ID; keep the local zero placeholder local |
| Worker names `tickets` / `tickets-production`, D1 name `tickets` | `wrangler.toml` | Choose account-appropriate names; retain binding names and `production` environment |
| `GITHUB_PR_OWNERS = "0x63616c"` | `wrangler.toml` | Replace in default and production vars with your owner allowlist |
| Production environment URL and health/API smoke URLs | `.github/workflows/ci.yml` | Point all at your origin before enabling main-branch deployment |
| Account-wide 3 million daily D1 rows-read alert | `.github/workflows/d1-usage.yml`, `xtask/src/d1_usage.rs` | Uses your production account variable and `CLOUDFLARE_API_TOKEN`; add Account Analytics Read for this GraphQL check, tune its threshold or disable it. It has no hardcoded database ID |
| Header GitHub link `0x63616c/tickets` | `crates/web/src/components/shell.rs` | Change the hardcoded repository link in the fixture shell; it is not project configuration |
| `waiting-on-calum` seed and pickup default | `migrations/0013_pickup_rules.sql`, `crates/core/src/pickup.rs` | Choose your approval label and pickup exclusions together. For existing databases use a new migration/settings update, never edit an applied migration |
| Mission Control approval label / names | `crates/web/src/pages/mission.rs`, `crates/worker/src/mission.rs`, `crates/core/src/mission.rs` | UI/SQL match `waiting-on-calum`; the UI heading is `Needs Calum`, and core exposes `waiting_on_calum` and accepts report field `needs_calum`. Coordinate a rename across API, clients, fixtures and tests |
| App display names `Tickets by World Wide Webb` / `Tickets WWW` | `crates/core/src/github_app.rs` | Change manifest branding before registering your App |
| GitHub HTTP User-Agent `tickets-worldwidewebb` | `crates/worker/src/github_app.rs`, `crates/worker/src/github_app_webhook.rs` | Optional branding change; retain a valid GitHub User-Agent |
| `0x63616c/quantum` → `0x63616c/voxelite` identity alias | `crates/core/src/github_mirror.rs` | Retain only if importing that history; define explicit aliases for your own renames without changing existing external IDs |
| Ticket URL convention `tickets.worldwidewebb.co/t/KEY-N` | `AGENTS.md`, `CONTRIBUTING.md`, `.github/pull_request_template.md`, `.github/workflows/pr-conventions.yml`, `xtask/src/github_pr_check.rs` | Set your instance convention and check together. The xtask file is a webhook test fixture |
| Hardcoded ticket links in Tickets check output | `crates/core/src/github_app_checks.rs` | Change the runtime check-output origin and matching tests; no origin setting currently exists |
| `0x63616c` import/check examples and fixtures | `xtask/src/import_check.rs`, `xtask/src/import_github.rs`, `xtask/src/api_smoke.rs`, `xtask/src/mirror_check.rs`, `xtask/src/polish.rs` | Adapt tests if changing identity/allowlist behavior. Import CLI requires `--repo` and `--project`, defaults to loopback URL and has no maintainer repo default |
| Demo GitHub issue/project links and reports | `crates/web/src/pages/detail.rs`, `crates/web/src/pages/tickets.rs`, `crates/web/src/fixtures.rs` | Optional demo branding cleanup; these are fixture views/data, not imported production records |
| Upstream repository badges, clone URL and private reporting link | `README.md`, `SECURITY.md` | Point fork-specific badges, clone instructions and vulnerability reports at your repository |
| Maintainer workflow/project keys and operational examples | `AGENTS.md`, `CONTRIBUTING.md`, `docs/DOGFOOD.md`, `docs/DEPLOY.md`, `docs/DEVELOPMENT.md`, `docs/API.md` | Adapt governance, keys and example origins to your fork; preserve historical provenance when relevant |

## Troubleshooting

- **401 from the API:** send `Authorization: Bearer <personal-token>`; tokens
  come from Settings → API tokens, not the Cloudflare deploy token. Invalid
  Authorization headers never fall back to a valid cookie. Check revocation and
  session expiry. Cookie writes need a matching Origin (otherwise 403). Access
  redirects/HTML mean the request may not have reached Tickets.
- **Login fails:** verify the production bootstrap secrets, hash format and literal
  `$` characters. Use the same HTTPS hostname for UI and API. More than five login
  attempts per source IP in 15 minutes return 429; wait for the window to expire.
- **Missing tables / migration failures:** confirm `DB`, the production ID and
  `migrations_dir`. `cargo xtask migrate` changes only local D1;
  `cargo xtask migrate --remote` targets the named production binding. Verify the
  selected account and D1 permissions before retrying. Do not delete remote data
  to fix a migration problem.
- **Webhook signature failure:** distinguish App and legacy endpoints and secrets.
  Verify `X-Hub-Signature-256` over unchanged raw bytes; proxies must not rewrite
  bodies. App credentials must decrypt with the original store key. Missing App
  registration/unreadable credentials returns 503; invalid App signatures return
  401. Confirm your owner allowlist and installation access if valid deliveries
  are ignored rather than linked.
- **App registration unavailable:** set nonempty setup state and a valid base64
  32-byte store key. An existing registration cannot be overwritten/reset through
  an API. Consult [the callback contract](API.md#github-app-registration-phase-1)
  before changing stored registration data.
