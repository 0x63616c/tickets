# Production setup and deployment

**Deploying requires Calum's explicit OK. Do not run the remote setup or deployment
commands below without authorization.** CI never deploys: it builds, checks Rust,
scans for secrets, and applies migrations to local D1 only.

Production uses the named `production` environment in `wrangler.toml` and the
custom domain `tickets.worldwidewebb.co`. Its workers.dev endpoint and preview
URLs are disabled. Keep the account on Workers Free; Wrangler configuration does
not enforce an account billing plan. Do not enable paid services.

## One-time setup (authorized operator only)

1. Install the pinned tools in [DEVELOPMENT.md](DEVELOPMENT.md). The
   `worldwidewebb.co` DNS zone must be active on Cloudflare in the same account as
   the Worker. Resolve any conflicting DNS record for `tickets.worldwidewebb.co`
   before attaching the custom domain; Cloudflare manages its DNS and certificate.
2. Authenticate and create the production database:

   ```sh
   wrangler login
   wrangler d1 create tickets
   ```

   Replace the all-zero `database_id` under `[[env.production.d1_databases]]`
   with the returned ID. Leave the top-level local placeholder alone. The binding
   must remain `DB`, with `migrations_dir = "migrations"`.
3. Once authentication is implemented, enter the bootstrap password and a strong,
   independently generated session secret through Wrangler's interactive prompts:

   ```sh
   wrangler secret put BOOTSTRAP_PASSWORD --env production
   wrangler secret put SESSION_SECRET --env production
   ```

   These are reserved secret names for the future authentication implementation;
   the current scaffold does not consume them or provide login. Confirm the names
   against that implementation before setup. Secret creation may create an empty
   Worker before its first deployment. Never put values in TOML, command arguments,
   committed files, or PRs. Day one has one username/password login and no sign-up.
4. Apply versioned migrations to the production database:

   ```sh
   wrangler d1 migrations apply DB --remote --env production
   ```

   Review pending SQL first. `cargo xtask migrate` intentionally remains local
   only; never change it to migrate production implicitly.

## Build, validate, deploy

From the repository root, build both the Leptos assets and Worker, then validate
the production configuration without uploading or creating remote resources:

```sh
cargo xtask build
wrangler deploy --dry-run --env production --outdir /tmp/tickets-dry
```

**Only after Calum's explicit OK**, with the real database ID, secrets and remote
migrations in place, the operator may run:

```sh
wrangler deploy --env production
```

After an authorized deployment, verify `https://tickets.worldwidewebb.co/api/health`
returns the expected JSON and the root serves the Leptos application. No production
deployment or remote provisioning was performed as part of configuring this repo.

## Live updates (#16)

The Worker does not yet export a Durable Object class, so no active DO binding or
class migration is configured. In #16, export the Rust class, add its binding under
`env.production.durable_objects.bindings` (and the local binding for development),
and introduce a uniquely tagged `[[env.production.migrations]]` entry using
`new_sqlite_classes = ["<exported class name>"]`. Use SQLite storage and WebSocket
hibernation to stay within Free allowances; do not use `new_classes` for the legacy
key-value backend. This is separate from D1 SQL migrations.

## Free-tier limits worth knowing

These are account allowances, shared with other workloads; check the linked
Cloudflare documentation before setup as limits can change. Stay on Free so
exceeding allowances fails operations rather than enabling paid overages.

| Service | Free allowance / limit |
| --- | --- |
| Workers | 100,000 dynamic requests/day; 10 ms CPU per invocation. Static asset requests are free and unlimited. |
| D1 | 500 MB per database, 5 GB total account storage, 10 databases; 5 million rows read/day and 100,000 rows written/day. |
| SQLite Durable Objects | 100,000 requests/day, 13,000 GB-s duration/day; 5 million rows read/day, 100,000 rows written/day, 5 GB total SQL storage; 100 classes per account. |

Sources: [Workers limits](https://developers.cloudflare.com/workers/platform/limits/),
[Workers pricing](https://developers.cloudflare.com/workers/platform/pricing/),
[D1 limits](https://developers.cloudflare.com/d1/platform/limits/),
[D1 pricing](https://developers.cloudflare.com/d1/platform/pricing/),
[DO pricing](https://developers.cloudflare.com/durable-objects/platform/pricing/),
[DO limits](https://developers.cloudflare.com/durable-objects/platform/limits/).
Daily allowances reset at midnight UTC. DO requests include WebSocket connections,
incoming messages (metered at 20:1), and alarms; hibernation avoids idle duration
usage when eligible.
