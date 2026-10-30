# TIK-44 command palette evidence

Before: `40ef5f9` (TIK-40), built from an isolated detached worktree. The actual
previous palette is opened over `/design/board`, with only repository fixtures.
After: the same fixture route, data, fonts and dark scheme; isolated
`/design/c/palette` additionally exercises the shared component.

- `before/after-palette.png`: initial palette, tight 2× crop.
- `before/after-search.png`: exact `TKT-7` query, tight 2× crop.
- `before/after-mobile.png`: 390×844 viewport, cropped dialog.
- `before/after-full-page.png`: 1440×900 dark page context.
- `after-actions.png`, `after-status.png`, `after-filters.png`, `after-empty.png`:
  contextual commands, property choices, combined filters and no results.

All values are fixtures. No production account, ticket data or API tokens appear.
The browser uses `/usr/bin/google-chrome` through the existing child-only
`/workspace/tickets-polish/chrome-noproxy` policy wrapper; shared browser policy
is untouched. Files are also copied to `/workspace/tickets-polish/TIK-44/`.

## Brief research

- [cmdk](https://cmdk.paco.me/): searchable grouped items, custom ranking, persistent
  input focus and predictable selection.
- [Linear search](https://linear.app/docs/search): identifier lookup, recent issues
  and project-scoped discovery.
- [Raycast Action Panel](https://manual.raycast.com/action-panel): primary Enter
  action, contextual actions, fuzzy command filtering and Escape to return.
- [Emil Kowalski](https://emilkowal.ski/ui/7-practical-animation-tips): quick feedback,
  reduced motion, subtle press state and little movement for frequent actions.

The implementation keeps the project's pinned near-black surfaces, compact rows,
Inter/Geist typography and shared icons/modal/spinner. It adds no JS UI library.

## Verification

The palette fixture check covers grouped results, search without input movement,
keyboard action levels, contextual accessible names, touch actions, 390px width,
390×400 constrained height, opener focus, and reduced motion. The API acceptance
check exercises all commands, cross-project search, body matching, recent history,
failed-save retry, project moves, clipboard, creation and stale-response handling.
The API ranking regression uses SQLite and the complete migration chain, asserting
two database calls including authentication.

Final verification after merging `origin/main` at `6f1b42a`:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo clippy --workspace --all-targets --target wasm32-unknown-unknown -- -D warnings`
- `cargo test --workspace` (test debug info set to 1 to isolate shared-cache artifacts)
- `palette-check`, `skill-check` (36 curl examples), `first-run-check`,
  `polish-check`, `contrast-check`, `design-check`, and `component-shot all`
- Impeccable critique and polish; see [review.md](review.md)

All passed. The initial contrast run found the TIK-40 sidebar's 10px shortcut
badge; merged TIK-41 already fixes it. One local login-focus check needed a retry.
Port-8795 collisions were retried without stopping other jobs. Builds and final
captures were serialized to keep asset replacement out of browser runs.
