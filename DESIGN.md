# Tickets design contract

Calum's taste overrides Impeccable defaults. Preserve the existing dense,
Linear-style dark-only interface. Ignore system colour preferences; there is no
theme setting. Near-pure-black surfaces are intentional; do not lift them to
generic charcoal. Inter is approved: do not flag or replace it.
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
| Blocked dependency | `#f07856` |
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
Motion acknowledges input and preserves continuity. Enter/exit use
`cubic-bezier(.23, 1, .32, 1)`; on-screen movement uses
`cubic-bezier(.77, 0, .175, 1)`. Press / popover / modal / exit / tooltip / movement
are 120 / 180 / 240 / 120 / 125 / 200ms. Never use ease-in or scale from zero.
Modals fade and scale from .97 while staying centered; anchored surfaces use the
trigger's origin and flip above it near the viewport bottom. Exits are shorter.
Presence retains content through exit; rapid reopen cancels pending removal.
Animate transform and opacity only. Pressables scale to .97; hover rules require
`(hover:hover) and (pointer:fine)`. Keyboard-driven frequent actions are immediate.
Loading uses the published ascii-reveal package: a scan skeleton sweep while
waiting, then a fast decode reveal (about 300–500ms) as data arrives. Reduced motion
removes both effects: static skeletons and immediate content. Other interactions
retain their short opacity and color feedback.

Shared Select/DropdownMenu/Combobox replace browser selects. Keep the compact
32px controls, clear focus rings, named options and saved-state authority. Use
project glyphs from the shared SVG family or a user-selected emoji; absent icons
keep a colored initial tile. These foundations supersede the previous native
select and single-easing contract (TIK-38).

## Accessibility

Require WCAG AA: at least 4.5:1 contrast for normal text and 3:1 for large text
and meaningful UI boundaries, icons and focus indicators. Measure rendered
foreground/background pairs, including alpha-composited label tints and states;
a token's presence is not proof of compliance. Preserve semantic controls,
visible keyboard focus and non-colour status cues. Report any existing contrast
failure and fix it within the authorized UI scope while preserving this taste.

## Token enforcement

New UI spacing, radius, colour, font size and shadow values must use
`var(--token)`. Define tokens only in `crates/web/style/tokens.css`; prefer an
existing semantic role to another literal. This includes logical margin/padding
and inset properties, gaps, scroll spacing, letter/word spacing, radius longhands,
colour shorthands (background, border, outline, text decoration), SVG fill/stroke,
font shorthand and both box/text shadows. Local custom properties cannot hide
raw lengths or colours. Raw border/outline widths (including 1px borders),
structural width/height/grid geometry, line height, SVG geometry, animation timing
and media-query breakpoints are outside this lint's scope; their colours and
spacing still use tokens.

Allowed values include zero, CSS inheritance/reset keywords, `auto`, `none`,
`normal`, `currentColor`, `transparent` and relative percentages (including 50%
round radii). `calc`/`min`/`max`/`clamp` may combine tokens, percentages and
unitless multipliers, but may not introduce raw lengths. `color-mix` and gradients
must derive colours from tokens. `var()` fallbacks obey the same rules. Safe-area
insets may use `env(safe-area-inset-*)`. These are syntax allowances, not a reason
to replace a semantic role with an unrelated token of the same current value.

Rust literals are scanned inside `view!`, other macros, classes and `format!`,
including raw strings and formatted pixel units. Static `style=` / `style:`
values use the CSS policy. Runtime style bindings are explicit reviewed debt;
put new static styling in CSS classes. Existing API/user colour data, fixture
examples and measured geometry are retained in the baseline, not rewritten as
CSS variables in persisted API data. The lint is a source guard, not Rust dataflow
analysis: review constructed strings, JavaScript DOM geometry and API colour
boundaries too.

To add a token, choose a semantic name, define its dark value in `tokens.css`,
document the role here or in `docs/design-system.md`,
and use `var(--name)` at the call site. Run `npm ci --prefix crates/web`, then
`cargo xtask check design-tokens` (or `npm run lint:css --prefix crates/web` for
CSS only). Do not use Stylelint disable comments. Fixing existing debt requires
removing its entry/count from `xtask/lint-allowlist.json`; that file must exactly
match remaining findings and must never grow. See CONTRIBUTING.md for the ratchet.
