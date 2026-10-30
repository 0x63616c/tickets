# TIK-83 font preference evidence

Fixture-only `/design/settings/appearance` and `/design/board`, dark only.
Before captures use unmodified web assets from main at
`b8e85d18450f09a9966d963dcd72ed56b7fa9760`.

`cargo xtask font-check` captures each font at 1440×900 plus default Geist
Settings at 390×844. It selects fonts through the shared Select, verifies distinct
computed body families and preview inheritance, reload persistence, HTML-only
boot (app JS/Wasm blocked), invariant code/mono font, invalid preferences,
unavailable storage and mobile overflow. No live accounts or tokens are used.

The four faces retain the existing type sizes and tracking. Geist Mono is wider
by design; the sample title wraps naturally on mobile.

Captures use a fresh Chromium profile per page and allow the loaded fonts and
compositor to settle. The before refresh uses the clean main snapshot at the
same SHA in `/workspace/wt/tickets/TIK-77-before`; it has no Font picker or Geist
sans asset. Original-resolution PNG inspection confirms the shell glyphs.

One Impeccable critique + polish pass: independent design and detector/evidence
assessments, 32/36 (Good), zero priority issues and zero detector findings.
Existing sizes and tracking were retained after desktop/mobile inspection.
