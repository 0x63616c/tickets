# TIK-45 UI cleanup evidence

All captures use disposable fixture data in isolated local Worker/D1 state through
`cargo xtask polish-check`. No production records or credentials appear here.

The before UI is commit `40ef5f9` (the main branch when work started). Its assets
were built in a temporary worktree with a build-only binary target named
`tickets-web-tik45-baseline`, to keep the shared Cargo output from being replaced
by another worktree's Wasm. No baseline UI source was changed.

The after UI is this branch. Both phases use the same fixture seed, loaded bundled
fonts, dark color scheme and reduced-motion preference. Desktop and mobile
viewports are 1440×900 and 390×900; list/detail PNGs capture the full page and may
be taller. The extra mobile properties capture shows the expanded panel.

| Surface | Before | After |
| --- | --- | --- |
| Desktop list | [PNG](before-list-desktop.png) | [PNG](after-list-desktop.png) |
| Mobile list | [PNG](before-list-mobile.png) | [PNG](after-list-mobile.png) |
| Desktop detail | [PNG](before-detail-desktop.png) | [PNG](after-detail-desktop.png) |
| Mobile detail | [PNG](before-detail-mobile.png) | [PNG](after-detail-mobile.png) |
| Expanded mobile properties | Included in the old mobile detail | [PNG](after-properties-mobile.png) |

Chrome runs through `/workspace/tickets-polish/chrome-noproxy`, which launches
`/usr/bin/google-chrome` in a private mount namespace with only the managed proxy
policy overridden. Shared policy files are unchanged.

The acceptance fixture covers hidden legacy metadata, field-only priority,
dependency-derived Blocked state and resolution, backlog presentation, canceled
visibility, aligned list columns and tablet containment, same-project references,
model metadata deduplication, mobile properties, UTC timestamps, import provenance,
brief/sustained socket failures and preservation of stored labels/body/status.
