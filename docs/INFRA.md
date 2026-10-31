# Cloudflare infrastructure

Tickets owns the `tickets-production` Worker and its bindings and custom domain.
`wrangler.toml` is the desired state; normal CI deployment applies it. Never deploy,
run remote migrations, change billing, or call write APIs by hand without Calum's
explicit approval. The zone `worldwidewebb.co` is shared with other projects;
Tickets must not manage their records or zone-wide settings.

## Audit and ownership

Read-only live audit: **2026-10-31**. Workers Logs was OFF (observability absent).
The configuration now enables invocation logs and 100% head sampling explicitly
at the top level and in `env.production`; the next normal deployment applies this.
The other audited settings matched the values below.

| Resource / setting | Where it's defined | Codified? | Drift-checked? |
| --- | --- | --- | --- |
| Worker `tickets-production`, fetch + scheduled handlers | `wrangler.toml`; `crates/worker/src/lib.rs` | Yes | Script existence; handlers exercised by application checks |
| Compatibility date 2026-10-27, no flags; standard usage model | `wrangler.toml`; `infra.rs` standard-model expectation (Wrangler 4 has no usage_model key) | Yes; model automatic in Wrangler 4 | Yes |
| Workers Logs enabled, invocation logs enabled, both sampling rates 1 | `wrangler.toml` | Yes | All five observability fields |
| Logpush false, no tail consumers, default placement | `wrangler.toml` (`placement.mode = "off"`) | Yes | Yes |
| workers.dev and preview URLs disabled | `wrangler.toml` | Yes | Yes |
| ASSETS binding and asset routing | `wrangler.toml` | Yes | Binding name/type; routing behavior covered by app checks |
| DB / D1 `tickets` (`aac62c45-1f61-4858-956f-c0e4556b52ab`) | `wrangler.toml`; `migrations/` | Binding/schema yes; initial database provisioning requires owner approval | Binding name/type/database ID |
| BOARD_ROOMS / BoardRoom, SQLite namespace | `wrangler.toml` binding + `v1-board-room` migration | Yes | Binding, namespace ID/class/script, SQLite backend |
| GITHUB_PR_OWNERS plain variable | `wrangler.toml` | Yes | Name/type/value (value withheld from report) |
| Five Worker secret names | `xtask/src/infra.rs` `SECRETS`; deploy uploads in `ci.yml` | Yes, names only; test enforces agreement with deploy | Exact binding set; extra/missing secrets fail |
| Cron every minute | `wrangler.toml` | Yes | Exact schedule set |
| Custom domain `tickets.worldwidewebb.co`, enabled | `wrangler.toml` production routes | Yes | Exact domain set for script, hostname/enabled |
| DNS AAAA `100::` for custom domain | Cloudflare creates it from Wrangler custom domain | Yes, indirectly | Exactly one record, read-only, matching custom-domain `origin_worker_id`; extras fail |
| No Worker routes | Custom-domain-only production routes | Yes | No zone routes targeting this script |
| No Tickets custom WAF/rules | No custom rules declared; login limiter is Worker code | Absence is policy | Non-managed rules referencing hostname fail; optional WAF permission |
| No KV, queues, R2 bindings | No bindings in `wrangler.toml` | Yes | Exact binding set rejects additions; unrelated account resources excluded |
| Workers Paid plan | Owner's Cloudflare billing | Manual | No (audit token cannot read billing) |
| API credentials, account ID, GitHub environment controls | GitHub `production` environment / Cloudflare | Manual | Workflow needs credentials; values/protections excluded |
| Shared zone defaults and other projects' DNS | Owned elsewhere (some Pulumi-managed) | Outside Tickets | Only Tickets hostname DNS and explicit host references inspected |

The audit found only Cloudflare-managed/default normalization, managed free,
DDoS L7 and response firewall rulesets; no custom Tickets rules. The check reads
non-managed ruleset details because list responses do not contain expressions.
It ignores managed rules and rules for other hosts. This literal hostname check
is not a full evaluator: indirect matches through lists, wildcards or zone-wide
expressions remain part of the shared zone owner's responsibility.

## Why Wrangler and xtask, rather than Terraform

Wrangler manages every deployable resource Tickets owns, including the custom
domain and its automatic DNS record. Terraform would introduce another tool and
a state backend without adding resources to manage. Rust xtask supplies the
missing read-only drift detection, using the same API-client pattern as
`d1-usage-check`. D1's existing identity is recorded in TOML; provisioning a new
database is a separate, explicitly authorized owner operation.

## Running the check

Provide `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` through your shell's
secure environment mechanism, then run:

```sh
cargo xtask infra-check
```

The command only sends GET requests. It parses production settings from TOML,
compares live state, prints a Markdown table, and appends it to
`GITHUB_STEP_SUMMARY` when set. Any DRIFT or unreadable essential endpoint exits
non-zero. An optional ruleset endpoint returning HTTP 403 prints
`WARN rulesets skipped: token lacks Zone WAF Read (HTTP 403)` and does not fail.
Other API errors still fail. Token and secret values are never printed, and
upstream error bodies are withheld. Fixtures run without credentials:

```sh
cargo test -p xtask infra::tests
```

`.github/workflows/infra-drift.yml` runs in the `production` environment after a
successful **CI** push workflow on this repository's `main`, every six hours
at minute 19 UTC, and on manual dispatch. Post-deploy checks use that deployed
commit; scheduled/manual checks use `main`. It never consumes workflow-run
artifacts or PR code. A failed workflow is the alert channel; inspect its table/step summary,
then correct TOML or restore state through the normal reviewed deploy process.
The existing deploy workflow is unchanged.

## Can't be codified / manual

- **Workers Paid billing plan:** owner action in Cloudflare's dashboard. The audit
  token cannot read billing. Agents must not change plans.
- **Secret values:** live in GitHub's `production` environment, uploaded by CI via
  an ephemeral secrets file. The five Worker names are declared in `infra.rs`;
  the environment names add `TICKETS_`. Update through GitHub's encrypted-secret
  interface, then run the latest approved `main` deploy. Never commit values or
  place them in command arguments. See [authentication secrets](DEPLOY.md#authentication-secrets)
  and the [GitHub App setup flow](API.md#github-app-registration-phase-1).
  `TICKETS_BOOTSTRAP_USERNAME` sets the login name; rotate it by updating that
  secret. Generate `TICKETS_BOOTSTRAP_PASSWORD_HASH` with `cargo xtask auth-hash`
  from a password-manager passphrase; rotation affects future logins, not existing
  sessions. `TICKETS_GITHUB_WEBHOOK_SECRET` must match the legacy GitHub webhook's
  signing secret: generate a random value and coordinate both ends when rotating.
  `TICKETS_GITHUB_APP_SETUP_STATE` is an unguessable random setup authorization
  value; rotate it before starting a new registration, avoiding in-flight callbacks.
  `TICKETS_GITHUB_APP_STORE_KEY` is standard base64 of 32 random bytes and encrypts
  stored GitHub App credentials. Replacing it alone makes existing encrypted data
  unreadable: coordinate re-encryption or an authorized App reconnection during
  rotation; it is not an ordinary login-secret rotation.
- **Cloudflare API token:** create/rotate `tickets-github-deploy` in Cloudflare and
  update GitHub environment secret `CLOUDFLARE_API_TOKEN`. Existing deploy scope:
  Workers Scripts Write, D1 Write, Account Settings Read on the account; Workers
  Routes Write, Zone Read and DNS Write on **worldwidewebb.co only**. Those write
  permissions are for deployment; this checker uses only reads (Workers Scripts,
  Workers Routes, Zone and DNS read access, including DO namespace metadata).
  Optional **Zone WAF Read** on that zone enables ruleset inspection. No billing
  permission is needed. A separately scoped check token may use the corresponding
  read permissions. Keep tokens out of files committed to the repo and logs.
- **Account identity:** GitHub environment variable `CLOUDFLARE_ACCOUNT_ID` must
  identify the account holding the Worker and zone. Update it explicitly when
  moving accounts, together with resource IDs and token scope.
- **GitHub environment protection rules:** owner configures `production`, allowed
  deployment branches (`main`), reviewers and secret access in GitHub settings.
  The drift workflow uses this same environment and obeys its protection rules.
- **Shared zone:** worldwidewebb.co's Free plan, zone-wide settings, certificates,
  managed defaults and unrelated DNS belong to its owners/other projects. Tickets
  does not take ownership or modify them.

Workers Paid currently includes **20 million log lines per month** and **7-day
retention**; additional logs cost $0.60 per million. Full sampling includes every
invocation and emitted application log, so monitor volume. Cloudflare announces a
pricing change on December 1, 2026; recheck the
[Workers Logs pricing](https://developers.cloudflare.com/workers/observability/logs/workers-logs/#pricing)
before budgeting beyond that date.
