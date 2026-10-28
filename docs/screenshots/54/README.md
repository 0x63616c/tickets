# Issue 54 visual and contrast evidence

Before assets: origin/main `5e35985` built in `/tmp/tickets-54-before`.
The temporary worktree was removed after capture. Both phases use the real local
Worker routes, identical seeded ticket/label data, bundled fonts loaded, dark
scheme, desktop 1440×900 and mobile 390×844. No production access.

Desktop filenames end in `desktop-rest` or `desktop-hover`. Mobile captures use
`mobile-rest`; `ticket-properties` scrolls the real detail route to show labels.
Board columns intentionally scroll internally on mobile; list metadata follows
the existing mobile hiding rules. Every committed PNG was opened and inspected.
The sidebar stays 240px on desktop; rows and cards retain their geometry.

## Reproduce

Run `CHROME_BIN=<headless-chromium> CARGO_TARGET_DIR=target CARGO_BUILD_JOBS=4 cargo xtask contrast-check`.
Set `TICKETS_BEFORE_ASSETS` to the absolute dist directory built from origin/main
for the before phase. Do not run builds or port-8795 browser checks concurrently.

## Measurements

`contrast-pairs.json` records computed foreground/background sRGB channels,
font size and opacity for each route/state. The xtask composites transparent
backgrounds over every ancestor before computing WCAG relative luminance:
sRGB channels ≤0.04045 divide by 12.92; others use ((channel+0.055)/1.055)^2.4;
L = 0.2126R + 0.7152G + 0.0722B; ratio = (lighter L+0.05)/(darker L+0.05).
Color-mix channels are converted from 0–1 sRGB; backgrounds are not rounded
before measurement. Ancestor opacity must be 1 (explicitly asserted).

Primary text and primary shortcut foreground is #f7f8f8:

| State | Background | Ratio |
| --- | --- | ---: |
| Rest / keyboard focus | #535fc4 | 5.19:1 |
| Hover | #4854b8 | 6.10:1 |
| Active | #414dab | 6.87:1 |
| Disabled | #343b70 | 9.84:1 |

The shortcut border now uses currentColor, with the same ratios as its text.
The unchanged offset focus ring #5e6ad2 has at least 4.21:1 against the four
base surfaces (#000000, #030303, #070708, #0a0a0b), above the 3:1 focus threshold.
Disabled primary controls use full opacity, including their shortcut child.

All unique measured text pairs are listed below (sRGB channels 0–255). These
include unchanged label tints, neutral shortcuts and metadata as regression
coverage; repeated route/state observations are retained in the JSON.

| Text role(s) | Foreground RGB | Composited background RGB | Ratio |
| --- | --- | --- | ---: |
| KBD, resolved | (138, 143, 152) | (0, 0, 0) | 6.46:1 |
| KBD, agent-badge, comment-count | (138, 143, 152) | (7, 7, 8) | 6.20:1 |
| SPAN | (175.35, 215.4, 253.65) | (10.92, 23.38, 35.28) | 12.04:1 |
| SPAN | (175.35, 215.4, 253.65) | (16.94, 29.4, 42.16) | 11.33:1 |
| SPAN | (202.8, 198.75, 237.9) | (19.46, 18.2, 30.38) | 11.37:1 |
| SPAN | (202.8, 198.75, 237.9) | (22.04, 20.78, 32.96) | 11.12:1 |
| SPAN | (202.8, 198.75, 237.9) | (25.48, 24.22, 37.26) | 10.74:1 |
| avatar | (208, 214, 224) | (10, 10, 11) | 13.55:1 |
| project-mark | (208, 214, 224) | (15.3, 15.3, 15.3) | 13.09:1 |
| SPAN | (229.35, 212.7, 223.5) | (27.72, 22.54, 25.9) | 12.60:1 |
| SPAN | (229.35, 212.7, 223.5) | (33.74, 28.56, 32.78) | 11.82:1 |
| SPAN | (246.0, 179.4, 179.4) | (32.9, 12.18, 12.18) | 10.73:1 |
| SPAN | (246.0, 179.4, 179.4) | (38.92, 18.2, 19.06) | 10.19:1 |
| KBD, button button-primary | (247, 248, 248) | (52, 59, 112) | 9.84:1 |
| KBD, button button-primary | (247, 248, 248) | (62, 70, 138) | 8.06:1 |
| KBD, button button-primary | (247, 248, 248) | (65, 77, 171) | 6.87:1 |
| KBD, button button-primary | (247, 248, 248) | (72, 84, 184) | 6.10:1 |
| KBD, button button-primary | (247, 248, 248) | (83, 95, 196) | 5.19:1 |

## Heading and polish audit

`contrast-check` asserts a page h1 and no skipped heading level on `/`, `/list`,
`/ticket`, `/login`, `/projects`, `/settings/tokens`, `/design`, `/design/board`,
`/design/list`, `/design/ticket`, `/design/empty` and `/design/projects`.
There is no separate settings route: API tokens is the settings screen.
Markdown headings compress source-level gaps; their shared class preserves the
existing 13px acceptance-heading appearance. A core regression test covers
starting at h3, nesting directly to h6 and returning to shallower sections.

Impeccable polish: reviewed shared typography, state contrast, label tints,
sidebar geometry, compact rows, mobile overflow and all screenshot pairs against
DESIGN.md. No remaining defects in the changed scope. The telemetry-disabled
source detector returned `[]`; Rust view macros are not a substitute for browser
checks, so rendered WCAG/heading assertions and visual inspection are the evidence.
The optional formal two-agent critique was omitted; polish and deterministic
browser checks provide the review evidence.
