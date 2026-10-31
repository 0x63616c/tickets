# Production setup and deployment

Production runs at `https://tickets.worldwidewebb.co` (Worker `tickets-production`,
named `production` environment in `wrangler.toml`). Its workers.dev endpoint and
preview URLs are disabled. Calum approved moving to Workers Paid ($5/month)
for TIK-88; the owner handles billing, and agents must not change plans. Calum
approved the production deploy and redeploy-on-merge on 2026-10-28.

## Continuous deployment

Every push to `main` runs the `deploy` job in `.github/workflows/ci.yml` after the
`check`, `migrations` and `secrets` jobs pass. It runs in the `production` GitHub
environment (deployments restricted to `main`) and:

1. applies pending D1 migrations with `cargo xtask migrate --remote`;
2. installs the locked npm dependency with `npm ci --prefix crates/web`, then
   builds the Leptos assets and Worker with `cargo xtask build` (which also ensures
   that install for local callers);
3. runs `wrangler deploy --env production`, uploading the login secrets from a
   temporary 0600 file that is deleted afterwards;
4. smoke-tests `/api/health` and checks that unauthenticated `/api/v1` returns 401.

Pull requests never deploy and never see the production secrets.

### Environment configuration (`production` GitHub environment)

| Name | Kind | Purpose |
| --- | --- | --- |
| `CLOUDFLARE_API_TOKEN` | secret | Account token `tickets-github-deploy`: Workers Scripts Write, D1 Write, Account Settings Read on the account; Workers Routes Write, Zone Read, DNS Write on `worldwidewebb.co` only. |
| `CLOUDFLARE_ACCOUNT_ID` | variable | Cloudflare account ID. |
| `TICKETS_BOOTSTRAP_USERNAME` | secret | Single-user login name. |
| `TICKETS_BOOTSTRAP_PASSWORD_HASH` | secret | Output of `cargo xtask auth-hash` (see below). |

Rotate the password by generating a new hash and updating the environment secret,
then re-running the latest `main` deploy. Never put values in TOML, command
arguments, committed files, or PRs.

## Migrations

`cargo xtask migrate` applies migrations to local D1 (CI and development).
`cargo xtask migrate --remote` applies them to the production database and needs
Cloudflare credentials in the environment; normally only the deploy job runs it.
Migrations must be backwards compatible with the previously deployed Worker,
because they are applied before the new Worker goes live.

## Manual validation

```sh
cargo xtask build
wrangler deploy --dry-run --env production --outdir /tmp/tickets-dry
```

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
Cloudflare documentation before setup as limits can change. The following Free
limits describe the original deployment; confirm the current plan with the owner.
See [D1 operations](d1-operations.md) for the 3M daily read alert.

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

The Worker reads `BOOTSTRAP_USERNAME` and `BOOTSTRAP_PASSWORD_HASH` secrets, set by
the deploy job from the `production` environment. Generate a unique passphrase with a password manager;
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
