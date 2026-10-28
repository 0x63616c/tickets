# Tickets design contract

Calum's taste overrides Impeccable defaults. Preserve the existing dense,
Linear-style dark interface. Near-pure-black surfaces are intentional; do not
lift them to generic charcoal. Inter is approved: do not flag or replace it.
Label chips use backgrounds softly tinted in each label's own colour, with a
related border and lighter text. Preserve the 240px sidebar and shared shell.

The source of truth for implementation is `crates/web/style/tokens.css`;
`docs/design-system.md` and `/design` document the shared components. Reuse
semantic tokens and controls rather than introducing a parallel palette.

## Colours

| Role | Existing values |
| --- | --- |
| Background / sidebar | `#000000` / `#030303` |
| Surface / raised | `#070708` / `#0a0a0b` |
| Hover / active | `#ffffff0a` / `#ffffff0f` |
| Border / subtle / strong | `#141416` / `#ffffff08` / `#1f2024` |
| Text / secondary / muted / faint | `#f7f8f8` / `#d0d6e0` / `#8a8f98` / `#797d86` |
| Action fill / hover / active / disabled | `#535fc4` / `#4854b8` / `#414dab` / `#343b70` |
| Action text | `#f7f8f8` |
| Accent / hover / surface / focus | `#5e6ad2` / `#6d79df` / `#0d0e1f` / `#5e6ad2` |
| Backlog / Todo / In progress | `#8a8f98` / `#d0d6e0` / `#f0bf00` |
| In review / Done / Canceled | `#27a644` / `#5e6ad2` / `#8a8f98` |
| Urgent / high / medium / low priority | `#eb5757` / `#d0d6e0` / `#8a8f98` / `#8a8f98` |
| Success / warning / danger | `#27a644` / `#f0bf00` / `#eb5757` |
| Feature / bug / infra / design label | `#8b82d9` / `#eb5757` / `#4ea7fc` / `#c6a1b9` |

Filled primary actions use the action tokens, including shortcut text. Hover and
active fills darken; disabled actions retain full text opacity with a quieter fill.
Accent tokens remain for links, selection and status meaning.

Chip tint tokens: fill 14%, border 26%, text 55%. Derive these from the label's
own hue, including custom labels; avoid neutral grey chip fills.

## Typography

Use bundled Inter Variable with system sans fallbacks, and Geist Mono with
monospace fallbacks. Size tokens xs/sm/base/md/lg/xl/title are
11/12/13/14/16/20/24px. Medium/semibold/bold weights are 510/590/680; line height
is 1.5. Preserve compact hierarchy and readable metadata.

## Spacing, geometry and motion

Spacing tokens 1/2/3/4/5/6/8/10/12/16 map to
4/8/12/16/20/24/32/40/48/64px. Radii sm/md/lg/modal are 4/6/8/12px.
Sidebar width is 240px and topbar height 44px. Select horizontal padding is
12px, chevron size 14px. Popover shadow is `0 4px 24px #0003`; scrim is `#0009`.
Fast/normal/slow durations are 100/150/200ms with
`cubic-bezier(.25, .46, .45, .94)`. Reduced-motion preferences set all three
durations to zero.

## Accessibility

Require WCAG AA: at least 4.5:1 contrast for normal text and 3:1 for large text
and meaningful UI boundaries, icons and focus indicators. Measure rendered
foreground/background pairs, including alpha-composited label tints and states;
a token's presence is not proof of compliance. Preserve semantic controls,
visible keyboard focus and non-colour status cues. Report any existing contrast
failure and fix it within the authorized UI scope while preserving this taste.
