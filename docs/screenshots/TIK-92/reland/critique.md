# Board drag critique and polish

Dual-agent critique: independent Assessment A (design/source/rendered images) and
Assessment B (source detector/rendered browser evidence). Target:
`crates/web/src/pages/board.rs`. Operate mode, existing DESIGN.md contract.

Design specificity: dense black shell, compact metadata, restrained purple
insertion slot and original-size preview preserve Tickets' existing identity.
Cognitive load remains low at rest and moderate during a gesture: readable preview,
gray source copy and empty purple destination have distinct meanings. The anchored
source reduces uncertainty; cancellation and saved-state feedback end the flow.

Heuristics in order (status, real-world match, control, consistency, prevention,
recognition, efficiency, minimalism, recovery, help): 3,4,4,3,4,3,4,4,3,3 = **35/40**.
No P0/P1/P2 issue in this repair. Strengths: original card geometry/content retained;
source remains spatially anchored; keyboard/status-menu alternatives preserved;
decorative clones are excluded from focus/accessibility and pointer targets.

P3 observations, existing/outside this narrow repair: mobile move guidance mentions
Escape; compact status controls deserve a separate touch-target audit. Preserve
the visual distinction among preview, origin and insertion slot without adding
further decoration. Desktop/mobile fixture captures use sample data and are not
matching interaction before/after pairs.

Assessment B: detector ran once with telemetry disabled, exit 0, JSON `[]`:
0 findings, 0 rules, 0 locations, 0 false positives. No ignore file. Disabling
preview transitions is intentional direct manipulation; the .35-opacity source
is decorative rather than independently readable content. No overlay injected,
no visible persistent browser or critique server. Rendered PNGs and recorded
interaction geometry were used. Its initial evidence snapshot preceded the final
Chromium harness retry; use the final `smoke.log` for browser acceptance.

Final source follow-up confirmed the relocation of drag-aware motion into
the versioned module preserves the same interaction. Legacy `dom.js`
is byte-identical to main; popup imports retain their existing path.

Polish: inspected desktop/mobile board/component frames and complete gesture
states. Existing dark tokens, card dimensions, readable preview, faint source,
focused keyboard path, cleanup/cancel semantics and immediate tracking are
preserved. No further product styling changes were needed. Browser acceptance
and final checks are recorded separately in README.md.

Questions skipped: narrowly authorized repair with an established design contract;
no product direction decision needed.
