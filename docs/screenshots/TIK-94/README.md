# TIK-94 navigation shortcuts

Before: commit `5a7ea24` (the unchanged starting worktree). After: this PR.
All captures use fixture-only `/design` routes, real bundled fonts and the
existing dark tokens. No production data or services are used.

`cargo xtask navigation-check` captures palette and shortcut dialogs at
1440×900 (cropped at 2×), the shortcut sheet at 390×844, and navigation tooltips.
Navigator platform is overridden before app initialization to verify both
Mac (⌘) and Linux/Windows (Ctrl+) hints and matching modifier behavior.

Reproduce after building:

```sh
CHROME_BIN=google-chrome cargo xtask navigation-check
```

Set `TICKETS_BEFORE_ASSETS` to the saved starting build's dist directory for
matching before captures. Before mode records the old interface without
asserting the new behavior. After mode checks every destination, editable
controls (including inherited/plaintext contenteditable), palette focus,
preventDefault, ignored modifier/repeat/composition combinations, removal of
chord navigation, consistent hints and mobile overflow.

Local validation used a private Chrome mount namespace with an empty managed
policy directory, because the box's managed SOCKS policy prevents loopback
access. Shared policies were unchanged. Worker checks used a private network
namespace to avoid collisions with other issue worktrees on port 8795.

Impeccable critique used independent design and detector/browser assessments:
33/36, no blocking findings. CLI detector returned no findings; browser warnings
were the existing Geist and 11px metadata choices pinned by DESIGN.md. Polish
settled animations before capture and verified real 2× desktop crop dimensions
and sharpness. Mobile images and full-page tooltips remain native 1× captures.
Additional standard palette/sidebar component evidence is in `components/`.
