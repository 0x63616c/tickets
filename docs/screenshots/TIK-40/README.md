# TIK-40 sidebar and Settings evidence

Baseline: `01f92c0df23b7c1f5bc3cab773c0ddef93fc5b8d` (TIK-38 on main),
built in a detached temporary worktree. All captures use fixture data, dark
appearance and loaded bundled fonts. No personal account data or API secrets.

- `before-sidebar.png` and `after-sidebar.png`: isolated sidebar at 2× DPR.
- `before-user-menu.png`: the actual previous header; it had no user menu.
  `after-user-menu.png`: the new shared dropdown open, fixture account name.
- `before-settings-*.png`: the previous `/settings/tokens` entry point. Account,
  Projects & GitHub, Appearance, and Settings sub-navigation did not exist.
- `after-settings-*.png`: each new Settings section, full page at 1440×900;
  `*-settings-mobile.png` uses 390×844. `*-section-*.png` crops the content.
- `*-shell.png`: full-page fixture board and sidebar.

Reproduce with `cargo xtask build`, then `component-shot sidebar`,
`component-shot user-menu` (`--output docs/screenshots/TIK-40`), and
`cargo xtask settings-shot`. Set `TICKETS_BEFORE_ASSETS=<base dist>` for
`settings-shot`, or `--before-assets <base dist>` for component baselines.
On this managed box use `CHROME_BIN=/workspace/tickets-polish/chrome-noproxy`.

Functional verification is separate: `cargo xtask settings-check` uses isolated
local D1 and validates account persistence, menu focus, token redirect/lifecycle,
collapsed sections, appearance preferences, shortcuts and mobile navigation.
