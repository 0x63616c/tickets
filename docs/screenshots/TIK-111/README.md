# TIK-111 — Board move extraction

Dark fixture-only `/design/board` captures with bundled fonts loaded.
Before assets were built from `648af10aecb59a88b74e9f309afedc656e2a5291`
before editing the browser implementation, and retained under ignored
`target/TIK-111-baseline-assets/`. The extraction changes no markup, CSS,
tokens, focus policy or drag adapter.

Reproduce the matching desktop/mobile component and board captures after building:

```sh
source .job.env
export PATH=$HOME/.cargo/bin:$PATH
CHROME_BIN=/workspace/tickets-polish/chrome-noproxy cargo xtask component-shot board-columns --output docs/screenshots/TIK-111
```

For matching before captures, temporarily use the retained baseline assets as
`crates/web/dist`, run the same command into a separate output directory, rename
its `after-` files to `before-`, and restore the final assets. The harness's legacy
`--before-assets` mode skips the Columns popover and leaves a different mobile
focus state; using the same interaction sequence avoids that comparison artifact.
All eight before/after PNG pairs are byte-identical.
Toolbar and popover crops use 2× DPR; the full board viewports use 1× DPR.
These images show sample data, not a live API collection. The actual move module
is exercised by host interface tests and the isolated local-D1 browser checks:

```sh
cargo test -p tickets-web board_move --locked
TICKETS_SKIP_BUILD=1 CHROME_BIN=/workspace/tickets-polish/chrome-noproxy cargo xtask board-check
TICKETS_SKIP_BUILD=1 CHROME_BIN=/workspace/tickets-polish/chrome-noproxy cargo xtask polish-check
TICKETS_SKIP_BUILD=1 CHROME_BIN=/workspace/tickets-polish/chrome-noproxy cargo xtask first-run-check
```

The browser wrapper changes proxy policy only inside its child process namespace;
plain Chrome on this box cannot reach loopback under the managed proxy policy.
Run browser and Cargo checks sequentially and retain `.job.env`'s shared target.

Validation uses fresh local fixtures, with Cargo and browser checks serialized:

- `cargo xtask ci`: exit 0 (format, native build, host/wasm clippy, 171 host tests,
  autofill, design tokens and CSS policy tests).
- `npm run lint:css --prefix crates/web`: exit 0.
- `cargo xtask build`, `board-check`, `first-run-check`, and the component capture:
  exit 0. Browser checks used a private copy of this checkout's `xtask` executable,
  following CONTRIBUTING.md, with `TICKETS_SKIP_BUILD=1` after the verified build.
- Full `polish-check`: exit 1 on both the unchanged baseline assets and this
  branch, at the existing ticket-detail comment-form/icon geometry assertion in
  `xtask/src/polish.rs`. The baseline was run through the same after-mode path;
  no assertion was skipped or changed.

`cargo xtask cleanup-check` passed (exit 0). It runs the existing TIK-45 acceptance cases independently
of that broad audit, including a real board reorder followed by a server read
checking that the legacy ticket still stores Todo and its `later` label.

Local Cargo validation used four build jobs, bypassed sccache, and used a temporary
pass-through `RUSTC_WORKSPACE_WRAPPER` under ignored `target/` to distinguish
workspace artifacts inside the configured shared target directory. Stale shared
artifacts were refreshed before clippy. The final complete CI run passed.

Manual preservation review and polish found no change to rendered board pixels,
focus messages, controls, hierarchy, layout or tokens. The Impeccable mechanical
detector returned no findings on the final board source. The full two-assessment
Impeccable critique is pending review-agent permission; no design changes were
made to resolve unrelated baseline issues.
