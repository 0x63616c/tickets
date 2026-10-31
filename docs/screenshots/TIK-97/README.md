# TIK-97 API reference evidence

All captures use local Wrangler, isolated local D1, the generated fixture account,
loaded self-hosted fonts and the dark application shell. No production data or
credentials appear in these images.

- `before-desktop.png` / `before-mobile.png`: the actual `/api-docs` route at
  baseline `e48062e` (Page not found; the API reference did not exist). Baseline
  assets came from a disposable worktree, with a debug build and only Wasm debug/name
  metadata stripped to fit Wrangler's 25 MiB asset limit. Application code was unchanged.
- `after-desktop.png`: 1440 × 900 endpoint index.
- `after-mobile.png`: 390 × 844 endpoint index.
- `after-desktop-expanded.png` / `after-mobile-expanded.png`: filtered Projects
  reference with GET expanded, parameters and response schemas visible.
- `after-desktop-try.png` / `after-mobile-try.png`: session GET runner, native
  dark fields and the successful response.
- `*-full.png`: full-document versions from the document top, with explicit CDP
  viewport clips and `captureBeyondViewport`.

Reproduce after `cargo xtask build`:

```sh
CHROME_BIN=/usr/bin/google-chrome TICKETS_SKIP_BUILD=1 cargo xtask api-docs-check
```

For the before images, also set `TICKETS_BEFORE_ASSETS` to a built baseline dist.
The check logs in through the browser, checks every operation is listed, filters
and resets the list, expands schemas, sends a GET through the existing session,
and asserts desktop/mobile containment. The static spec is checked without auth
and with invalid bearer credentials. The compiled Worker test separately proves
zero binding/D1 accesses using an environment that throws on every binding read.

This machine's mandatory browser proxy excludes loopback. Captures use a private
mount namespace with a direct-connection policy only for the child Chromium
process. Shared browser policies were not changed.

Impeccable critique used independent design and detector assessments. The detector
reported zero findings. Polish addressed compact parameter schema summaries,
downward expanded chevrons, a visible tag-filter hint and native GET runner fields,
while preserving the
existing tokens, black canvas, fonts and shell.
