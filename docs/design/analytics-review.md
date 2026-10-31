Method: dual-agent (A: /root/critique_design · B: /root/critique_evidence)

Design specificity: strong; ticket flow, current inventory and actionable Home queues
serve the product's workflow within the pinned dark Geist system.

| Heuristic | Score | Evidence |
|---|---:|---|
| Status | 3 | Timestamp, settlement gate, retained refresh data |
| Real-world match | 3 | Ticket language; coverage enum terms need translation |
| Control | 3 | Drilldowns/return paths; preserve project scope |
| Consistency | 3 | Shared shell, palette and chart frame |
| Prevention | 3 | Zero versus unavailable respected; partial cue missing |
| Recognition | 3 | Legends/tables; mobile scroll undiscoverable |
| Efficiency | 3 | Keyboard marks, linked counts |
| Minimalism | 3 | Two focused charts; fragmented mobile toolbar |
| Recovery | 3 | Retry keeps saved data |
| Help | 2 | Coverage note needs clearer labels |
| Total | 29/40 | Good |

Strengths: actionable Home queues, shared zero-based scales, exact tables,
retained data on refresh failure. Cognitive load is low; chart series and
existing grouped navigation necessarily exceed four items. The investigation
ends in linked, verifiable data. Hidden recent weeks create mobile uncertainty.

Priority issues (all P2): tooltip paint order obscures values; mobile hides recent
weeks without a cue; cross-page links lose project scope; coverage wording and
partial-week marks need clarification. P3: mobile toolbar refresh orphan; long
names and growing inventory need resilient geometry.

Persona risks: power users lose scope, keyboard users see obscured exact values,
mobile users miss current data. Fix through the already-authorized polish pass.

Detector once: zero source findings. Browser detector generic font/type findings
are intentional under DESIGN.md. Fresh live routes and screenshot evidence
confirmed defects. Muted text contrast 6.20:1, axes 13.79:1, focus 4.28:1.
Questions skipped: user already authorized this scoped critique and polish.

## Polish resolution

The single polish pass moves active values into a top HTML tooltip layer with
Escape dismissal, fits complete plots to mobile widths with readable SVG labels,
preserves project scope in both cross-page links, translates coverage labels and
marks partial capture weeks with `*`. Long axis names truncate while full names
remain in tooltips/tables; inventory height grows with its row count. The mobile
toolbar uses a deliberate second period row, keeping refresh next to freshness.
Chart skeletons use the same titles, subtitles and responsive plot proportions.

Run notes: source detector ran once with no findings. Independent browser
visualization succeeded, with no user-visible browser presentation tool. Generic
compact-type/Geist browser warnings are intentional under DESIGN.md. The review
used fixture-only live routes and screenshots; temporary browser/live-server
processes were cleaned up. No whole-app contrast or screen-reader certification
is claimed. The final browser check verifies the corrected interactions, layout,
project continuity and mobile plot widths.
