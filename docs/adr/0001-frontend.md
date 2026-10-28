# ADR 0001: Leptos CSR and a small CSS design system

- Status: Accepted; framework locked by the project owner
- Date: 2026-10-27
- Scope: #6, with implementation consumers #7–#10 and #16

## Decision

Use **Leptos** in client-side rendering (CSR) mode. Trunk builds the Rust frontend
into WebAssembly, its generated JavaScript loader, and static CSS/font assets.
The Cloudflare Worker serves those assets through `ASSETS`; SPA fallback resolves
client routes while `/api` and `/api/*` remain Worker routes. No SSR service,
JavaScript application framework, or separate frontend host is introduced.

Use **plain CSS with custom-property design tokens**. `style/tokens.css` owns the
palette, typography, spacing, radii, elevation, and motion. `style.css` owns
component and responsive layout rules. Leptos components own semantics and
interaction. No heavy CSS framework is needed for this deliberately small system.

## Why

- **Rust throughout.** Leptos fits the existing Rust workspace and permits shared
  domain types without adding a second application language. Runtime code is
  gated on wasm32; pure Markdown rendering is tested on the host.
- **UI quality remains under our control.** Semantic HTML, inline SVG, precise CSS,
  and Leptos signals support the required dense board, list, and detail layouts.
  The framework does not dictate the visual design. Small property controls,
  visible creation actions, and keyboard alternatives are component contracts.
- **Simple hosting.** CSR needs only static assets plus the existing Worker API.
  This authenticated ticket tracker does not need server-rendered search-engine
  pages. SSR/hydration complexity has no established benefit for this scope.
- **Measured size tradeoff.** Rust/WASM adds a download and initialization cost.
  Leptos does not make that free. The release profile uses size optimization and
  LTO, and Trunk runs `wasm-opt -Oz`. Record actual release sizes in
  `docs/design-system.md`; debug WASM is not a shipping-size measurement. The
  preview includes routing and the Markdown parser/sanitizer, so measure again
  as API integration replaces fixtures. Avoid large component libraries and
  additional runtimes; profile before adding lazy modules or changing rendering.
- **A small CSS surface.** Native controls and a single stroke icon family cover
  the current interactions. A utility framework or large widget library would
  add conventions and overrides without improving this specific design. Revisit
  only if a concrete, repeated accessibility or maintenance need justifies it.

React and Vue would violate the Rust frontend constraint. Yew and other Rust UI
frameworks do not justify revisiting the locked Leptos decision. SSR is deferred,
not part of the initial architecture.

## Visual contract

Use the supplied Linear reference pack: `#08090a` canvas, `#0f1011` sidebar,
`#141516` / `#191a1b` surfaces, `#23252a` hairlines, `#f7f8f8` primary text, and
`#5e6ad2` as the single action/focus accent. Status, priority, and label colors
appear in small icons/dots. No colored column fills or decorative shadows.

Inter Variable is self-hosted under the SIL Open Font License, with Geist Mono
for IDs/code. Retain the licenses in `crates/web/fonts/`. UI text is 13px, with
510 row weight and 590 headings; secondary text is 12px. Use a 4px spacing grid,
6px controls, 8px cards/popovers, and 12px dialogs. The 228px sidebar has 28px rows;
headers are 44px and list rows 36px. Shadows belong to floating overlays only.
Hover/focus changes take 100–150ms; reduced motion disables transitions.

## Interaction and security contract

- Board/List are visible tabs. New ticket and New project are visible actions.
  Dragging has a native status-select alternative. No hidden workflow modes.
- Native dialogs contain focus, make the background inert, close on Escape, and
  focus their first input. Command search opens with Cmd/Ctrl+K; C creates a ticket
  outside editable fields. Menus and controls remain usable with Tab/Enter.
- Ticket descriptions and comments use the **same** `pulldown-cmark` → `ammonia`
  function. Only the sanitized output reaches `inner_html`. GFM task checkboxes
  are forced disabled; raw forms, scripts, events, and unsafe URLs are removed.
- The connection indicator has connected, reconnecting, and offline states with
  text and distinct icons. #16 will drive it from the actual WebSocket state;
  the preview does not open a connection or promise offline write persistence.
- Day one remains a personal workspace. Avatars identify activity authors, not
  an invitation or assignee management system. No sign-up or multi-user controls.

## Consequences

`/` uses the new shell; `/design` and its reference routes use fixture data.
Preview changes reset on reload and are identified by the sidebar's Sample data
notice. They are reference interactions, not API/authentication implementations.
Sol should replace `DesignState` with API resources while retaining the visual
and interaction contracts. The API must independently enforce dependency cycles,
authorization, validation, and persistence. See `docs/design-system.md` for the
route inventory, screenshot evidence, and handoff details.
