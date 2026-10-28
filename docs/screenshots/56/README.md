# Issue 56: retain sign-in password after a network failure

Real `/login` route on an isolated local Worker/D1. Chromium blocks
`*/api/v1/login` with `Network.setBlockedURLs` after the form is filled;
no fetch replacement is used for these captures. Dark mode, loaded fonts,
1440×900 desktop and 390×844 mobile. All credentials shown are local fixtures.

Before: origin/main `ef20fe690f61ece4d394dcbe8b61423c49e10c94`, built in
`/tmp/tickets-56-main` and selected with `TICKETS_BEFORE_ASSETS`.
The password is empty after the failed request.

After: issue branch, same route, viewport, fixture and blocked request.
The password stays masked and filled, with an explicit retry message.

Reproduce with `CHROME_BIN=<headless-chromium> cargo xtask login-check`;
set `TICKETS_BEFORE_ASSETS=<main-worktree>/crates/web/dist` for before captures.
