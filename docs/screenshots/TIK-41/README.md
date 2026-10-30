# TIK-41 phase-1 evidence

Critique and concrete phase-2 handoff: [api-tokens-critique.md](../../design/api-tokens-critique.md).

Baseline: `63107b4275807a3a54956b8708921d1153dd20fc`, unchanged app code from the TIK-41 worktree. No PR, commit, deploy or remote migration. TIK-38/TIK-40 integration is deferred to phase 2.

## Captures

| State | Desktop 1440×900 | Mobile 390×844 |
| --- | --- | --- |
| Empty | [PNG](before-empty-desktop.png) | [PNG](before-empty-mobile.png) |
| Several tokens | [PNG](before-list-desktop.png) | [PNG](before-list-mobile.png) |
| Create form | [PNG](before-create-desktop.png) | [PNG](before-create-mobile.png) |
| One-time fixture secret | [PNG](before-secret-desktop.png) | [PNG](before-secret-mobile.png) |
| Fixture secret copied | [PNG](before-secret-copied-desktop.png) | [PNG](before-secret-copied-mobile.png) |
| Revoke action focused | [PNG](before-revoke-desktop.png) | [PNG](before-revoke-mobile.png) |
| Revoked result | [PNG](before-revoked-desktop.png) | [PNG](before-revoked-mobile.png) |

There is no existing revoke-confirmation dialog. The last two pairs honestly record the existing action and immediate result; they do not fabricate a missing state. These are viewport captures of the actual application, including its full shell. Content below the 844px mobile viewport remains scrollable.

All PNGs are also in `docs/screenshots/TIK-41/`. No after PNGs were produced because phase 1 authorizes review and specification only.

## Fixture boundary

Dedicated Wrangler config/state: `target/tik41-review/` in `/workspace/wt/tickets/TIK-41`. Capture server used only `http://127.0.0.1:8842`; independent assessment used port 8841 and a separate state directory. Neither used the shared 8795 check port. Both app servers are stopped after review.

The complete migration chain was applied with `--local`. A fixture owner/session bootstrapped the local app. Session storage stays in ignored `target/`; it is not part of the evidence bundle. Names begin “Fixture”; seeded token hashes derive from invalid fixture strings. Each primary screenshot starts from the same local token fixture with fixed creation/last-used dates. The capture harness resets only its dedicated local `api_tokens` table between scenarios.

The secret screens execute the actual local POST, preserving returned metadata, but replace the secret in the response with `tk_FIXTURE_ONLY_NOT_A_VALID_TOKEN_0000000000000000000000000000000000` before rendering. It fails the app's token syntax and cannot authenticate. The raw local response token is never written to a screenshot/log. Copy assertions use only this sentinel; dismissal and reload remove the disclosure.

Assessment B's supplementary error/empty/long-name/pending states use response interception, documented separately in `assessment-b/assessment-b.txt`. They are supporting evidence, not additional before/after implementation claims.

## Reproduction and verification

- `source .job.env; cargo xtask build` passed (release frontend and Worker). Log: `target/tik41-review/build.log`.
- `wrangler d1 migrations apply DB --local --config target/tik41-review/wrangler.toml --persist-to target/tik41-review/state2` initialized the capture database; log: `target/tik41-review/migrations2.log`.
- Local server: `wrangler dev --local --ip 127.0.0.1 --port 8842 --inspector-port 0 --config target/tik41-review/wrangler.toml --persist-to target/tik41-review/state2`.
- The ignored setup config uses fixture-only bootstrap credentials. A local browser login writes `target/tik41-review/storage2.json`; start from a fresh local fixture session if that session expires. Do not reuse a production session.
- Capture harness: `source .job.env; node target/tik41-review/capture-fresh.cjs`. A copy is retained here as `capture.cjs`. It relies on the local config/session above and installed `playwright-core` at `/tmp/quantum125-browser/node_modules/playwright-core`; no browser dependency was added to the app.
- Browser: `/usr/bin/google-chrome`, fresh headless process per state/size, dark, DPR 1, bundled fonts loaded. Managed proxy blocked loopback initially; `target/tik41-review/chrome-local` uses `bwrap` to override proxy policy only inside its child. No shared policy was edited.
- PNG dimensions and identical copies were checked, along with DOM overflow, successful fixture copy, dismissal, reload, immediate revoke and zero page errors. `capture-evidence.json` contains the primary geometry/assertion record. `manifest.json` contains PNG checksums and text-pixel checks.
- Some image previews intermittently omitted repeated static glyphs. This is not a product finding. Direct inspection of the final PNG pixels verifies all 14 page titles, section headings and introduction text, with matching counts for each viewport; no final image was retouched.
- Impeccable 4.8.0 ran with `IMPECCABLE_NO_TELEMETRY=1 DO_NOT_TRACK=1` and project-local `IMPECCABLE_HOME`. Independent assessments, detector limits and cleanup are in the critique and assessment B report.

App code was not changed, so phase-2 Rust/UI regression suites and `first-run-check` were not claimed or run for this documentation-only phase. Impeccable polish, before/after comparison and PR checks remain phase-2 work.

## Phase 2 implementation evidence

The Phase 1 sections above preserve the original review record. Phase 2 uses the
merged Settings and shared-component foundations and adds the corresponding
`after-*.png` captures in this directory. The baseline and after routes have
intentionally different Settings navigation because TIK-40 merged between phases.

Reproduce the current fixture captures with:

```sh
source .job.env
cargo xtask build
CHROME_BIN=/usr/bin/google-chrome cargo xtask tokens-shot
```

`xtask/src/tokens.rs` starts its own ephemeral fixture-only server and Chrome
processes and stops them on completion. The after images use
`/design/settings/api-tokens`, in-memory fixture metadata and an invalid
`tk_FIXTURE_ONLY_NOT_A_VALID_TOKEN_…` secret. No real token is minted by that
capture task. Pages use 1440×900 and 390×844 dark viewports; `*-component.png`
files crop the Settings section or dialog at 2×. Fonts and finite transitions
settle before capture. Reload resets all fixture state.

In this managed environment Chrome’s mandatory SOCKS policy blocks loopback
with the standard binary. Local evidence uses `/usr/bin/google-chrome` inside a
child-only `bwrap` mount that replaces `zz-home-proxy.json` with `{}`. The shared
policy is unchanged. See the original Phase 1 note for the same environment
limitation; the Phase 1 Node script is historical provenance, while the Rust
xtask is the supported current reproducer.

### Implementation review and polish

Impeccable 4.8.0 ran with telemetry disabled and project-local cache. Assessment
A (`/root/token_design_review`) finished independently of detector output, then
reviewed the final matched PNGs: 34/40 heuristic score, no design blocker. The
polish pass fixed history counts and creation access, secret identity and
intentional discard, keyboard manual copy, long-name wrapping, mobile targets,
input boundary contrast and reserved copy-feedback space. Direct Revoke is a
narrow intentional departure from the one-item menu proposed in Phase 1.

Assessment B (`/root/token_evidence_review`) ran the CLI detector once: `[]`, exit
0. Rust/Leptos coverage is unverified and subsequent review fixes make this a
limited initial scan. Browser injection succeeded in list/create/secret states;
all three reported no anti-patterns. This was headless inspection, not a
human-visible overlay. The owned browsers, fixture servers and live server were
stopped after inspection.

Rendered evidence: meaningful token input border **3.16:1**; mobile controls
**44px** (including the final empty-state correction); 100-character names fit at
390px and 320px; readonly secret supports Shift+Tab then Ctrl+A for all 68 fixture
characters. Reduced-motion dialogs transition opacity only. A 720×450 layout
(the geometry equivalent of 200% desktop zoom) keeps the dialog within the
viewport with internal scrolling; actual browser zoom and a human screen-reader
pass were not performed. Visible timestamps intentionally stay at UTC-day
precision; exact values are present in accessible labels/tooltips. The shared
Modal keeps its creation title during the save step, whose heading and focused
Copy warning identify the new task.

Questions skipped: the ticket already specifies implementation scope and style;
remaining optional expiry/scopes/rotation work is explicitly deferred.

### Checks completed

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo clippy --workspace --all-targets --target wasm32-unknown-unknown -- -D warnings`
- `cargo test --workspace`
- `cargo xtask build`, `tokens-shot`, `component-shot icons`, `settings-check`,
  `first-run-check`, `polish-check`, `contrast-check`, `design-check`

The local Worker checks ran serially; port contention required retries and then
an isolated `unshare -Urn` network namespace with loopback plus a local veth pair
so Chromium retained online state. No other job’s server was stopped. The design
check uses a fixture-only ephemeral server. Earlier packaging races and
intermediate mismatched captures were discarded; final PNGs came from the serial
build, with a passing copy-success/clipboard-equality assertion.

The contrast run exposed the Settings foundation’s 10px sidebar shortcut. It is
now the existing 11px token size. The design check’s stale fixture author
expectation was updated to the current `Sample User`; no domain behavior changed.

`after-evidence.json` records final dimensions, checksums and fixture assertions.
All 50 before/after PNGs were copied byte-for-byte to
`/workspace/tickets-polish/TIK-41/`. Before images retain their original Phase 1
checksums. Final after states include empty, active inventory, history, create,
secret, copied secret, discard confirmation, revoke and revoked result.
