# Impeccable critique and polish

Method: two agents, design review (`palette_design_review`) and detector/browser
evidence (`palette_detector_review`). Target: `crates/web/src/components/palette.rs`.
The review assessed the first working rendered implementation before the final
polish changes. B completed its measurements before accidentally seeing A's
finished report in agent status output; its measurements were not derived from A.

| Heuristic | Score / 4 | Observation before polish |
| --- | --- | --- |
| System status | 3 | Saved values and busy/error states are present |
| Real-world match | 3 | Familiar ticket keys, statuses and commands |
| Control and freedom | 3 | Escape returns a level and restores the query |
| Consistency | 3 | Nested input name and action hints need precision |
| Error prevention | 3 | Context stays visible; selection boundary is weak |
| Recognition | 3 | Filters are visible; their scope label needs updating |
| Efficiency | 4 | Keyboard path keeps input focus and supports actions |
| Aesthetics | 3 | Dense, restrained surface fits the pinned design |
| Error recovery | 3 | Errors preserve context and allow retry |
| Help | 2 | Nested empty copy and descriptions need specificity |
| Total | 30/40 | Good foundation with local accessibility defects |

The result fits Tickets: compact status glyphs, monospaced keys, shared icons,
near-black surfaces and predictable grouped commands. It preserves the project's
identity. Grouped choices and nested actions reduce cognitive load; the six-item
status list is a deliberate domain-sized exception to four-choice guidance.
Persistent ticket context and saved-value checks support confident edits.

Priority findings and polish:

1. **P1: selection boundary.** Its measured contrast was 1.11:1 against the row.
   Use the pinned focus token (measured 4.28:1) so keyboard selection is clear.
2. **P2: touch actions and tab order.** Tapping a result opened it immediately;
   a non-first result lacked its own action entry. Add per-row action buttons,
   44px mobile targets, and exclude the inert scroll region from Tab order.
3. **P2: nested semantics and short viewports.** Input names and footer verbs
   remained generic; at 390×400 the result minimum could clip the footer. Use
   contextual names/descriptions and Open/Choose/Set/Copy/Create/Toggle hints;
   let results shrink when height is constrained.
4. **P3: scope and empty copy.** Reflect `p:` in the scope label and use clear
   nested-choice empty instructions. Show the platform's action modifier.

Power users benefit from direct exact-key lookup and preserved input focus;
keyboard and screen-reader users need the stronger selection and accurate names;
touch users need the explicit per-ticket action path. Search/loading/error,
mutation failure/retry, reduced motion, mobile and constrained-height behavior
are covered by the palette checks.

CLI detector: one successful invocation returned no findings, but Rust is outside
its default source extensions; that result does not certify Leptos markup.
Browser injection succeeded and reported no anti-patterns. Manual browser
measurements supplied the findings above. The browser was headless: there is no
user-visible overlay. Agent browser and detector server processes were stopped.
No ignore list was present.

Questions skipped: the user already authorized implementation, critique and
polish within the palette scope; these findings require no new product decision.
