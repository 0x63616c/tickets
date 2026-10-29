# Dual-agent Impeccable critique — TIK-38 foundations

Assessment A reviewed design independently; Assessment B ran the detector and fresh browser inspection. A completed before B findings were read. Target: crates/web/src/components. Scope: shared controls, loading, motion and project identity; page redesigns remain separate tickets.

The foundation belongs to Tickets: dense geometry, near-black surfaces, restrained Inter hierarchy and familiar status shapes preserve the pinned design. Static skeletons, shorter exits and fixed busy-button footprints provide quiet feedback. Glyph, emoji and initial project marks share one identity system.

| Nielsen heuristic | Score / 4 |
|---|---:|
| System status | 3 |
| Real-world match | 4 |
| Control and freedom | 3 |
| Consistency | 3 |
| Error prevention | 3 |
| Recognition | 3 |
| Efficiency | 2 |
| Minimalism | 4 |
| Recovery | 3 |
| Help | 3 |
| Total | 31 / 40 |

Cognitive load is low. Six statuses exceed the four-choice heuristic, but are bounded domain choices; hiding valid states would hurt this operating interface. Compact controls reassure on entry; lost search focus and replacing saved content with skeletons create the main emotional valleys.

## Priority issues and polish decisions

1. P1: Both assessments reproduced Combobox opening on selected Feature, leaving search inaccessible. Focus search first and return to it with Arrow Up from the first option; add keyboard regression coverage.
2. P2: B reproduced modal close returning focus to BODY. Restore opener after native dialog removal releases inertness, preserving interrupted exits.
3. P2: A found skeletons returning during foreground refresh with existing content. Track successful initial load; retain saved content during refresh.
4. P2: A reproduced tooltip drift after scrolling. Share the popup scroll/resize watcher and make the trigger wrapper intrinsic width.
5. P2: A found search's focus outline removed. Give its enclosing search region a visible focus-within ring.

A also found a documentation mismatch around exact label matches. Preserve the existing tested create-or-attach behavior (Enter on an already attached label is idempotent), document it explicitly, and honor busy guards. Selecting an option continues to toggle. Reactive icon option colors now follow the live picker color.

Suggested command for these scoped corrections: impeccable polish. These are implementation defects within the approved work, not requests to reshape pages.

## Personas

Alex, the power user, loses efficiency when typing fails to reach label search. Sam, using keyboard navigation, needs search access, visible focus and reliable opener restoration. Casey, using a narrow touch viewport, benefits from the inspected 390px layouts without page overflow.

## Evidence

The source detector returned [] (zero findings, exit 0). Browser detect.js executed on /design and isolated select, combobox, modal and busy-button routes; each logged no anti-patterns and no runtime exceptions. Rust/Leptos limits the static detector's semantic coverage, so the manual findings take precedence. Black and Inter follow DESIGN.md. Known stale assets were excluded from new findings.

Questions skipped: the user already authorized implementation and polish; all decisions above follow the pinned design and task scope.

## Polish and verification follow-up

The scoped pass corrected search focus/return, native-dialog opener restoration,
initial-load skeleton gating, tooltip scroll positioning, search focus indication,
and reactive icon colors. Browser regression checks exercise these paths. During
integration, pending focus work and tooltip removal events were guarded against
disposed Leptos owners, and project initials gained readable text contrast. Route snapshot hit testing,
isolated gallery navigation and closed-select Home/End were also verified.

Run accounting: no ignore list; A/B isolated; detector invoked once, no findings;
five browser injections succeeded in fresh headless pages. No visible overlay is
claimed. Detector server and owned browser sessions were stopped. The report body
temporary file was removed; the snapshot was written and its first-run trend read
(31/40; no earlier trend). The target was a directory; storage marked the snapshot closed on its latest
lookup and returned no active snapshot. Polish continued through an independent
pass. PRODUCT.md schema drift was reported by context and left
unchanged because repairing it is outside this ticket.

