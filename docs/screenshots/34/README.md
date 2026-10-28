# Issue 34: standalone login

Before images come from a real origin/main build at
`c70541948afc2c230445d292df548a39c15d6d11`, built and served with local Wrangler
in a temporary `/tmp/tickets-34-main` worktree, which was removed afterward.
`before-root-*` and `before-login-*` are the actual logged-out routes.
`before-target-*` captures `/design` separately as the fixture reference.
The sidebar and shared application shell retain their original geometry.

After images show the actual `/login?next=%2Flist` route against an isolated
local Worker and D1, with generated local fixture credentials. `after-error-*`
shows a real rejected sign-in. `after-loading-*` holds a real login request at
Chromium's network interceptor to capture the disabled button deterministically.
No credential or token secrets are captured.

All images use dark mode, loaded bundled fonts and a device scale of 1:
1440×900 desktop and 375×812 mobile. `cargo xtask login-before` and
`cargo xtask login-check` reproduce the captures; set `CHROME_BIN` to a headless
Chromium binary (this box used the installed Playwright headless shell).
The login check owns port 8787; other API smoke checks retain port 8795.
`curl.txt` contains real local HTTP headers, with the session cookie value omitted.

Production deployment and curl verification follow an approved merge through CI;
this PR does not deploy or change remote D1.
