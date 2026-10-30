# TIK-56 token inventory

Before: `8bc5d99bacba936de87d2531c49993decfc4ec3f`.
After: this branch, same fixture route `/design/settings/api-tokens`.

Dark desktop 1440×900 and mobile 390×844 PNGs, plus cropped 2× settings
content. Fonts loaded before capture. Fixture metadata only; no token secrets
or production data appear in these captures.

Reproduce after building:

```sh
CHROME_BIN=/workspace/tickets-polish/chrome-noproxy \
TICKETS_SCREENSHOT_DIR=/tmp/TIK-56-shots cargo xtask tokens-shot
```

Only the `after-list-*.png` outputs are published here. The browser check also
verifies the removed access paragraph, compact heading gap, and retained
create-modal grant explanation, plus the existing fixture token lifecycle.
