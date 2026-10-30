Dark, fixture-only evidence for Settings → Account → GitHub.

Before images use commit `38a5121`, which has no GitHub connection section;
the section crops show its existing Account surface. After images show the
unregistered and installed (selected repositories) states. The browser check
also verifies registered without installation, all repositories, loading,
error/retry and full-page navigation to the Worker registration page.

Page captures use desktop 1440px and mobile 390px widths with the full content
height. Section crops are native 2× PNGs. Motion is reduced for settled
captures. All account and app details are fixtures.

Run `cargo xtask settings-check` with `CHROME_BIN` set to a local Chromium.
Set `TICKETS_GITHUB_BEFORE_ASSETS` to the prior build's `crates/web/dist` to
capture before images too. Build baseline assets with a separate Cargo target
directory to avoid reusing current worktree artifacts.
