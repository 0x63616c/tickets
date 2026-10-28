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
3. Set the single-user login secrets through Wrangler's interactive prompts (see
   "Authentication secrets" below for generating the hash):

   ```sh
   wrangler secret put BOOTSTRAP_USERNAME --env production
   wrangler secret put BOOTSTRAP_PASSWORD_HASH --env production
   ```

   Secret creation may create an empty Worker before its first deployment. Never
   put values in TOML, command arguments, committed files, or PRs. Day one has one
   username/password login and no sign-up.
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

## Authentication secrets
Production deployment requires Calum's explicit authorization. This document does
not authorize deployment or remote database provisioning.

Set `BOOTSTRAP_USERNAME` and `BOOTSTRAP_PASSWORD_HASH` as Worker secrets through
an approved secure workflow. Generate a unique passphrase with a password manager;
`cargo xtask auth-hash` accepts it on stdin, refuses fewer than 16 characters, and
outputs a randomly salted PBKDF2-SHA256 hash at 100,000 iterations. Avoid command
arguments and shell history. Never commit the password or hash, or put the hash in
D1: the Worker secret is authoritative. Migration 0004 drops the unused D1
password column, so D1 holds no password material.

The hash stores its iteration count; verification accepts canonical positive counts
up to the runtime maximum of 100,000. Future runtime changes can raise that maximum
and the generation default without invalidating existing hashes. Regenerate the
secret to upgrade its work factor. Secret rotation changes future logins; existing
sessions remain valid until logout or seven-day expiry.

Keep the five-attempts-per-IP-per-15-minutes limiter and constant-time comparisons.
See [ADR 0003](adr/0003-auth.md) for the accepted tradeoff against OWASP's 600,000
recommendation and the offline-guessing risk if a secret leaks. Verify production
CPU telemetry during an authorized deployment; local wall time is not CPU evidence.
