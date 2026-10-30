# TIK-59 — board creation actions

Before assets: `cd30ecc67378c79a47a9649e1ce727ae180cb6ac` (synced origin/main,
including TIK-52), built in `/tmp/tik53-baseline`. After assets: `9c5d634` (the original TIK-59
implementation before the subsequent main sync). This pair isolates TIK-59 from
independently merged shell and branding changes.
Both use the same in-memory `/design/board` fixtures, dark scheme, bundled fonts,
and 1440×900 / 390×900 viewports. No production data or services are used.

- `before/after-board-topbar-{1440,390}.png`: tightly cropped topbar, 2× DPR.
- `before/after-board-{1440,390}.png`: full-page context, 1× DPR.

Reproduce after building:

```sh
CHROME_BIN=/workspace/tickets-polish/chrome-noproxy cargo xtask component-shot board-topbar --output docs/screenshots/TIK-59
```

For before captures, add `--before-assets <absolute-base-dist>`.
The harness asserts the topbar remains 44px and the document does not overflow,
and checks fixture list, ticket and Home actions remain available. Pixel comparison
shows the full-page content below the topbar is identical before and after at
both widths (desktop y ≥ 44; mobile y ≥ 88, including its mobile navigation bar).

One scoped Impeccable critique/polish pass: the board's global creation action
is redundant beside status-specific column controls. Remove its orphaned divider
alongside the button, retain the existing shell geometry and connection state,
and preserve the C shortcut. Source scans of the board and shared shell returned
no findings. Rendered desktop/mobile evidence is inspected with no broader restyling.
The real `board-check` verifies all six status defaults and persistence, C → Todo
while In review's + is focused, and working creation actions on list/detail/Home.
