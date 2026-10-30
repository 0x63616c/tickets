# TIK-50 — Board column visibility

Fixture-only `/design/board` screenshots, dark UI, bundled fonts loaded.
Baseline: `ada33c3` (the worktree's base commit). The before assets were built
before changing the frontend and saved outside the repository.

- `before/after-toolbar-{1440,390}.png`: tightly cropped toolbar at 2× DPR.
- `after-popover-{1440,390}.png`: shared Columns checkbox menu at 2× DPR.
- `before-popover-{1440,390}.png`: the previous toolbar, recording that the old
  fixture had no Columns popover; these are deliberately the actual baseline.
- `before/after-board-{1440,390}.png`: full board viewport at 1× DPR.
- `after-board-popover-{1440,390}.png`: full board viewport with the menu open.

The fixture's In review column is empty. After shows it as a normal visible
column; Canceled starts hidden. The fixture and API board use the same menu and
selection logic, with separate localStorage namespaces. The live API toolbar
also includes label and assignee filters. All data shown is in-memory sample
data, with no production API data or credentials.

Reproduce after `cargo xtask build`:

```sh
CHROME_BIN=/workspace/tickets-polish/chrome-noproxy cargo xtask component-shot board-columns --output docs/screenshots/TIK-50
```

Pass `--before-assets <baseline-dist>` to reproduce the before images.
