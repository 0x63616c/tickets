# TIK-94 WebKit verification

Release app from TIK-94-cmd-number-nav after merging origin/main (`1149989`).
Playwright WebKit 27.2, installed at `/home/box/.cache/ms-playwright/webkit-2370`;
Playwright package 1.64.1 from the existing box installation. Dark 1440×900,
loaded bundled fonts, real WebKit key dispatch with `navigator.platform=MacIntel`.

The local harness follows xtask browser checks: isolated local Wrangler/D1,
complete migrations, generated fixture credentials, and a sample project/ticket
seeded through the API. No production requests or remote migrations.
WebKit rejects Secure cookies over loopback HTTP; the harness installs the session
returned by successful fixture login with `secure:false` in its browser context
only. Application and production cookie settings are unchanged.

`smoke.log` records authenticated hard loads of `/login`, `/`, `/board`, `/list`
and `/mission`, followed by Cmd+1 → Home, Cmd+2 → Analytics, Cmd+3 → Board,
Cmd+4 → List. Every pageerror and console error fails the run. All passed with
zero errors; nine PNGs show each route/shortcut result. This verifies a clean local
release module graph, including the board drag bridge, rather than reproducing
an existing production browser cache or testing native macOS Safari.

Validation: `cargo xtask ci` (fmt, host + wasm32 Clippy with -D warnings,
workspace tests, design tokens and policy tests), CSS lint/policy tests, release
`cargo xtask build`, Chromium navigation-check, palette-check and settings-check.
The first release build reused stale shared Cargo output; navigation-check caught
the missing shortcuts. Final browser checks used forced fresh web and Worker
release compiles (`RUSTC_WRAPPER=`, touched crate roots; no source changes), and
all passed. Browser checks use private network/Chrome policy mount namespaces.
