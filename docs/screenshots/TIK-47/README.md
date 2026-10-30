# TIK-47 loading evidence

Uses the published **ascii-reveal 0.3.0 by Quan Hoang, MIT License**.
[npm](https://www.npmjs.com/package/ascii-reveal/v/0.3.0) ·
[source](https://github.com/quanhoangindex/ascii-reveal) ·
[full notice](../../../THIRD_PARTY.md).

The before build is main at `8bc5d99bacba936de87d2531c49993decfc4ec3f`,
including the TIK-38 custom loaders. Both builds use the same immutable
`docs/fixtures/home-api.json` data on the real routes. Browser instrumentation
holds API responses; it does not replace, simulate or delay the animation code.
Production has no artificial data delay. Captures use Chromium headless shell,
dark mode, loaded local fonts, and a 1440×900 viewport at 1× to keep files small.

| Route | Before | After |
| --- | --- | --- |
| `/board` | [before-board.png](before-board.png) | [after-board.png](after-board.png) |
| `/t/TKT-1` | [before-detail.png](before-detail.png) | [after-detail.png](after-detail.png) |
| `/list` | — | [after-list.png](after-list.png) |
| `/projects` | — | [after-projects.png](after-projects.png) |

[loading.gif](loading.gif) records the real board's skeleton sweep, response
arrival and text decode. It is 960×540 at 15 fps, 1,344,296 bytes, and below 3 MB.
The recorded board decode took 451 ms with zero uncaught browser exceptions. The response is
held for 2.5 seconds solely to make the sweep visible. Separate component crops
are generated under ignored `target/component-shots/`; the six PNGs above are
the committed full-page evidence.

## Reproduce

```sh
export CHROME_BIN=/path/to/headless-chromium
cargo xtask build # includes npm ci --prefix crates/web and Trunk release build
TICKETS_BEFORE_ASSETS=/path/to/main/crates/web/dist cargo xtask loading-check
cargo xtask loading-check
cargo xtask component-shot all
```

The before worktree must be built at the documented base revision. For current
direct Cargo wasm or Trunk builds, first run `npm ci --prefix crates/web`.

`loading-check` covers moving package canvases and decode markers on board,
list, detail, projects, Home, account, token and project settings; search results; repeat
busy buttons with a stable footprint; live OS/app reduced motion; interrupting
a decode; route disposal; and mobile overflow. It deliberately observes the
package's actual DOM/canvas activity so a static imitation cannot pass.

`first-run-check` exercises an empty local D1 through the UI. `detail-check`
covers real local comment CRUD, including decoding only the new/edited body,
as well as failed-save recovery, labels, dependencies and mobile layouts.
Fixtures and recording contain sample data only. The read-only capture server
does not upgrade WebSockets; its offline indicator is expected. The local D1
acceptance runs use the real Worker transport.

See [critique and polish](review.md) for independent visual review and its limits.

## Build and test evidence

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo clippy --workspace --all-targets --target wasm32-unknown-unknown -- -D warnings`
- `cargo test --workspace` (70 tests)
- `cargo xtask build` (`trunk build --release --locked`, then Worker build)
- `CHROME_BIN=… cargo xtask first-run-check`
- `CHROME_BIN=… cargo xtask detail-check`
- `CHROME_BIN=… cargo xtask loading-check`
- `CHROME_BIN=… cargo xtask component-shot all`

The emitted `snippets/tickets-web-*/node_modules/ascii-reveal/dist/core.js` is
byte-identical to the installed npm module. Its SHA-256 is
`106baa3a05fa216d5bb0ed9cf979f817402a2c443dad688c19608ced2c44c8d8`.
No library source or generated Trunk bundle is committed.
