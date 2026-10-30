# TIK-40 — one Impeccable critique and polish pass

Two independent assessments: design review and detector/evidence review. The
visual review covered the sidebar, open account menu, all four Settings sections,
and desktop/mobile layouts. The pinned black/Inter/240px design contract wins.

Design score: 31/40 before polish. Strengths were compact navigation, distinct
Settings destinations, clear account save feedback and project scanability.

Resolved findings:
- Mobile Settings navigation now uses intrinsic row height instead of stretching.
- The fixture Home route is separate, so Home and Board are not both selected.
- An all-archived project list has an explicit empty state.
- Display-name hints/errors are associated with the field; account loading has
  status and retry feedback. Project links expose active state and open counts.
- Chrome's offscreen crop path blended top-layer popup content. Component captures
  now await finished animations and capture within the viewport; the final PNG
  shows the opaque, readable menu at 2× DPR.

The mechanical detector ran once with telemetry disabled and returned `[]`.
It does not establish Rust/Leptos coverage or prove contrast compliance. No live
browser overlay was claimed. The Rust CDP harness supplied rendered evidence and
interaction checks. Screenshot servers and browsers clean up their owned children.

Motion review:

| Before | After |
| --- | --- |
| No section controls or account menu | Shared anchored 180ms menu; interruptible 200ms disclosure rotation |
| System motion preference only | Browser override removes movement and keeps subtle fades |
| Static account actions in the main navigation | Menu uses shared presence, focus return and keyboard behavior |

Questions skipped: the ticket supplied exact acceptance criteria and requested
one focused critique/polish pass. No additional design scope was introduced.
