# TIK-95 loading surfaces

Baseline: `c8d4494849ccf02daebe492f57dcd973d57cff68`, built in this issue's
worktree before changing any frontend assets. Both phases use the same immutable
fixture API data on real application routes, with responses held by `loading-check`.
No production data or credentials appear in these images.

`before-<route>-<width>.png` / `after-<route>-<width>.png` pair dark desktop
(1440×900) and mobile (390×844) loading states with fonts loaded. Routes cover
Home, Analytics, Board, List, ticket detail, Projects, Mission Control and Settings
(Account including GitHub, API tokens, Projects and Pickup rules). Appearance
has no asynchronous initial placeholder. All use the shared AsciiSkeleton /
LoadingIndicator; search, label saving, refresh indicators and BusyButton use the
same palette and transparent glyph hosts.

The existing card shell uses `--surface`, matching a loaded board card and
Analytics card. Other hosts expose the page's `--bg`, popup or button surface.
The package's block-only `currentColor` tint is removed. Static glyph opacity is
0.12 and the scan peak is 0.22; readable loading captions retain `--text-muted`.
The published ascii-reveal 0.3.0 package and lifecycle wrappers are unchanged.

`after-board-reduced-os.png` and `after-board-reduced-app.png` show static loading
with OS reduced motion and the app override. The check verifies no canvas remains
and the held skeleton DOM stays unchanged. `after-search.png` covers the shared
search placeholder.

`cold/` contains paired Home screenshots and geometry diagnostics from uncached
1280×800 and 390×844 navigation: 150ms latency, 200KB/s download, 75KB/s upload,
4× CPU slowdown, held API responses, decode and settlement. This existing check
also measures Board, List, ticket detail and Projects through every phase.

Reproduce after building, from the worktree root:

```sh
source .job.env
export PATH=$HOME/.cargo/bin:$PATH
TICKETS_LOADING_SHOTS=docs/screenshots/TIK-95 CHROME_BIN=google-chrome cargo xtask loading-check
```

For before captures add `TICKETS_BEFORE_ASSETS=<unchanged-base-dist>`.

## Verification and review

Passed `cargo xtask ci` (format, workspace build, host and wasm32 clippy with
`-D warnings`, workspace tests, tokens and lint-policy tests), `npm run lint:css`,
`npm run test:lint`, `first-run-check`, `loading-check`, `home-loading-check`,
`board-loading-check`, `home-shot` and 2× `component-shot skeletons` / `busy-button`.
The token baseline remains exactly 356 findings (311 CSS + 45 Rust), with no
allowlist change. All ten after cold-load cases have zero overflow frames.

Browser checks used a private verified xtask copy, as documented in CONTRIBUTING,
and a child-only mount namespace masking this box's managed Chrome proxy policy.
Worker checks used a separate loopback network namespace to avoid shared-port
collisions. No shared Chrome policy or production resources were changed.

`components/` contains tightly cropped 2× before/after pairs from the same
`/design/c/skeletons` and `/design/c/busy-button` fixture routes. Baseline assets
were placed in the local build output slot only during these isolated captures,
then the completed fixed build was restored. This avoids the generic historical
`--before-assets` fallback's different gallery framing.

Impeccable critique used independent design and source-detector assessments.
The design review identified the extra inset fill and over-bright sweep;
both are addressed by the shared CSS. The source detector reported zero findings.
The polish pass reviewed actual desktop/mobile route captures and component
pairs: loading captions remain readable, surfaces and geometry stay consistent,
and glyphs remain visible but quiet. No scoped blockers remain. No browser
inspection overlay was used; computed-style assertions and CDP captures supply
rendered evidence. Startup splash, board drag code, keyboard shortcuts and
live/mission implementation files have no changes in this PR.

## Route pairs

| Surface | Desktop before / after | Mobile before / after |
| --- | --- | --- |
| Home | [Before](before-home-1440.png) / [After](after-home-1440.png) | [Before](before-home-390.png) / [After](after-home-390.png) |
| Analytics | [Before](before-analytics-1440.png) / [After](after-analytics-1440.png) | [Before](before-analytics-390.png) / [After](after-analytics-390.png) |
| Board | [Before](before-board-1440.png) / [After](after-board-1440.png) | [Before](before-board-390.png) / [After](after-board-390.png) |
| List | [Before](before-list-1440.png) / [After](after-list-1440.png) | [Before](before-list-390.png) / [After](after-list-390.png) |
| Ticket detail | [Before](before-detail-1440.png) / [After](after-detail-1440.png) | [Before](before-detail-390.png) / [After](after-detail-390.png) |
| Projects | [Before](before-projects-1440.png) / [After](after-projects-1440.png) | [Before](before-projects-390.png) / [After](after-projects-390.png) |
| Mission Control | [Before](before-mission-1440.png) / [After](after-mission-1440.png) | [Before](before-mission-390.png) / [After](after-mission-390.png) |
| Settings: Account and GitHub | [Before](before-account-1440.png) / [After](after-account-1440.png) | [Before](before-account-390.png) / [After](after-account-390.png) |
| Settings: API tokens | [Before](before-tokens-1440.png) / [After](after-tokens-1440.png) | [Before](before-tokens-390.png) / [After](after-tokens-390.png) |
| Settings: Projects | [Before](before-project-settings-1440.png) / [After](after-project-settings-1440.png) | [Before](before-project-settings-390.png) / [After](after-project-settings-390.png) |
| Settings: Pickup rules | [Before](before-pickup-settings-1440.png) / [After](after-pickup-settings-1440.png) | [Before](before-pickup-settings-390.png) / [After](after-pickup-settings-390.png) |
