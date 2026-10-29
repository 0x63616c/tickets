# TIK-32 pull request links

`before-desktop.png` / `after-desktop.png` (1440×900) and
`before-mobile.png` / `after-mobile.png` (390×900) capture the real `/ticket`
route in dark mode with loaded fonts and disposable local D1 fixtures.
The mobile view scrolls to the dependencies/PR/activity content.

The before assets were built from base commit
`b1bb53d1621989c896418d0ab806a02163a2c365` in a temporary worktree.
Both captures use the same signed PR payload shape with isolated local D1.
The after images were refreshed after merging TIK-33; the retirement check now
creates an editable imported ticket first, so the PR ticket is TIK-2. The after
capture also includes polished activity status labels. The PR repository is
accepted by its configured owner; no issue-mirror list remains. No fixture-only `/design` route substitutes for the
real PR-link UI. Older `ticket-detail-*` and `kanban-*` images in this directory
are pre-existing evidence and are not the PR-link before/after captures.

Reproduce sequentially, after sourcing `.job.env`:

```sh
TICKETS_BEFORE_ASSETS=/absolute/base/crates/web/dist CHROME_BIN=<headless-chromium> cargo xtask mirror-check
CHROME_BIN=<headless-chromium> cargo xtask mirror-check
```

The test verifies signed ping without a repository, HMAC rejection, owner
allowlisting (including mixed case), unknown keys, key-based ticket reads,
head SHA, redelivery, merge completion, stale delivery and edited-key removal.
SQLite tests additionally cover all status transitions, draft/review behavior,
reference merges, multiple PRs, 100-key ingestion and deletion cascades.


Impeccable 4.8.0 critique used independent design and evidence reviews (31/40,
no blocking findings; detector returned zero findings). Polish preserved the
existing layout, added a 44px mobile PR target, capitalized display states and
humanized activity statuses. Desktop keyboard focus and mobile wrapping/overflow
were verified by the acceptance browser. No overlay or computed contrast audit
is claimed. No fixture routes substitute for these actual ticket captures.
