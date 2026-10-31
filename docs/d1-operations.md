# D1 availability and daily read alert

TIK-88 follows the October 30 quota outage. Calum approved Workers Paid
($5/month); the billing decision is answered. Billing is an owner action, outside
this PR. The 3M early warning remains useful after moving plans.

The Store adapter classifies D1 quota, capacity, overload, timeout, reset and
connection failures as unavailable. Authentication preserves that classification.
API responses use HTTP 503 and `error.code = unavailable`, with a safe message;
ordinary SQL/type/data failures keep existing 500/validation/conflict behavior.
Worker logs contain `D1 failure code=<D1 constant> unavailable=<boolean>`.
The constant is extracted from the binding error (fallback `D1_ERROR`); SQL,
parameters, credentials and raw upstream error text are never logged. See
[Cloudflare's error list](https://developers.cloudflare.com/d1/observability/debug-d1/).
No automatic write retries are added: a failure may have an uncertain commit
outcome, so clients should read back state before retrying a mutation.

`.github/workflows/d1-usage.yml` runs `cargo xtask d1-usage-check` approximately
every 15 minutes on main, or manually with workflow_dispatch. It uses the existing
production environment `CLOUDFLARE_API_TOKEN` secret and `CLOUDFLARE_ACCOUNT_ID`
variable, without provisioning secrets or changing permissions. The GraphQL
`d1AnalyticsAdaptiveGroups.sum.rowsRead` query aggregates **all databases in the
account** for the current UTC calendar day, matching the scope of the Free daily
allowance. It performs no SQL, authentication calls to Tickets, or D1 reads/writes.
See [Cloudflare analytics](https://developers.cloudflare.com/d1/observability/metrics-analytics/).

At **3,000,000 or more rows**, the job fails with a daily read alert. Each run
prints the date and count and saves them in the Actions summary. The failed run is
the alert: enable GitHub Actions failure notifications for this repository in the
operator's GitHub notification settings. There is no new webhook/email secret or
D1-backed alert state. Runs continue failing while over threshold until UTC rollover;
GitHub controls notification delivery/coalescing. Schedule delays and analytics
lag mean this is an early warning, not a guarantee against a fast quota spike.

Missing credentials, HTTP/GraphQL errors, absent/invalid data also fail visibly;
unknown usage is never reported as zero. Empty early-day analytics can therefore
produce a monitoring failure until the first sample arrives. The existing deploy
token's documented scopes do not include Account Analytics Read, so the first run
may require owner review of permissions. Do not silently skip monitoring, grant
permissions, or rotate secrets as part of this job. A failed permissions check
means the usage alert has **not yet been verified operational**.

After merge, inspect the first run, confirm analytics permission and notification
delivery, then record a full UTC day's count in TIK-88. Keep the ticket open using
`Refs`, since that acceptance requires a day after deploy. If the 3M alert fires,
inspect D1 analytics/expensive queries, confirm the paid plan transition with the
owner, and investigate read regressions. Do not automatically change billing.

PR #105 (TIK-71, merge `d7fdb7e`) was verified as an ancestor of `04fa46d`, whose
[production deploy succeeded](https://github.com/0x63616c/tickets/actions/runs/41170626138/job/124450614657)
on 2026-10-31 at 05:16:34 UTC. Local Wrangler was unauthenticated during TIK-88,
so no current daily rows-read value could be recorded from this box.
