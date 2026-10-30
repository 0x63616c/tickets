# TIK-52 — ticket topbar actions

Before: actual assets built from `8bc5d99bacba936de87d2531c49993decfc4ec3f`.
After: this branch's implementation. Both render `/t/TIK-52` with the same
intercepted sample API responses (no production data), loaded bundled fonts,
dark scheme, and 2× device scale at 1440×900 and 390×900 viewports.

- `before/after-topbar-actions.png`: tightly cropped desktop actions.
- `before/after-topbar-1440.png`: complete desktop topbar.
- `before/after-topbar-390.png`: complete mobile topbar (shortcut intentionally
  hidden by the existing mobile rule).

The shared topbar buttons use 32px height, 12px horizontal padding and 8px gaps.
Edit uses the existing secondary treatment. The shortcut keeps its 20px box,
4px internal padding and vertical centering, with 12px clearance to the button
border. The enclosing 44px topbar and existing responsive behavior are retained.

A single scoped Impeccable critique/polish pass found the ghost Edit treatment
and redundant shortcut margin to be the relevant consistency issues; both are
resolved. The mechanical scan of `api_detail.rs` returned no findings.

Local after-build verification uses a temporary unique Cargo binary name inside
this workspace's shared target directory to prevent concurrent worktrees from
overwriting the Wasm output. The manifest is restored before commit; application
source and screenshot fixture data are unchanged by that build isolation.
