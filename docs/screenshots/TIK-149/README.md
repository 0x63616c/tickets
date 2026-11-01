# TIK-149 — ticket classification properties

The before build is the real UI from `b4968bf` (the ticket branch's starting
commit), exported into ignored `target/tik149-baseline/`. The after build is
this ticket's implementation. Both use isolated local Worker/D1 fixtures,
loaded bundled fonts, dark mode, and the same desktop (1440×900) and mobile
(390×844) viewports. Mobile captures expand Properties.

- `before-{desktop,mobile}.png`: existing ticket detail.
- `after-{desktop,mobile}.png`: saved `feat` type and `ui/api` scope.
- `after-type-{desktop,mobile}.png`: fixed type list with descriptions.
- `after-scope-{desktop,mobile}.png`: free text scope picker and clear action.

Reproduce with `CHROME_BIN=/usr/bin/google-chrome TICKETS_SKIP_BUILD=1 cargo
xtask detail-check` after `cargo xtask build`. For before images, use the same
command with `TICKETS_BEFORE_ASSETS` pointing at the baseline build's absolute
`crates/web/dist` path.

The real API browser checks cover project-only, deduplicated suggestions,
keyboard navigation/cancel/focus restoration, normalization, clearing, validation,
exact history changes, derived PR titles, rejected writes, and mobile overflow.

Verification used `.job.env`, `$HOME/.cargo/bin` first on PATH, and
`CARGO_BUILD_JOBS=4`:

- `cargo xtask ci` (format, host/wasm Clippy, workspace tests, token/autofill checks).
- `npm run lint:css --prefix crates/web`.
- `cargo xtask build`.
- `TICKETS_SKIP_BUILD=1 cargo xtask detail-check` (full regression suite).
- `TICKETS_SKIP_BUILD=1 cargo xtask first-run-check`.
- `cargo xtask component-shot ticket-type` (fixture-only 2× captures in ignored target).
- Capture-only detail replay after switching picker images to the existing
  `visible_crop` compositor helper; functional assertions still run.

The browser commands set `CHROME_BIN` to an ignored local launcher that overrides
only the forced proxy policy inside a private user/mount namespace. The default
machine policy failed loopback navigation with `ERR_SOCKS_CONNECTION_FAILED`.
Shared Cargo artifacts were also rejected when they rendered an older UI; the
accepted captures show this implementation and the full detail suite passed
against those assets. No shared policy/configuration was changed.

Manual polish inspected the saved properties and picker states at both sizes:
the existing dark surfaces, type scale, property rhythm, focus rings, and popup
geometry are preserved. The Impeccable source detector reported no findings.
The formal two-agent Impeccable critique is pending permission to spawn reviewers;
it is not recorded as completed.
