# Dogfooding

Tickets tracks its own build work inside Tickets, in the **Tickets** project
(key `TIK`) on <https://tickets.worldwidewebb.co>, linked to `0x63616c/tickets`.

## How it is seeded

The project is seeded from this repository's GitHub issues (open and closed) with
the GitHub importer described in [DEVELOPMENT.md](DEVELOPMENT.md#github-issue-import):

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

Moving the source of truth from GitHub issues to Tickets (filing new work directly
in `TIK`) is a later step, decided by Calum.

## First production seed

28 October 2026: GitHub open/closed 5/17, Tickets 5/17, labels 6/6,
dependencies 24/24, comments 1/1, gaps 0. A second run changed nothing.
