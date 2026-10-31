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
  rounded tile in the dark-only interface. Every icon uses a black background, never purple.

Regenerate everything with `python3 build.py` (needs `cairosvg` and `Pillow`).
Run it from this directory. It also copies `apple-touch-icon.png` to Safari's
implicit `apple-touch-icon-precomposed.png` path and updates/prints the URL token.

Icon URLs in `../index.html` (including the black Safari mask) and every manifest
icon use the same `?v=<token>` to invalidate Safari's URL-keyed favicon cache.
The token **must be bumped whenever icons change**. It is the first eight hex
digits of SHA-256 over the sorted shipped image filenames, each followed by a
NUL byte and its file contents (the `IMAGES` list in `version.py`). The manifest,
generator, documentation and unshipped previews are excluded to avoid a circular
hash and unrelated cache invalidation. Add new shipped images to that list.

After changing images manually, keep the precomposed copy identical and run
`python3 version.py --write`. Verify with `python3 version.py --check` (no extra
dependencies); this fails on stale/missing versioned links or a mismatched copy.
These commands also work from the repository root using
`python3 crates/web/icons/version.py --check` (or `--write`).
