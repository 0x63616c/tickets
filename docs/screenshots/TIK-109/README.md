# TIK-109 visual evidence

Dark-only, rendered PNGs at 2× DPR, after fonts load. Desktop is 1440×900;
mobile is 390×844. Crops cover board cards, list rows, ticket details and the
shared create/edit classification controls; `*-page.png` captures their pages.
All data is local fixtures. No production access is used.

Before assets were built from commit `339e204` in an isolated temporary worktree.
After assets are from TIK-109. Because concurrent worktrees were replacing the
shared release artifacts, the final verification used a temporary `tik109` Cargo
profile inheriting `release`, supplied through process-local Cargo configuration.
It kept the existing shared target directory and changed no repository build
settings. The normal `cargo xtask build` was also run.

Reproduce from the repository root after building:

```sh
CARGO_BUILD_JOBS=2 cargo xtask component-shot ticket-type --before-assets <baseline-dist> --output docs/screenshots/TIK-109
CARGO_BUILD_JOBS=2 cargo xtask component-shot ticket-type --output docs/screenshots/TIK-109
```

Set `CHROME_BIN` to a headless Chromium with loopback access. In this environment,
a temporary browser wrapper used a mount namespace with a direct proxy policy
for that child process only; shared browser policies were never modified.

Classification reuses the existing dark surface, text, border, spacing, radius,
and type tokens. The editor offers all nine types and No type, with an optional
scope. Scope-only tickets show No type alongside their scope; PR formatting still
falls back to chore. Long badge values truncate visually and retain their full
text in the accessible label and hover title.

Validation includes the full migration chain with fixture rows (prefix parsing,
label fallback, unknown prefixes, FTS, retained history IDs and repeat-run ticket,
history and cache-version stability), all three API mutation routes, null clears,
claim PR titles and shared formatter unit tests. Host/wasm Clippy, workspace tests,
release web/Worker builds, token/CSS lint and the autofill check passed. Local
Worker smoke and all 53 skill curl examples passed. The Impeccable mechanical
scan returned no findings; visual polish preserved the existing dark hierarchy.
The screenshot task checks type selection/clearing and real browser pasting of
a 32-character scope with surrounding spaces on desktop/mobile. The input
preserves that raw text for API normalization instead of silently truncating it.
