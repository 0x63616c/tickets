# TIK-65: Home loading continuity

Before assets: `07343d7`. After source: TIK-65 based on `d103655`. `ada33c3` was merged after capture;
it only changes unrelated board/list blocked indicators.
All evidence uses sample projects/tickets in an isolated local Worker/D1; no production data.

`before-frames.png` and `after-frames.png` contain the first four seconds of a cold
Home navigation, captured at a 50ms target interval. Read left to right, then down.
`transitions.txt` summarizes counts, actual cadence and timestamps; JSON files retain
request completion times and DOM loader mount/unmount events. Page-wide decode is
counted as loading, since this is what visually obscured the page. The toolbar's
small refresh indicator is excluded. Frame capture itself can take longer than 50ms.

The skeleton unmounted once in both builds. Live room opening scheduled catch-up,
whose busy→ready cycle replayed Home's page-wide ASCII decode. Before has three
loading/content transitions; after has exactly one. Analytics and project reads
now start together, and the initial gate waits for both. Later requests keep content.
Query navigation also retains the current `/` or `/home` alias to avoid a remount.

`before-home.png` / `after-home.png` show the stable desktop page; the additional
mobile capture shows the selected project at 390×844. Layout and charts are unchanged.

Local build note: parallel worktrees overwrite shared Trunk/Wasm aliases. The final
local assets were packaged from this worktree's fingerprinted release Wasm with
wasm-bindgen 0.2.129 and wasm-opt 123; the harness reused those built assets with
`TICKETS_SKIP_BUILD=1`. Source checks used `RUSTC_WRAPPER=` and
`RUSTFLAGS='-C debuginfo=1'` to avoid stale shared artifact hashes. CI builds normally.

One Impeccable critique/polish pass: two independent assessments, zero detector
findings, no additional in-scope visual defects. Dark identity and chart placement
preserved. Existing loading-check retains shared reduced-motion coverage.
