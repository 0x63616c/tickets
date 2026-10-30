# TIK-58: whole list labels

Baseline: `3ab9a59883b39831b54d55fe656b0cd0b77bb226` (origin/main after the requested merge).

Before/after use the same local fixture server, `/list?project=1`, bundled fonts,
dark theme, and four sample tickets. No production data or credentials appear.
The many-label row has `area:ui`, `model:sol`, `model:astra`, and
`area:performance`; legacy `priority:p1` remains hidden.

- `*-list-desktop.png`: full page, 1280 × 900.
- `*-list-narrow.png`: full page, 1100 × 900.
- `*-list-mobile.png`: full page, 390 × 900 (existing optional-label hiding).
- `*-labels-*.png`: tightly cropped 2× sample row.
- `after-tooltip-*.png`: shared overflow Tooltip opened by keyboard focus.

The existing fixture screenshot harness was used with this ticket's sample
labels and viewport sizes; its temporary capture configuration is not shipped.
`cargo xtask list-check` contains the durable regression: all-fit labels,
oversized label, complete chip bounds and counts, keyboard Tooltip, no horizontal
overflow, and a 1280 → 1100 → 1280 resize sequence.

One focused Impeccable critique and polish pass preserved the list's existing
visual language. The source detector returned no findings.
