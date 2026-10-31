# TIK-95 real WebKit loading verification

Release build of merge commit `3ffa7d1`, containing TIK-95 and origin/main
`04755ad`. This supplements the paired Chromium/component evidence in [../README.md](../README.md).

The harness adapts TIK-94's working local Wrangler/D1 login harness. It applies
all migrations to isolated local state, generates temporary fixture credentials,
and creates one sample project/ticket through the local API. The session cookie
returned by fixture login is installed with `secure:false` only inside the test
browser context for loopback HTTP. Application cookie settings are unchanged.
Port 18995 avoids other worktrees' local checks. No production data is used.

Run from the worktree root:

```sh
source .job.env
export PATH=$HOME/.cargo/bin:$PATH
cargo xtask build
node docs/screenshots/TIK-95/webkit/smoke.mjs
```

Requires the existing Playwright package at `/tmp/wk2/node_modules/playwright`
and WebKit installation at `/home/box/.cache/ms-playwright/webkit-2370`.
The generated credentials and local D1 state remain ignored under `target/`.

Dark captures use 1440×900 and 390×844 viewports with bundled fonts loaded.
On Home, Analytics, Board, List and Mission Control, `page.route` holds
`/api/v1/**` requests until both loading PNGs are captured. These are real
application routes backed by the local Worker, without mocked animation.
`*-loading-a.png` and `*-loading-b.png` show different moments of the live
package sweep. Computed-style assertions require transparent glyph/block hosts,
card shells matching `--surface`, and base/peak glyph opacity of .12/.22.
Actual canvas pixel comparisons require the shimmer to move. Releasing the
requests must remove the skeletons. Login has a form and a pending submit
button instead of a page skeleton; both states are captured at each width.

Every pageerror and console error fails the run, including after responses
resume. See [smoke.log](smoke.log) for engine version and per-route results.
These checks verify real Playwright WebKit, rather than native macOS Safari.

Post-merge validation: release `cargo xtask build`, `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
`cargo xtask check design-tokens`, and `npm run lint:css --prefix crates/web`.

Result: PASS in Playwright WebKit 27.2, zero pageerrors and console errors
through all six routes at both widths. Visual review of desktop/mobile captures
confirms the skeletons sit on the existing black/page and near-black/card
surfaces, without grey inset blocks; the paired captures and canvas assertions
confirm a quiet moving shimmer. All listed post-merge validation passed.
