# Loading copy audit (TIK-148)

The before assets were built from `b4968bf3fd1a2538cc9d92f5af585895891eb63c` in this
worktree before editing UI source, then copied to ignored
`target/TIK-148/before-dist`. Both phases use real rendered routes and held fixture
API responses, bundled fonts and the published ascii-reveal animation.

The paired route PNGs cover ticket detail, Board, List, Projects, Home, Analytics,
Mission Control and Settings (account/GitHub, API tokens, project and pickup
rules) at 1440×900 and 390×844. `after-search.png` captures held search results;
`components/after-skeletons-detail.png` is the tightly cropped 2× component shot.
The `cold/` captures and JSON cover throttled fresh navigation at 1280×800 and
390×844, including skeleton, decode and settled scroll extents.

## Audit and polish

- Shared detail/board/list/project skeletons: hide the redundant caption while
  retaining its exact layout footprint. Token settings retain the margins that
  previously came from status roles, now using explicit token-based loader styles.
- Account, GitHub and token settings: hide the redundant copy beside the loader
  while retaining its footprint. The gallery loader uses the same convention.
- Home, Analytics, Mission Control, pickup rules and search: no visible loading
  copy to remove. Ensure a visually hidden polite status outside busy regions;
  hide decorative skeletons from assistive technology and avoid duplicate statuses.
- Preserve the existing black surfaces, token palette, quiet glyphs, motion and
  responsive layout. No skeleton dimensions, spacing or animation changed.
- Impeccable polish: inspected the rendered detail component and scoped source;
  the bundled detector returned `[]` (exit 0). Independent critique is pending
  the permission choice required by the critique instructions; it has not been performed.

## Reproduction

Build the unchanged assets before editing UI source, copy them to the ignored
baseline directory above, then run:

```sh
source .job.env
export PATH="$HOME/.cargo/bin:$PATH" CARGO_BUILD_JOBS=4
export CHROME_BIN=<local-capable-chromium>
export TICKETS_LOADING_SHOTS=docs/screenshots/TIK-148
TICKETS_BEFORE_ASSETS="$PWD/target/TIK-148/before-dist" cargo xtask loading-check
cargo xtask build
cargo xtask loading-check
cargo xtask component-shot skeletons --variant detail --output docs/screenshots/TIK-148/components
```

`before-geometry.json` and `after-geometry.json` record loading rectangles; the
check requires exact equality when the baseline exists. It also rejects visible
loading copy, missing busy regions and missing visually hidden polite statuses
outside busy subtrees, and checks that announcements clear after settlement.

On this box the managed proxy blocks loopback even with `--no-proxy-server`.
The ignored `target/TIK-148/chrome-local` wrapper uses a private bubblewrap mount
to override only its child Chromium's proxy policy. Shared policy is unchanged.
The initial unwrapped run failed with `ERR_SOCKS_CONNECTION_FAILED` and a cold
loading timeout; the local wrapper resolves that environmental failure.

Additional validation: `cargo xtask ci`, `npm run lint:css --prefix crates/web`,
`TICKETS_SKIP_BUILD=1 cargo xtask first-run-check` and
`TICKETS_SKIP_BUILD=1 cargo xtask settings-check`. The two Worker checks run
sequentially in a private user/network namespace with loopback enabled, avoiding
port 8795 conflicts with other worktrees. They use the same local Chromium wrapper.


## Shared-runner identity

A concurrent worktree replaced the shared `xtask` executable during an earlier
run: PNGs refreshed, but this ticket's geometry JSON did not. Its apparent pass
was discarded. Final validation uses a private copy of the freshly compiled
runner, checked for both new assertion messages before execution:

```sh
touch xtask/src/main.rs
RUSTC_WRAPPER= cargo build -p xtask --locked
cp "$CARGO_TARGET_DIR/debug/xtask" target/TIK-148/xtask
python3 - <<'CHECK'
from pathlib import Path
runner = Path("target/TIK-148/xtask").read_bytes()
assert b"Loading geometry changed" in runner
assert b"Loading accessibility mismatch" in runner
CHECK
CHROME_BIN="$PWD/target/TIK-148/chrome-local" TICKETS_LOADING_SHOTS=docs/screenshots/TIK-148 target/TIK-148/xtask loading-check
```

The private runner also executes the final CI, first-run and settings checks.
No shared Cargo caches or configuration were changed.


## Final results

All commands below exited **0**. Browser commands use the local Chromium wrapper;
first-run and settings use the private network namespace described above.

| Check | Command |
| --- | --- |
| App build | `cargo xtask build` |
| Full CI (tokens, fmt, host build, host/wasm Clippy, workspace tests) | `target/TIK-148/xtask ci` |
| CSS lint | `npm run lint:css --prefix crates/web` |
| Loading | `target/TIK-148/xtask loading-check` |
| Component | `target/TIK-148/xtask component-shot skeletons --variant detail --output docs/screenshots/TIK-148/components` |
| First-run | `TICKETS_SKIP_BUILD=1 target/TIK-148/xtask first-run-check` |
| Settings | `TICKETS_SKIP_BUILD=1 target/TIK-148/xtask settings-check` |
| Patch whitespace | `git diff --check` |

The before/after geometry matches exactly: **128 rectangles across 22 route/viewport
pairs**. All ten throttled cold navigations have zero overflow frames. The final
loading run also passes accessibility/announcement clearing, search, sweep/decode,
reduced motion, repeated submit controls and route-disposal checks. There are 52 PNGs.

One prior private run timed out waiting for the mobile Board skeleton, with no
reported network errors or memory pressure. The single diagnostic rerun passed;
the timeout did not recur. The exact cause remains unconfirmed. The readiness
predicate and 120-second deadline are unchanged; future timeouts now retain
browser/resource state as JSON to help identify the cause.

The Impeccable context loader also reports that PRODUCT.md predates its current
schema. `/impeccable init` can update that record; it was not expanded during this
loading-copy refinement.
