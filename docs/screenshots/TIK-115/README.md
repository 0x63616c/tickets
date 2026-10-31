# TIK-115: linked pull requests in ticket properties

Dark-mode evidence uses synthetic data only. Before assets are built from
`1f8dd9c54f5281e21730e15a2510b63dded0bd50` in a detached temporary worktree.
After assets come from this issue worktree. Separate inherited release profiles
avoid shared build artifacts being replaced by concurrent issue worktrees.

- `before-desktop.png` / `after-desktop.png`: actual API-backed ticket detail,
  1440×900, with the same signed GitHub PR fixture.
- `before-mobile.png` / `after-mobile.png`: same ticket at 390×900; Properties
  expanded and scrolled into view.
- `after-linked-pull-requests.png`: tightly cropped fixture-only 2× component preview,
  showing open/draft/merged/closed and passing/pending/none/failing CI, with long
  titles. This shares the live sidebar component.

Reproduce after building web and Worker assets:

```sh
CHROME_BIN=/usr/bin/google-chrome TICKETS_SKIP_BUILD=1 cargo xtask mirror-check
CHROME_BIN=/usr/bin/google-chrome cargo xtask component-shot linked-pull-requests --output docs/screenshots/TIK-115
```

For the before pair, supply `TICKETS_BEFORE_ASSETS=<base crates/web/dist>` to
`mirror-check`. All Worker/D1 state is isolated and local. No production writes.

This machine's managed Chrome proxy blocks loopback. Captures used the existing
`/workspace/tickets-polish/chrome-noproxy` wrapper, which overlays only the Chrome
child process's proxy policy via bwrap. Shared policies remain unchanged.

Validation passed: `cargo xtask ci` (format, host/wasm clippy, workspace tests,
source token policy and policy tests), CSS lint, all six built-Worker integration
tests, mirror acceptance (before/after), component-shot, first-run-check, and all
50 documented skill curl examples. Custom-profile Wasm was processed with the
cached Trunk wasm-opt (`-Oz --strip-debug`) before serving through local Workers.

Impeccable critique used independent assessments (`ui_critique_a`,
`ui_critique_b`), followed by polish of the corrected captures. No P0/P1 findings;
31/36 applicable heuristic points (read-only error recovery is not applicable).
The source detector reported zero findings. Long titles truncate as requested;
the accessible link name and native title retain the full text. Raw provider
conclusions in hover titles explain neutral/skipped/canceled CI displayed as None.
No native browser presentation tool was exposed, so no visual overlay was claimed;
separate Chromium CDP measurements supplement the real Worker browser acceptance.
Temporary browser/server processes were stopped after measurement.

| Measured component text | Contrast against composited background |
| --- | ---: |
| Heading | 19.74:1 |
| Repository and title | 14.38:1 |
| CI | 6.46:1 |
| Open / Draft badges | 11.23:1 / 11.52:1 |
| Merged / Closed badges | 10.33:1 / 10.73:1 |

At desktop 1440px and mobile 390px, document scrollWidth equals viewport width.
Links measure 224×40 and 224×44 respectively. The real detail captures also verify
sidebar placement and keyboard focus; empty links remove the section after reload.
