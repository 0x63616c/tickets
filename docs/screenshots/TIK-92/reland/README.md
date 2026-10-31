# TIK-92 Safari-safe re-land evidence

Release build from `TIK-92-reland-safari`, based on main `b9db5b0`. Restores #113
and #118, with a new content-versioned drag/motion module. Existing `dom.js` is
byte-for-byte identical to main. Browser checks use local services only; no deploy
or remote migration was performed.

## Browser acceptance

`verify.mjs` starts isolated local Wrangler/D1 on port 8796, applies the complete
local migration chain, generates fixture credentials, logs in through the UI,
and seeds a sample project through the API. It runs Playwright WebKit 27.2
(`/home/box/.cache/ms-playwright/webkit-2370`) and real Chromium at 1440×900, dark,
with bundled fonts loaded. Loopback fixture authentication installs the session
returned by the successful UI login with `secure:false` in the browser context
only; production cookie settings remain unchanged.

Every run fails on any pageerror or console error. Before navigation it intercepts
all previous snippet paths: main's `dom.js` is returned verbatim; ascii-reveal is
the same package bytes (lockfile integrity asserted unchanged). New digest and
Sortable paths did not exist on previous main and are fetched normally. The old
responses remain substituted throughout both drag/save/reload round trips.
The harness asserts legacy DOM source identity and the new module's source digest.

Each browser moves the same card Backlog → Todo → Backlog using real mouse input,
20 measured moves per direction. Every sample asserts ≤2px absolute preview axis
error against the original pickup rectangle, original source geometry, grayscale,
opacity .35, inertness, aria-hidden and pointer transparency. Each drop checks a
successful PATCH, settled application state and the destination after a reload.
Navigation waits for pending application requests to settle; aborting them during
hard navigation can otherwise surface a WebKit fetch error unrelated to drag.

Final run: **zero pageerrors and zero console errors**, including stale-snippet
substitution, both saves and both reloads in each engine.

| Engine | Real mouse samples | Maximum axis error | Persisted round trip |
| --- | ---: | ---: | --- |
| WebKit 27.2 | 40 | 0.8px | Passed |
| Chromium 139.0.7258.154 | 40 | 0.000055px | Passed |

See `smoke.log` and `tracking.json` for final results and every measured point.
`checks.log` contains build, lint/test logs and browser-suite acceptance summaries.
`*-before.png` means before the gesture in this release, not a pre-change build.
`*-out-drag.png` / `*-back-drag.png` show the floating preview, grey source and
purple insertion slot; `*-destination-drag.png` capture the final cross-column
position before release; `*-reloaded.png` show persisted destinations. Historical
pre-change dark PNGs restored with the original feature are in
`../before-1440-step-*.png` and `../../TIK-51/before-board-desktop.png`; they are
labelled historical evidence, rather than matching this newly seeded fixture.

## Additional checks

- Release web and Worker build (`cargo xtask build`).
- `cargo fmt --all --check`.
- Host and wasm32 `cargo clippy --workspace --all-targets -- -D warnings`.
- `cargo test --workspace`, including a regression test requiring the new module
  source digest, Rust imports and Worker public URL to stay in agreement; malformed
  digest paths remain private. Retained digest-named modules are public because
  Trunk can preload cached retired versions.
- `cargo xtask check design-tokens`, `npm run lint:css --prefix crates/web`,
  `npm run test:lint --prefix crates/web`.
- Acorn 8.15.0 ECMAScript 2020 module parsing of legacy DOM, new drag/motion and
  bundled SortableJS, plus a check for regex lookbehind/logical assignment.
  `syntax.log` records the source files. This is a Safari 15+ syntax floor check;
  actual runtime evidence uses current WebKit, not Safari 15 itself.
- Full `board-check`: keyboard, real mouse/touch tracking, fixed origin, insertion,
  persistence, Escape/interruption/outside cancellation, reduced motion, vertical
  and horizontal autoscroll, filters, project isolation and rejected-write rollback.
- `first-run-check`: empty local database, UI login/project/ticket creation,
  Kanban move, label and dependency; screenshot `first-run-kanban.png`.
- `component-shot board-columns`: fresh fixture-only, tightly cropped 2× dark
  component captures and full boards under `components/`.
- Independent Impeccable critique and polish: 35/40, no blocking scoped issue,
  detector exit 0 with `[]`. See `critique.md` for scope and minor observations.

## Reproduce

```sh
source .job.env
export PATH="$HOME/.cargo/bin:$PATH"
cargo xtask build
PLAYWRIGHT_MODULE=/tmp/wk2/node_modules/playwright/index.mjs \
  CHROME_BIN="$PWD/target/chromium-local" \
  node docs/screenshots/TIK-92/reland/verify.mjs
TICKETS_SKIP_BUILD=1 CHROME_BIN="$PWD/target/chromium-local" cargo xtask board-check
TICKETS_SKIP_BUILD=1 CHROME_BIN="$PWD/target/chromium-local" cargo xtask first-run-check
```

`PLAYWRIGHT_MODULE` accepts another installed Playwright module. `CHROME_BIN` is
optional; on this box the ignored local wrapper runs `/opt/ungoogled-chromium/chrome`
in a child-only bubblewrap namespace with browser policy directories hidden,
`/dev` bound and `/proc` mounted. This allows loopback without editing shared
managed proxy policy. A normal unrestricted machine can omit `CHROME_BIN`.
Concurrent worktrees can replace shared Cargo artifacts between Trunk phases.
One build reported success but emitted another worktree's glue; the missing digest
import was caught before acceptance. Final verification requires the emitted
legacy/new modules to match their sources and the glue to import the new digest.
Rebuilds used `RUSTC_WRAPPER=`; the final browser run checks the exact emitted
bytes and final digest import before acceptance.

The script creates a fresh ignored state directory per run and stops its local
Worker/browser in `finally`. No fixture password or cookie is logged or committed.
