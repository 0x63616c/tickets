# TIK-53 sidebar account menu

Dark fixture-only before/after captures using Sample User and sample projects.
Before assets: cd30ecc67378c79a47a9649e1ce727ae180cb6ac (the base commit), built
in an isolated temporary worktree. Debug sections were stripped from the base
Wasm to keep browser loading bounded; application code is unchanged. No production
account data is used. The after images also include the Stub logo from
`3c1ec7e`, merged from origin/main during this task.

`sidebar` and `user-menu` are tightly cropped 2× component captures. `desktop`
and `mobile` show the full fixture board at 1200×900 and 390×844 respectively.
The mobile capture has the navigation drawer and account menu open.

Reproduce with `cargo xtask component-shot sidebar --output docs/screenshots/TIK-53`
and `cargo xtask component-shot user-menu --output docs/screenshots/TIK-53`;
add `--before-assets <base-dist>` for matching before captures. Use
`CHROME_BIN=/workspace/tickets-polish/chrome-noproxy` locally.

The checks exercise menu activation, arrow navigation, shortcuts, Escape/focus
restoration, upward positioning, inset 2px item gaps and short-viewport pinning.
