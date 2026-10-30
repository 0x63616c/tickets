# TIK-61 — Pickup rules

Dark fixture-only captures, bundled fonts loaded. Baseline is `07343d7` (TIK-60,
origin/main at implementation). Before shows the actual previous Settings →
Appearance page: no pickup editor existed. After shows the new Settings page and
its shared isolated component. No real tokens or account data are shown.

- `before-rules-desktop.png`, `after-rules-desktop.png`: 1440px full-page Settings.
- `before-rules-mobile.png`, `after-rules-mobile.png`: 390px full-page Settings.
- `before-rules-component.png`, `after-rules-component.png`: tight 2× crops.
- `after-rules-project.png`: saved project override, cap 2.

Reproduce after `cargo xtask build`:

```sh
CHROME_BIN=/workspace/tickets-polish/chrome-noproxy cargo xtask component-shot pickup-rules --output docs/screenshots/TIK-61
CHROME_BIN=/workspace/tickets-polish/chrome-noproxy cargo xtask component-shot pickup-rules --before-assets /tmp/tik61-before-07343d7/crates/web/dist --output docs/screenshots/TIK-61
```

The base assets are built in a detached temporary worktree. CI uses google-chrome;
the local wrapper isolates the managed SOCKS policy. Copies of all PNGs are kept
at `/workspace/tickets-polish/TIK-61/`.

## One Impeccable critique and polish pass

Two independent assessments: A reviewed design and interaction without detector
results; B ran the detector and a separate browser context. Initial design score
31/40, no P0/P1 issues. The existing near-black, Inter, dense Settings design is
binding. Heuristic scores, in standard Nielsen order: 3, 3, 3, 4, 3, 3, 3, 4, 2, 3.

The editor's three groups, visible inheritance, explicit saves, and responsive
stacking were strengths. Material findings and the single polish batch:

- Scope became disabled for an unsaved draft while its explanation was below the
  form. Added a nearby live explanation and associated it with the shared select.
- Label validation was technical and disconnected from its input. Added a local
  error, aria-invalid/description, and specific duplicate/length feedback.
- The scope border had only 1.237:1 contrast against its fill. Applied the same
  stronger boundary token mix already used by the text/number fields.
- Shortened the fixed eligibility hint to avoid a mobile one-word last line.
- Wait for the shared ASCII reveal to settle before capturing a saved override.

Initial browser evidence: no horizontal overflow at 1440/390px; minimum visible
form text contrast 6.463:1; disabled Save text 9.841:1; text/number boundaries
3.162:1 against fill; mobile controls 44px; checkbox labels 358×65.5px; visible
2px keyboard focus ring 4.468:1 against black. No browser application exceptions.

CLI detector ran once, exit 0 and zero findings, but does not establish Rust
Leptos coverage. The live overlay found zero issues on desktop/component and one
mobile `straggler` (the shortened hint). It ran in a headless browser, with no
user-visible overlay presentation. All review servers and contexts were closed.
Questions skipped: the findings fit the authorized polish pass.

Closure verification of only those findings: A confirmed nearby associated scope
guidance, input-associated duplicate/length feedback, and the settled override
capture. B measured the selector boundary at 3.162:1 against its fill and 3.298:1
against black. At 390px the shortened hint fits on one line (328.39px inside its
358px container). Both reviewers closed their original findings; no second
critique or detector run. All seven final PNGs were visually inspected.

Real local Settings checks additionally cover workspace persistence, failed-save
retention, duplicate-label validation, scope locking, project inheritance,
override creation/removal, and mobile overflow. API tests separately exercise
real D1 HTTP contention, heartbeat, release, cron, history and empty 204 responses;
these fixture PNGs are visual evidence, not proof of persistence.
