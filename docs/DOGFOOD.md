# Dogfooding

Tickets tracks its own build work inside Tickets, in the **Tickets** project
(key `TIK`) on <https://tickets.worldwidewebb.co>, linked to `0x63616c/tickets`.

## How it is seeded

The project is seeded from this repository's GitHub issues (open and closed) with
the GitHub importer described in [DEVELOPMENT.md](DEVELOPMENT.md#github-import):

```sh
# TICKETS_TOKEN: a personal API token from Settings → API tokens, set in the
# environment only (never pass it as an argument or print it).
cargo xtask import-github --repo 0x63616c/tickets --project TIK \
  --url https://tickets.worldwidewebb.co --allow-remote --dry-run
cargo xtask import-github --repo 0x63616c/tickets --project TIK \
  --url https://tickets.worldwidewebb.co --allow-remote
```

Each GitHub issue `#N` becomes ticket `TIK-…` with external ID
`github:0x63616c/tickets:N` and a link back to GitHub. Labels, `Blocked by #N`
dependencies, open/closed state and comments come across. The run ends with a
reconciliation report and exits non-zero on any gap.

## Keeping it current

Tickets is the sole system of record as of 2026-10-29; GitHub Issues is retired.
Re-run the import command after issues change: reruns update existing tickets and
never duplicate them. Test changes to the importer locally first with
`cargo xtask import-check`. Never run two imports at the same time.

New work is filed directly in Tickets. Reruns are deliberate migrations that can
overwrite native edits; review a dry run before applying them.

## First production seed

28 October 2026: GitHub open/closed 5/17, Tickets 5/17, labels 6/6,
dependencies 24/24, comments 1/1, gaps 0. A second run changed nothing.

## Maintainer-instance operations

Tickets became the sole system of record on 2026-10-29. GitHub Issues is retired;
imported tickets remain editable with GitHub provenance and comment markers.
Project keys remain QUA, TIK, APP (apps), WWW (world-wide-webb), and BOX
(grok-box-config). The maintainer's instance runs at `tickets.worldwidewebb.co`;
merges to `main` deploy there automatically behind login. The current deployment
uses Workers Paid; the original README's Free-tier target is historical.

For an existing project, preview a one-shot import with:

```sh
cargo xtask import-github --repo 0x63616c/world-wide-webb --project WWW --dry-run
```

Open issues are selected by default. `--state all|open|closed`, `--since YYYY-MM-DD`,
`--exclude N,M` and `--close-github` support migration; see
[the importer guide](DEVELOPMENT.md#github-import). Production imports require
explicit maintainer approval. Issue events are ignored and issue reconciliation
no longer runs; signed webhooks remain for pull-request integration.

Register the GitHub App through the [manifest flow](API.md#github-app-registration-phase-1),
then select **All repositories** during installation. Keep `GITHUB_PR_OWNERS`
configured, use `/api/v1/github/app/webhook`, and grant Checks write and Pull
requests read (the generated manifest requests additional permissions documented
in the self-hosting guide). The App replaces per-repository hooks, syncs PR
status and posts Tickets checks with linked ticket details. Ticket pages show
current-head Tickets and CI results. Remove old hooks only after verifying the
installation; the legacy endpoint remains available during transition.
`GET /api/v1/github/app` reports installed repositories without secrets.

Historical v1 verification is in [V1-VERIFICATION.md](V1-VERIFICATION.md); the
original v1 spec was GitHub issue #1.
