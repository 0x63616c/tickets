# One palette (TIK-136)

`crates/web/style/tokens.css` is the only app color definition file. Tickets is
dark only, as required by DESIGN.md; this consolidation adds no light theme.
Use semantic CSS variables for rendering. `crate::colors::value` reads the same
source for fixed project/label choices sent to the hex-only API. It reads the
published default palette, never a browser accent override or a computed RGB value.
User-selected API colors remain validated data and may be rendered dynamically.

The existing neutral scale, status/priority meanings, action contrast and Stub
identity stay intact. Exact duplicate roles alias their shared color; white tint
mixing retains a dedicated white token so chips render identically. Project and
label choices remain fixed when the UI accent changes. `--info` shares the
infrastructure blue. Accent-picker implementation belongs to TIK-135.

`cargo xtask check raw-colors` runs in `cargo xtask ci`. It scans tracked and
unignored new text sources under `crates/web`, Worker sources, and the shared
Worker HTML renderer in `crates/core/src/github_app.rs`. CSS, Rust strings/macros,
HTML, SVG, JS/MJS, JSON/manifest and icon-generator Python are covered; comments,
CSS ID selectors, binary fonts/images and ignored generated/vendor outputs are
excluded. The existing design-token lint also checks declaration semantics.
The source guard is not dataflow analysis: review dynamically assembled CSS too.

Exceptions live in `xtask/raw-color-allowlist.json`, keyed by exact path and literal,
with positive occurrence counts and reasons. Counts must match, including removal.
Only independent browser assets/metadata, negative lint fixtures, literal renderer
assertions and UI copy naming a color remain exceptions. Do not exempt an app
component, directory or future literals wholesale.

## Initial inventory

This inventory records the starting revision (3e97d8d), excluding the authoritative
token file. Every rendering/default/fixture literal below is replaced by a token
or a value read from that file; browser assets and test/copy exceptions are listed
explicitly in the allowlist. No additional raw colors were found in Worker source,
its shared HTML styles, or JavaScript app snippets. There is no separate Worker
API docs HTML page in this revision; the GitHub App HTML renderer embeds the
same root token block and `github-app.css`.

| Source | Literal | Count |
| --- | --- | ---: |
| `crates/core/src/github_app.rs` | `#000000` | 1 |
| `crates/web/icons/build.py` | `#000000` | 1 |
| `crates/web/icons/build.py` | `#2A2A2A` | 1 |
| `crates/web/icons/build.py` | `#EDEDED` | 1 |
| `crates/web/icons/favicon.svg` | `#000000` | 1 |
| `crates/web/icons/favicon.svg` | `#2A2A2A` | 1 |
| `crates/web/icons/favicon.svg` | `#EDEDED` | 1 |
| `crates/web/icons/site.webmanifest` | `#000000` | 2 |
| `crates/web/icons/version.py` | `#000000` | 1 |
| `crates/web/index.html` | `#000000` | 2 |
| `crates/web/lint/value-policy.test.mjs` | `#0003` | 1 |
| `crates/web/lint/value-policy.test.mjs` | `#fff` | 4 |
| `crates/web/lint/value-policy.test.mjs` | `hsl(0 0% 0%)` | 1 |
| `crates/web/lint/value-policy.test.mjs` | `red` | 1 |
| `crates/web/lint/value-policy.test.mjs` | `rgb(0 0 0)` | 1 |
| `crates/web/src/components/feedback.rs` | `#5e6ad2` | 2 |
| `crates/web/src/components/icons.rs` | `#000` | 1 |
| `crates/web/src/components/icons.rs` | `#ededed` | 1 |
| `crates/web/src/components/shell.rs` | `#27a644` | 1 |
| `crates/web/src/components/shell.rs` | `#4ea7fc` | 1 |
| `crates/web/src/components/shell.rs` | `#5e6ad2` | 1 |
| `crates/web/src/pages/api_detail.rs` | `#4ea7fc` | 1 |
| `crates/web/src/pages/api_detail.rs` | `#8b82d9` | 1 |
| `crates/web/src/pages/api_detail.rs` | `#c6a1b9` | 1 |
| `crates/web/src/pages/api_detail.rs` | `#eb5757` | 1 |
| `crates/web/src/pages/gallery.rs` | `#000000` | 1 |
| `crates/web/src/pages/gallery.rs` | `#070708` | 1 |
| `crates/web/src/pages/gallery.rs` | `#141416` | 1 |
| `crates/web/src/pages/gallery.rs` | `#27a644` | 2 |
| `crates/web/src/pages/gallery.rs` | `#4ea7fc` | 2 |
| `crates/web/src/pages/gallery.rs` | `#5e6ad2` | 2 |
| `crates/web/src/pages/gallery.rs` | `#8b82d9` | 1 |
| `crates/web/src/pages/gallery.rs` | `#c6a1b9` | 1 |
| `crates/web/src/pages/gallery.rs` | `#eb5757` | 1 |
| `crates/web/src/pages/gallery.rs` | `#f7f8f8` | 1 |
| `crates/web/src/pages/projects.rs` | `#27a644` | 1 |
| `crates/web/src/pages/projects.rs` | `#4ea7fc` | 1 |
| `crates/web/src/pages/projects.rs` | `#5e6ad2` | 2 |
| `crates/web/src/pages/projects.rs` | `#8b82d9` | 1 |
| `crates/web/src/pages/projects.rs` | `#c6a1b9` | 1 |
| `crates/web/src/pages/projects.rs` | `#eb5757` | 1 |
| `crates/web/src/pages/projects.rs` | `#f0bf00` | 1 |
| `crates/web/src/pages/projects.rs` | `Green` | 1 |
| `crates/web/src/pages/projects.rs` | `Yellow` | 1 |
| `crates/web/src/pages/tickets.rs` | `#4ea7fc` | 2 |
| `crates/web/src/pages/tickets.rs` | `#8a8f98` | 1 |
| `crates/web/src/pages/tickets.rs` | `#8b82d9` | 1 |
| `crates/web/src/pages/tickets.rs` | `#c6a1b9` | 1 |
| `crates/web/src/pages/tickets.rs` | `#eb5757` | 1 |
| `crates/web/style.css` | `#fff` | 4 |
