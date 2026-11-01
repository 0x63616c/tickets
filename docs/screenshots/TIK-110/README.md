# TIK-110 reconciliation evidence

Baseline: `f8b4a522df17f5b6ad34b6e6575c92f4e4258d12` (origin/main when work began).
Before PNGs use that commit's built web assets, saved in ignored
`target/TIK-110/baseline-dist`. After PNGs use this PR's build. All content is
fixture data; no production account or credentials are captured.

`component-shot board-topbar` captures the unchanged dark board chrome at 2× DPR
and the corresponding full-page desktop/mobile fixture views. The baseline uses
its normal capture mode with output names changed from `after-` to `before-`;
this harness's historical `--before-assets` mode specifically asserts the older
TIK-57 New ticket button, which is already absent from the current main baseline.

```sh
source .job.env
export PATH=$HOME/.cargo/bin:$PATH
cargo xtask build
CHROME_BIN=<local-headless-browser> cargo xtask component-shot board-topbar
```

The full-page fixtures show the incumbent visual design; this refactor changes
no tokens, markup, CSS, layouts or controls. The in-process policy has eight
host interface traces (`cargo test -p tickets-web reconciliation`). Browser
acceptance retains the existing live event/DOM editing checks and collection
request-count assertions.

Runtime checks reuse one verified web/Worker build with `TICKETS_SKIP_BUILD=1`:
`live-check`, `perf-browser-check`, `board-loading-check`, and `first-run-check`.
Each runs sequentially against fresh local D1 state in its own loopback network
namespace, avoiding the shared port 8795 used by concurrent issue worktrees.
A namespace-local veth pair supplies an online interface: a loopback-only namespace
makes Chromium report navigator.onLine=false and correctly suppresses subscriptions.
The initial loopback-only run timed out waiting for Connected; its log is retained
alongside the successful checks.
The managed proxy is overridden only inside each browser child's mount namespace;
shared browser policies are unchanged. Runtime logs and additional captures are
kept outside the commit under `/workspace/tickets-polish/TIK-110/`.

## Critique and polish

Two independent Impeccable assessments found no priority issues in the extraction
(18/24 source-based heuristic score; unrelated UI heuristics were not applicable).
The static detector returned zero findings, with limited Rust coverage; it is not
behavioral evidence. Browser overlay injection is not applicable to a policy-only
extraction with no new rendered surface. The parent polish pass inspected the
fixture views and compared every PNG pair: desktop/mobile topbar crops and full
board captures are byte-identical to the baseline. No cosmetic edits were made.

The loading harness waits for Board to mount after its List → Board SPA click
before recording the refresh root. The initial run captured two List samples
before Board mounted (null boardY, 101 rows, then 6 Board cards), causing a false
continuity failure despite zero skeleton/content overlap. The saved main build
reproduced the same List-to-Board transition in its first samples. The added destination
readiness wait preserves the refresh assertions and all application behavior.
