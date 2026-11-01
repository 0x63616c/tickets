# Board blocked tooltip regression

Merged `origin/main` into `TIK-146-sidebar-tooltip` without conflicts.

TIK-146 applied navigation hint rules to the shared Tooltip: focus had to match
`:focus-visible`, and global capture listeners dismissed the tooltip on click.
Blocked board/list markers deliberately focus on click to expose their blocker
keys and titles. The shared changes suppressed that informational tooltip.

`navigation_hint=true` now explicitly opts sidebar hints into those rules.
Other tooltips accept ordinary focus and keep their existing scroll/resize
positioning listeners. The board-check expectations are unchanged.

## Verification

Every Cargo shell first ran:

```sh
source .job.env
export PATH="$HOME/.cargo/bin:$PATH" CARGO_BUILD_JOBS=4
```

| Exact command | Exit | Result |
| --- | --- | --- |
| `git fetch origin && git merge origin/main` | 0 | Clean merge. |
| `CHROME_BIN=/usr/bin/google-chrome cargo xtask board-check` | 1 | System Chrome's managed SOCKS policy prevented access to the loopback test server (`ERR_SOCKS_CONNECTION_FAILED`). No system policy was changed. |
| `TICKETS_SKIP_BUILD=1 CHROME_BIN=/home/box/.cache/ms-playwright/chromium_headless_shell-1248/chrome-headless-shell-linux64/chrome-headless-shell cargo xtask board-check` | 1 | Original built assets reproduced the exact CI timeout for `Blocked by TKT-1: Remember filters when switching views`. |
| `CHROME_BIN=/home/box/.cache/ms-playwright/chromium_headless_shell-1248/chrome-headless-shell-linux64/chrome-headless-shell cargo xtask board-check` | 0 | Rebuilt fixed assets; full board check passed, including board/list click/focus, multiple blockers, Escape and live dependency reconciliation. |
| `cargo xtask ci` | 0 | Full task: locked npm install, autofill/raw-color/design-token checks, CSS policy tests, formatting, workspace build, host and WASM Clippy, workspace tests. |
| `npm run lint:css --prefix crates/web` | 0 | CSS lint passed. |
| `CHROME_BIN=/home/box/.cache/ms-playwright/chromium_headless_shell-1248/chrome-headless-shell-linux64/chrome-headless-shell cargo xtask sidebar-tooltip-check` | 0 | Desktop/collapsed/mobile positioning, timing, focus and dismissal passed. |
| `CHROME_BIN=/home/box/.cache/ms-playwright/chromium_headless_shell-1248/chrome-headless-shell-linux64/chrome-headless-shell cargo xtask card-shots` | 0 | Board/list fixtures and multi-blocker hover tooltip passed. |
| `CHROME_BIN=/home/box/.cache/ms-playwright/chromium_headless_shell-1248/chrome-headless-shell-linux64/chrome-headless-shell cargo xtask component-shot tooltip --output target/TIK-146-tooltip/components` | 0 | Dark 2× component capture passed. |
| `TICKETS_SKIP_BUILD=1 CHROME_BIN=/home/box/.cache/ms-playwright/chromium_headless_shell-1248/chrome-headless-shell-linux64/chrome-headless-shell cargo xtask first-run-check` | 0 | Empty-install acceptance passed with UI-only project, ticket, label and dependency creation. |

No requested platform checks were skipped. Browser verification used the installed
Playwright headless Chromium because the system Chrome policy blocked loopback.

## Visual and design review

- [Before](before-board-blocked-tooltip.png): original branch's board-check failure capture; clicking TKT-2's blocked marker leaves the explanation hidden.
- [After](after-board-blocked-hover.png): fixed card fixture showing both blocker keys and titles. This fixture uses different tickets from board-check.
- Impeccable critique used independent assessments (`tooltip_design_review` and `tooltip_detector_review`). Source/screenshot review found no actionable issue in scope; heuristic score 32/36 (error recovery not applicable). Detector command below exited 0 with `[]` (zero findings). No browser overlay was injected; rendered verification used the regression checks and screenshots.
- Polish review preserved the dense dark layout, tokens, accessible marker name and `aria-describedby`, keyboard opening and Escape dismissal. Rendered multi-blocker text is legible and stays associated with its marker. No cosmetic changes were needed. Questions skipped: the requested regression fix defines the scope.

```sh
IMPECCABLE_NO_TELEMETRY=1 DO_NOT_TRACK=1 IMPECCABLE_HOME="$PWD/.impeccable" .agents/skills/impeccable/scripts/impeccable detect --json crates/web/src/components/feedback.rs crates/web/src/components/shell.rs
```
