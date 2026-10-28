# Projects (#10)

Dark mode, loaded bundled fonts, Chromium headless shell; desktop 1440×900,
mobile 390×900. All application data was seeded through the API into isolated
local D1 via `wrangler dev` (port 8795). No production access.

Before: origin/main `e488f34`, built in the temporary `/tmp/tickets-10-before`
worktree with its own target directory. `TICKETS_BEFORE_ASSETS` selected those
assets for `cargo xtask projects-check`. The real `/projects` route did not exist
on main, so the before image honestly shows Page not found. The temporary
worktree was removed after capture. `projects-target-desktop.png` separately
captures `/design/projects`; this fixture is not the before image.

After: `/projects` on `feat/10-projects`, with the same seed and viewports.
The existing fixture table dimensions, controls, tokens and 240px desktop sidebar
are retained. The project glyph uses its saved icon color. Extra images show
creation, archived projects, and GitHub Project/repository links in the header.

Reproduce:

```text
CHROME_BIN=/home/box/.cache/ms-playwright/chromium_headless_shell-1248/chrome-headless-shell-linux64/chrome-headless-shell cargo xtask projects-check
```

The check exercises UI creation, rename/prefix changes, user and organization
URLs, invalid URL feedback, optional repository persistence, reload, archive and
restore, sidebar visibility, and actual header links. It creates a ticket in a
chosen project, moves it on Kanban, attaches a seeded label and a dependency,
reassigns the ticket to another project, then checks persistence after reload.
Core tests cover URL parsing, and API tests cover owner type validation and
preserving tickets on archive/prefix change.

Clarity check: Projects → New project → open its name → New ticket is explicit.
Kanban provides a visible status select; detail offers Add label and Add dependency.
These actions completed in the browser check without hidden modes. Label creation
itself remains API-only, as in the existing detail view; this issue adds no label
management UI. Linking needs no GitHub authorization or API request.
