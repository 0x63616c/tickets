# TIK-39: new-ticket composer

Before source: `01f92c0` (merged TIK-38); assets from the clean TIK-38 worktree
at `a374f8d`, whose entire `crates/web` tree matches `01f92c0` (`git diff` empty).
After: this branch. Both render the actual shared API composer against the
component harness's fixture-only HTTP responses. No production data or credentials
are used. The isolated `/design/c/modal` also uses the same TicketComposer.

Component PNGs are dark, font-loaded and tightly cropped at 2× DPR. Status-open
includes the entire popover, even outside the modal. Full-page pairs are 1440×900;
mobile pairs are 390×844. `before-status-static.png` shows the locked baseline.

Pairs: modal (menus closed), segmented-write, segmented-preview, preview,
full-page and mobile. `after-status-open.png` demonstrates all six choices.

```sh
cargo xtask build
CHROME_BIN=/workspace/tickets-polish/chrome-noproxy cargo xtask component-shot modal --output docs/screenshots/TIK-39
CHROME_BIN=/workspace/tickets-polish/chrome-noproxy cargo xtask component-shot modal --before-assets <baseline-dist> --output docs/screenshots/TIK-39
```

The capture also checks equal Write/Preview geometry, empty preview, six status
icons, keyboard selection, Cmd+Shift+P, retained drafts, Escape confirmation and
both ways of returning to editing, reduced motion, field-boundary contrast and
mobile overflow. `board-check` covers real D1 persistence for every starting status,
context defaults, keyboard submit, busy lock and label-error retry without duplicate
creation. All external data in checks is local fixture data.

One Impeccable critique/polish pass used independent design and detector reviews.
The design assessment scored 32/40 before polish, with no P0/P1 issues. Resolved:
Escape focus restoration, disabled close while busy, partial-save Close wording,
platform shortcut hints, and field boundaries (existing token mix, measured above
3:1). The detector reported zero findings; it does not establish Rust parser
coverage or accessibility compliance. Rendered browser assertions supply the
interaction evidence. No palette or typography replacement was introduced.

Local harness note: these jobs share Cargo's native output directory. Another
worktree can replace `debug/xtask` even when Cargo considers this package fresh.
For the final runs, rebuilt this worktree's harness with
`cargo rustc -p xtask --bin xtask -- --cfg tik39_harness`, copied it to the ignored
`target/tik39-xtask`, and invoked the same xtask subcommands from that stable copy.
