# Home critique and polish

Target: `crates/web/src/pages/home.rs`, `crates/web/style/home.css`.
Mode: Operate. DESIGN.md and the trimmed Home contract govern the review.

Impeccable 4.8.0 context ran with telemetry disabled and the project cache.
Two independent assessments reviewed the first valid desktop/mobile/chart-table
captures. Assessment A completed before Assessment B's findings were synthesized.
The detector ran once with both changed UI targets, exited 0 and returned `[]`
(`detector.json`). This is not evidence that the detector understands every
Leptos template. No browser overlay was injected or claimed.

Assessment A scored the initial version 29/40. Strengths: compact useful metrics,
consistent existing tokens/icons, and explicit unavailable history. Findings and
polish:

| Finding | Applied change |
| --- | --- |
| Empty workspace buries the first action below charts | Empty Home now immediately offers New project and omits empty analytics. |
| Cohort loses selected week/status | Details name the UTC week or inventory status. |
| Initial error retains loading skeleton | Settled initial failures show the error/retry state; refresh errors retain saved data. |
| Mobile SVG labels shrink too far | Compact viewBox and sparse date ticks preserve readable labels; the table lists every week. The harness checks date-label separation at both viewport sizes. |
| Counts disclosure touches the total | Added spacing, a 24px hit area and project-specific accessible labels. |
| Mobile hides update time | Retained compact UTC freshness with wrapping toolbar. |

The implementation also fixes query serialization found by the browser harness.
Native disclosures and chart tables provide keyboard access and focus rings;
chart geometry and count changes do not animate. Density, Inter, near-black
surfaces and quiet decorative dividers are deliberate pinned choices.

Validation uses the fixture CDP harness and local D1 checks, with final screenshot
and state results recorded in README.md. Assessment B inspected saved renders and
source while the parent owned the browser; it did not run an overlay or independent
live browser session. No temporary review server remains running.

Questions skipped: the user supplied the product/design contract and authorized
implementation and trimming; these were local defects, not new product choices.

Final Assessment A reviewed all seven requested desktop/mobile/state captures:
**Ship — no remaining visual blockers.** It confirmed the visible empty action,
settled and retained-data errors, skeleton composition, readable mobile chart,
exact-count table/focus, and separated count disclosures. Cohort week/status
context is verified by the parent browser checks, not inferred from these images.
