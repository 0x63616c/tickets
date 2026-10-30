Method: dual-agent (A: `/root/loading_design_review` · B: `/root/loading_detector_review`)

# TIK-47 critique and polish

Assessment A reviewed source and rendered routes without detector results.
Assessment B ran the detector and independent browser evidence without reading
A's report. The parent combined both only after A had finished. The final polish
preserves the pinned black/Inter interface and its existing content hierarchy.

## Assessment A

The loading treatment fits Tickets' dense working interface. Placeholders reserve
space in familiar board/list/detail/project structures, and the short decode
returns attention to real ticket text. No additional user decisions are introduced.
The uncertain loading interval has explicit feedback; arrival remains interactive.

| Heuristic | Score | Observation before polish |
| --- | ---: | --- |
| System status | 3 | Generic labels and temporary zero counts needed refinement. |
| Real-world match | 4 | Loading geometry follows the ticket surfaces. |
| User control | 4 | Navigation and reduced-motion controls remain available. |
| Consistency | 3 | Avoid overlapping page and button reveal controllers. |
| Error prevention | 3 | Busy controls retain footprint and prevent duplicate input. |
| Recognition | 4 | Ready content uses the existing labels and layout. |
| Efficiency | 4 | Observed decodes completed in roughly 415–457 ms. |
| Minimalist design | 4 | Existing near-black tokens remain authoritative. |
| Error recovery | 3 | Existing retry and draft preservation paths retained. |
| Help/documentation | 3 | Shared component and build conventions documented. |
| **Total** | **35/40** | **Strong; focused polish required.** |

Keyboard and mobile users retain their existing controls; motion-sensitive users
get static placeholders and instant content. Decorative glyphs stay out of the
accessibility tree. Actual screen-reader software was not exercised.

## Findings resolved in polish

- **P2, overlapping reveals:** account saves now reveal the submit label only.
  Detail writes and live reconciliation reveal newly mounted comment bodies
  instead of replaying the whole page; the page reveal runs only on first mount. The write captures that intent before reactive mounting, avoiding
  a race with clearing `busy`. Unchanged keyed comments retain their nodes.
- **P3, detail alignment:** the skeleton reuses `detail-layout`, `ticket-detail`
  and `properties` geometry instead of its own content inset.
- **P3, misleading counts:** board/list totals and the projects count stay hidden
  until their data is known.
- **P3, loading copy:** full-page statuses name tickets/projects; comment submits
  and deletes have action-specific labels.
- **P3, reveal fixture:** added a named loading status, a 16px token-based content
  heading and spacing between the description and replay control.

No P0/P1 findings. The parent rechecked the final assets with the loading and
component acceptance commands. Small-save behavior is covered by the real local
detail acceptance path. Broader product redesign was outside this task.

## Assessment B and run notes

Target slug: `crates-web-src`; no critique ignore list exists. The one-shot CLI
scan exited 0 with `[]` (zero findings). Rust `view!` markup is outside the
detector's built-in extensions, so this is limited evidence, not a clean bill of
health. Independent DOM inspection supplied the missing coverage.

Browser overlays were injected successfully on board, list, detail, projects
and the reveal fixture. Three findings across two rules were reported:

- Board/list `tiny-text`: established 11px keyboard-help footer text, a deliberate
  auxiliary size pinned by the design system; no loading regression.
- Reveal fixture `flat-type-hierarchy`: its 13px sample heading lacked hierarchy;
  addressed by the 16px content heading in polish.

At 1440×900 and 390×844 all four product routes stayed within the viewport. Every
normal-motion route had moving package canvases while pending, decode markers
on arrival, and neither after settling. Both reduced-motion paths stopped active
sweeps and skipped decode. The loading caption's measured contrast was 6.46:1.
There were zero uncaught browser exceptions in the eight route cases and fixture.

The browser was headless: there was no user-visible `[Human]` tab. Overlay
screenshots, console output, accessibility snapshots and DOM measurements were
the fallback evidence. WebSocket handshake errors came from the read-only fixture
server, and Chromium reported preload-integrity warnings; no asset failed to load.
These are not assertions of a full WCAG or cross-browser audit.

All Impeccable launchers used `IMPECCABLE_NO_TELEMETRY=1 DO_NOT_TRACK=1` and the
project cache. The owned live-server was stopped; temporary overlay injection,
browser processes and its nested runtime directory were removed. No hooks were
installed. The context command noted legacy PRODUCT.md metadata; that unrelated
migration was left untouched. Questions skipped: no unresolved priority issues
remained after the authorized polish work.
