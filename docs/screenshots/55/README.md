# Issue 55 — mobile blocked state and property order

Before is the real API-backed UI built from origin/main at
`fe8cfed10751557608716642721acb707fdfb8e9` (#54 merged), in a temporary worktree
under `/tmp`. That worktree was removed after capture. After uses this branch.
No fixture routes or production data were used.

Both phases use isolated local Worker/D1 state and the same seeded project,
four tickets, label assignments, comment, and unresolved dependency: TKT-2
(Todo) blocks TKT-1 (In progress). Routes are `/list?project=1` and
`/ticket?id=1`. Dark scheme, loaded bundled fonts, device scale 1, pointer at
(0,0), scroll at top; desktop is 1440×900 and mobile is 390×844. All eight PNGs
were opened and inspected.

| Route | Before mobile | After mobile | Before desktop | After desktop |
| --- | --- | --- | --- | --- |
| List | [before](list-before-mobile.png) | [after](list-after-mobile.png) | [before](list-before-desktop.png) | [after](list-after-desktop.png) |
| Ticket | [before](ticket-detail-before-mobile.png) | [after](ticket-detail-after-mobile.png) | [before](ticket-detail-before-desktop.png) | [after](ticket-detail-after-desktop.png) |

Desktop is pixel-equivalent for both routes: ImageMagick
`compare -metric AE <before> <after> null:` reports **0 changed pixels** for
both desktop pairs. The mobile list keeps the existing 36px rows and ellipsis
behavior while exposing the icon/text Blocked marker. Detail properties now
follow the title and precede the description, dependencies and activity in DOM
and visual order. The dependency's existing Blocked by marker remains visible
when scrolled into view; it was already visible on the baseline detail route.

Reproduce captures with the existing Rust browser check:

```sh
TICKETS_MOBILE_LAYOUT_ONLY=1 CHROME_BIN=<chromium> cargo xtask contrast-check
TICKETS_BEFORE_ASSETS=<origin-main-worktree>/crates/web/dist TICKETS_MOBILE_LAYOUT_ONLY=1 CHROME_BIN=<chromium> cargo xtask contrast-check
```

After captures assert marker visibility/non-zero geometry, row containment,
viewport overflow, properties before description/activity, and matching DOM
order. CI's `first-run-check` separately exercises these layout assertions at
390×844 with its exclusively UI-created unresolved dependency, starting from
empty local D1. That check passed, along with fmt, host/wasm Clippy and workspace
tests.

Impeccable critique and polish completed with telemetry disabled. Independent
design review: 31/40, no release-blocking findings. Static detector: zero
findings across both detail markup files. Polish retained the approved dark
palette, Inter, label tints, native controls, compact spacing and shared shell.
The visible marker necessarily leaves less title space on mobile; the existing
ellipsis and full title on detail remain available. Browser evidence comes
from the Rust/CDP check and screenshots; no user-visible detector overlay was
available. Static scan results do not establish contrast compliance.
