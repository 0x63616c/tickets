# Tickets icons

The Tickets mark is a notched ticket stub with a perforation line (TIK-55).

- `favicon.svg`: master tile on the 32px grid (2px perforation holes, notch r=3).
- `favicon.ico`: 16/32/48. The 16px frame is drawn from a separate hinted
  16px-grid path (1px holes on whole pixels, 9-row stub) so it stays crisp.
- `favicon-32.png`, `apple-touch-icon.png` (180, full bleed), `icon-192.png`,
  `icon-512.png`, `icon-maskable-512.png` (glyph at 80% for the safe zone),
  listed in `site.webmanifest`.
- In the app the same hinted 16px glyph is the `BrandMark` component
  (`crates/web/src/components/icons.rs`): a #ededed stub on a black (#000)
  rounded tile in every theme. Every icon uses a black background, never purple.

Regenerate everything with `python3 build.py` (needs `cairosvg` and `Pillow`).
