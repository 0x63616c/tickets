Method: dual-agent (A: `/root/design_review` · B: `/root/evidence_review`).

# TIK-41 — API tokens: critique and redesign specification

Phase 1 only · reviewed 2026-10-29 · base `63107b4275807a3a54956b8708921d1153dd20fc`.

**Recommendation:** turn the page into a small credential inventory inside Settings. Give creation a focused create → copy → saved flow, make revocation a named decision, and expose useful lifecycle metadata. Keep Tickets’ near-black surfaces, Inter, compact rhythm and existing shell.

This is a design handoff, not an implementation. TIK-38 supplies the shared controls; TIK-40 supplies Settings and its navigation. Phase 2 starts after those changes merge. App code, migrations and API behavior are unchanged; no PR or commit was created. No after screenshots are claimed.

## Evidence and method

The actual `/settings/tokens` route was built with `source .job.env; cargo xtask build` and run under local Wrangler with the full migration chain and dedicated local D1 state. No production data was seeded or changed. Final captures are dark, DPR 1, 1440×900 and 390×844, with fonts loaded. They show the whole application viewport, not a mockup.

The one-time secret is deliberately **invalid fixture text**, `tk_FIXTURE_ONLY_NOT_A_VALID_TOKEN_…`. The harness forwards the local create request and preserves its real metadata, replacing only the returned secret before the app renders it. List fixtures use hashes of invalid fixture strings. No usable credential appears in published evidence. This response substitution is the only content intervention in the shown-once state; no app layout or CSS was changed.

| Evidence pair (`before-…-{desktop,mobile}.png`) | What it records |
| --- | --- |
| `empty` | Zero tokens, current inline creation form and empty copy |
| `list` | Four fixtures, different creation dates, used/unused and revoked states |
| `create` | Filled current inline form before submission |
| `secret` | Actual creation success UI with invalid fixture secret |
| `secret-copied` | Copy success; clipboard contains only the invalid fixture |
| `revoke` | Focused Revoke action; **no confirmation exists** |
| `revoked` | Immediate result of clicking Revoke |

All 14 PNGs live in `/workspace/tickets-polish/TIK-41/` and `docs/screenshots/TIK-41/`. `capture-evidence.json` records dimensions, geometry and interaction checks. The initial runtime check also reproduced a stale “Token copied.” message after dismissal and revocation. The final isolated state captures avoid carrying that previous action into every image.

The review combines independent Impeccable design assessment A and detector/browser assessment B, plus source inspection and public reference research. The detector’s support and browser limitations are recorded below; absence of a static finding is not an accessibility certification. This surface is **Operate**: clarity and safe task completion outrank decoration. The user’s motion rules and pinned design contract override generic skill defaults.

## Critique

**Design specificity:** the black/Inter vocabulary belongs to Tickets, but the central composition is a generic credential form. It emphasizes minting another token over identifying and managing the agents already connected. The opportunity is to make lifecycle and agent identity legible, not to add illustrations or more visual effects.

### Five priorities

1. **P1 — Make the shown-once secret a protected completion step.** The current form remains above the secret and can create again. Its submit handler clears the previous secret before the request succeeds; even a failed second request destroys the first display. “Dismiss” does not mean “I saved it,” there is no navigation guard, and success does not focus or announce the secret region. On mobile the new card starts around y=377, below an already completed form. Use one dialog that advances from creation to a focused saved-token step. Keep the secret in memory until explicit acknowledgement or deliberate discard.
2. **P1 — Revocation needs identity, consequence and pending feedback.** A neutral 32px Revoke button immediately sends DELETE, with no confirmation or per-row busy guard. Every active row exposes the same accessible name. Use a contextual menu action followed by a compact confirmation that names the token, identifies its preview/ID and explains what breaks. Confirm success only after the server responds; never suggest Undo for an irreversible credential revocation.
3. **P2 — Put the inventory first.** The existing form consumes roughly 223px on desktop and 241px on mobile before any token row. Repeated “API tokens” / “Personal API tokens” headings add little hierarchy; a second Sign out competes in the page header. There is no count, list heading, status grouping or token identifier. Rows are approximately 73px tall yet have only a name and 11px date metadata. Move creation behind one clear action, use a compact aligned list, and retain revoked history behind an explicit filter.
4. **P1 — Give async results an owner.** A single global `role=status` stores copy success, create failures and revoke failures. Its message can remain unrelated to the current action. Creation has no “Creating…” label; initial load and empty state share the same unstructured area. Errors are detached from fields, the server validates bytes while the input limits characters, and there is no dedicated recovery path for an ambiguous create response. Use field errors, per-operation pending labels, adjacent copy feedback and a named result announcement.
5. **P2 — Make the narrow layout usable, not merely contained.** The sampled states fit 390px without horizontal overflow and the secret wraps, which is good. However 11px lifecycle text and 32px actions are unnecessarily hard to read/tap. The secret increases the list’s vertical offset by about 221px. Use 12px metadata, 44px touch areas, stacked rows and a viewport-bounded dialog with reachable actions. Never hide destructive actions behind hover alone.

### What to preserve

- The restrained palette, bundled fonts, visible labels and direct token naming suit a daily work tool.
- The backend stores hashes only, returns the secret only at creation, and applies `Cache-Control: no-store`. The UI removes the secret on dismissal and does not recover it after reload.
- Created and last-used dates already exist; revoked rows are retained. Native buttons, a real labelled input, wrapped token text and the shell’s keyboard affordances are useful foundations.

### Hierarchy, copy and accessibility detail

The current instructional sentence explains “one token per agent” well but omits the authority being granted and the fact that tokens never expire. “Last used Never” should become “Never used”; full dates should be available to keyboard and touch users rather than only native hover titles. A prefix/ID distinguishes duplicate names. “No API tokens yet.” confirms emptiness but does not explain what a token connects or point to API usage.

Keep one page title and subordinate list/empty headings. Tie helper/error text to the input with `aria-describedby`; use `aria-invalid` only after validation. Make dialog names, warning descriptions and focus return explicit. Announce success and failure without speaking the secret. Browser/DOM checks cannot substitute for a real screen-reader pass; include that in phase-2 acceptance.

### Cognitive and emotional review

The page is not overloaded with choices; its problem is **missing decision context**. A first-time operator cannot judge access or lifetime. A keyboard user reaches indistinguishable Revoke controls and gets no guided secret handoff. An interrupted mobile user can return to an exposed secret or leave without saving it. Creation should end with confidence that the token is stored; revocation should end with an unambiguous result naming the affected agent.

### Heuristic score and measured evidence

| Nielsen heuristic | Score /4 | Assessment |
| --- | ---: | --- |
| Visibility of system status | 2 | Weak operation-specific pending/success states |
| Match with the real world | 3 | Agent naming is clear; authority and lifetime are implicit |
| User control and freedom | 2 | Secret loss and immediate revoke lack deliberate boundaries |
| Consistency and standards | 3 | Coherent shared visuals; duplicate account actions |
| Error prevention | 1 | One-click revoke and replaceable one-time secret |
| Recognition rather than recall | 3 | Useful names/dates, insufficient token identity |
| Flexibility and efficiency | 2 | Simple create form, weak inventory/focus transitions |
| Aesthetic and minimalist design | 3 | Restrained appearance, oversized persistent form |
| Error recovery | 2 | Preserved input, detached errors and missing list Retry |
| Help and documentation | 2 | Brief guidance without complete lifecycle explanation |
| **Total** | **23/40** | **Acceptable foundation; meaningful improvements required** |

Assessment A finished before detector findings entered synthesis. Its five priority groups align with the browser evidence. Assessment B found the following concrete details that strengthen the redesign requirements:

- Keyboard create, dismiss and revoke can leave focus on `BODY`; the new secret region has no focus handoff or live announcement. Active row buttons all have the accessible name “Revoke.”
- The Token name resting border is **1.29:1** against its exterior, below the local 3:1 meaningful-boundary contract. Its focus ring is **4.47:1**. Address the shared control boundary through the TIK-38 contract, preserving the black palette.
- Sampled text contrast passes: names **19.74:1**, metadata/Revoked **6.46:1**; primary button text **5.19 / 6.10 / 6.87 / 9.84:1** at rest/hover/active/disabled. These are measured samples, not proof of every page state.
- At 390px, the normal rows stay contained; an intercepted long-name fixture expands to 112px rather than overflowing. Both input and buttons remain 32px high. A failed initial GET leaves creation disabled with no Retry control.

The static detector returned zero findings (`[]`, exit 0), but its documented extensions omit Rust and it supplied no scanned-file count: **Rust/Leptos coverage is unverified**. The browser detector was successfully injected into four fixture states and reported no anti-patterns; it missed the behavioral and boundary issues above. No false positives were used. Full evidence: `/workspace/tickets-polish/TIK-41/assessment-b/assessment-b.txt`, `browser-evidence.json` and `verified-states.json`. B's error/long-name/pending states used intercepted fixture responses; the primary 14 captures used the local D1 lifecycle described above.


## Reference study

These are public documentation observations, not claims to have entered authenticated dashboards. “Not evidenced” means the reviewed sources did not establish that exact UI. In particular, do not invent expiry presets, last-used columns, confirmation wording or empty screens from memory. The proposed Tickets choices below are design recommendations.

| Product and public sources | Verified patterns worth adopting | Boundaries |
| --- | --- | --- |
| **GitHub fine-grained PATs** — [Managing personal access tokens](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens), [reviewing organization token access](https://docs.github.com/en/organizations/managing-programmatic-access-to-your-organization/reviewing-and-revoking-personal-access-tokens-in-your-organization) | Creation separates name, optional description, expiration, resource owner, repository access and permissions. Scope is an explicit decision. GitHub recommends minimal access; current docs allow infinite lifetimes subject to policy. Prefill docs specify a 30-day default and configurable duration. Adopt descriptive naming, visible authority and a short expiry choice. | Tickets has one owner, so no organization approval workflow or repository permission matrix. Organization-level revocation is not identical to deleting the credential globally. Exact fine-grained copy acknowledgement, preview, last-used/creator columns, regenerate UI and empty state were not evidenced here. |
| **Vercel access tokens** — [creation and usage guide](https://vercel.com/kb/guide/how-do-i-use-a-vercel-api-access-token), [list-token API](https://vercel.com/docs/rest-api/authentication/list-auth-tokens) | Account settings lead to descriptive name, scope, expiration, creation and one-time copying. Lost tokens require replacement. The API exposes name, prefix/suffix, creation, activity, expiry and revocation metadata; that is a useful inventory model, not proof of exact dashboard columns. The guide advises distributing a replacement before revoking the old token. | Adopt the compact lifecycle and identifiable preview. Public docs do not establish the precise expiration presets, copy checkbox, revoke-dialog wording, created-by column or empty-state layout. Do not copy account/team/project selectors without enforcement. |
| **Linear API keys** — [API and Webhooks](https://linear.app/docs/api-and-webhooks), [Security & Access](https://linear.app/docs/security-and-access), [developer authentication](https://linear.app/developers/graphql) | Keys belong in account Security & Access. Full access can be narrowed to Read, Write, Admin, Create issues or Create comments, with optional team limits. Administrators can view/revoke workspace keys. Adopt the Settings placement, concise access summary and a small set of comprehensible permission choices if Tickets later enforces them. | Avoid inventing a matching dashboard from the docs. Exact naming form, expiry presets, show-once/copy acknowledgement, masked identifier, activity/creator timestamps, regenerate and empty states are not established by these pages. Tickets does not need a member policy control. |
| **Stripe API keys** — [API keys guide](https://docs.stripe.com/keys) | User-created live secrets have a named create → copy → save/Done sequence and cannot be recovered. The guide distinguishes these from Stripe-created keys and sandbox keys, which have different reveal rules. Expire has a confirmation/cancel step. Rotation explicitly handles a replacement and an old-key grace period. Adopt deliberate completion, named irreversible actions and safe replacement ordering. | Tickets must not offer a reveal action for stored hashes, nor imply Stripe-style automatic rotation exists. Names/notes and request logs aid identification; the reviewed page does not prove exact last-used/created-by columns, masked-row layout or zero-key empty state. No test/live switch is needed in Tickets. |

### Pattern decision for Tickets

| Requested pattern | Decision |
| --- | --- |
| Create / naming | One short dialog; “Token name”, example “Build agent”, one token per script/agent |
| Scopes / permissions | Show the true full-workspace authority now; defer selectors until enforcement is complete |
| Expiry presets | Recommended additive expiry: 7 / 30 / 90 days / No expiration; 30 days selected for new UI creates |
| Show-once / copy / confirmation | Dedicated completion step, explicit Copy feedback, “I’ve saved this token” acknowledgement |
| Masked prefix | Persist only a short preview for new tokens; identify legacy rows by token ID |
| Last used | Promote existing field; null means “Never used”, not “inactive” |
| Created by / at | Show created date; omit repetitive creator column in a single-user product; do not infer who minted bearer-created tokens |
| Revoke | Name + identifier + consequence + Cancel/confirm; retain history |
| Regenerate | Defer atomic rotation; offer create replacement, update consumers, then revoke old token |
| Empty | Left-aligned explanation and one primary creation action; no artwork or zero-value metrics |

## Backend reality

Inspected `crates/worker/src/auth.rs`, `auth_queries.rs`, `live.rs`, `crates/core/src/auth.rs`, the migration chain (especially `0002_data_model.sql`), Worker auth/query-budget tests, and `docs/API.md`. Authentication behavior is also described in `CONTRIBUTING.md`, `docs/DEVELOPMENT.md` and `docs/adr/0003-auth.md`.

| Capability | Exists today | Design consequence |
| --- | --- | --- |
| Identity | `id`, `user_id=1`, `name`; duplicate names allowed | Always key actions by ID; do not use name as identity |
| Secret | `tk_` plus 64 hex characters from 32 random bytes; only SHA-256 hash stored | Retrieval/reveal is impossible after creation; never derive a pretend preview from the hash |
| Metadata | `created_at`, nullable `last_used_at`, nullable `revoked_at` | Useful list and history can ship without a migration |
| Last use | Bearer auth executes an UPDATE with timestamp + `RETURNING user_id` | This is last successful credential authentication, even if the later operation fails; it is not a heartbeat or proof a job is healthy |
| Expiration | Sessions expire; **API tokens do not** | Do not use session expiry as token expiry |
| Permissions | Every valid token has owner-level API access, including token management | Read-only/project-restricted controls would currently be false promises |
| Creation | POST `{name}` returns `{token,metadata}` with 201 | Insert returned metadata after confirmed success; a list-refresh failure must not discard the secret |
| Listing | GET returns a bare array, newest ID first, including revoked rows | Preserve that response shape; filter locally in the first redesign |
| Revocation | DELETE sets `revoked_at` with `coalesce`; repeat revoke is idempotent | No physical deletion, restoration or Undo; a retry can safely reconcile the same ID |
| Edit/rotate/provenance | No rename or rotate endpoint, description, preview, expiry, scopes or issuer field | Separate UI-only improvements from additional API contracts |

Important implementation details: name validation currently checks the **untrimmed UTF-8 byte length ≤100**, trims for storage, and returns 400; the input’s `maxlength=100` counts differently. The token Create DTO also lacks `deny_unknown_fields`, despite the general API convention. An old Worker could silently ignore a newly sent expiry/scope field. Phase 2 must verify returned metadata/capability before claiming restricted or expiring access and must sequence rollout accordingly. `docs/API.md` does not currently document the token CRUD contract in a dedicated section; add it with any token API change and update the Tickets skill.

Revocation and future expiry are checked at authentication. Existing live WebSockets are accepted into a BoardRoom without retaining token identity or periodically rechecking it; already connected sockets can keep receiving invalidation events. Future REST requests fail, but “all connections stop immediately” would overstate the current guarantee. This is relevant to expiry/scope design and should be handled in a dedicated auth/live follow-up, not concealed in UX copy.

### Recommended additive changes

| Priority / addition | Additive design | Risk and required guard |
| --- | --- | --- |
| **Low cost — metadata preview** | Nullable `token_prefix TEXT`; for new tokens save only `tk_` plus first 6 random hex characters at minting. Return it in creation/list metadata. Existing rows remain null and display `Token #17`. | **Low.** A short secret fragment becomes metadata (24 bits disclosed from a 256-bit random value). Never store a full token/suffix combination accidentally; test length, serialization and null legacy rows. No hash backfill. |
| **Recommended — optional expiry** | Nullable UTC `expires_at TEXT`, default null. POST accepts optional canonical expiry. Existing name-only API calls and existing rows remain non-expiring; UI explicitly submits its chosen expiry. Extend the existing bearer UPDATE predicate with `(expires_at IS NULL OR expires_at > now)` so the one-query auth budget is preserved. Return expiry in metadata. | **Medium.** Test equality boundary, UTC normalization, invalid/past/far-future input and revoked+expired precedence. No retroactive expiry. Old servers silently ignore new fields, so expose UI only with confirmed support. Existing WebSocket lifetime is a separate risk as above. |
| **Low cost — rename** | Add PATCH `/tokens/{id}` accepting only `name`; update no secret material. Reuse ID ownership checks and consistent trimmed UTF-8 validation. | **Low–medium.** Duplicate names remain legal; show prefix/ID in confirmations. Unknown fields rejected. Retrying the same name is safe. |
| **No schema change — dates/history/docs** | Reuse `last_used_at`, creation and revoked timestamps; expose precise accessible dates; add token contract to API docs/skill. | **Low.** Do not label a last-auth timestamp as a successful job or online status. Do not add extra writes just to improve its display. |
| **Later — issuance provenance** | Optional issuer kind and nullable `created_by_token_id` for future mints. Old rows stay unknown. UI may show “Created via API” in details. | **Medium.** `user_id=1` proves ownership, not whether the human or an agent minted it. Avoid invented creator names, IP collection and personal data. No backfilled attribution. |
| **Separate auth work — scopes** | Add a permission representation with legacy `full_access` default; start with one rigorously enforced read-only option if needed. Return capability in the auth query and check every dispatch path. | **High.** Schema is easy; authorization is not. Prevent scoped tokens minting full-access tokens, cover live upgrades, global labels, external upserts and cross-project dependencies. Resource scoping is broader still. Do not ship the selector first. |
| **Later — replacement/rotation relation** | Nullable `replaces_token_id` can link independently valid credentials. A future explicit rotate endpoint can create a replacement and schedule old-token expiry. | **Medium–high.** Ambiguous POST responses, grace periods, idempotency and lost secrets need design. Keep the current token valid until the new one is saved and deployed; never rotate automatically as a cosmetic action. |

No migration is proposed for last use, created date, revocation or the owner. No columns are dropped, existing tokens are not invalidated, and revoked history is not deleted. Keep bearer lookup atomic and all request/query budgets within the existing six-D1-call contract. Run migration/query-budget and auth tests when these additions are implemented.

## Settings → API tokens specification

### Placement and layout

Use the content slot of TIK-40’s Settings shell. Preserve `/settings/tokens` as the canonical deep link or an alias if that ticket establishes another canonical route. Do not add another sidebar or page-level Sign out. TIK-40 owns navigation, selected state and responsive Settings chrome.

Within the available content area, use a left-aligned max-width **880px** section, 32px desktop outer spacing and 16px mobile gutters. At 390px that leaves 358px. Use the shared 44px shell header. Page heading: 20px/590; body/name: 13px; secondary metadata: 12px; preview: 12px Geist Mono. Reuse token roles exactly: `--bg`, `--surface`, `--surface-raised`, `--border`, text roles and action fills; 6px controls, 8px menu and 12px modal radii.

```text
Settings / API tokens                       [TIK-40 shell]

API tokens                                  [Create token]
Connect agents and scripts to your workspace.  API documentation ↗
Full workspace access · Tokens do not expire.*

3 active tokens                              [Show revoked]
Token                           Last used     Created        ⋯
Build agent                     2 hours ago   Oct 28, 2026    ⋯
Token #4 / short preview
Release automation              Today         Oct 21, 2026    ⋯
Token #3 / short preview
Local CLI                       Never used    Oct 20, 2026    ⋯
Token #2 / short preview
```

`*` Baseline copy when expiry has not shipped. With expiry support, use “Tokens have full workspace access.” and replace the Created column with Expires; keep Created in details. Avoid repeated constant “Full access” pills on every row. Do not imply limited permissions.

Rows: approximately **52px** for two lines; 32px column header; 16px horizontal/8px vertical cell padding. Name/identifier gets the flexible column; dates get about 104–128px each; the final action column is 32px. These widths must yield to the actual TIK-40 content slot. Below about 640px content width, stack metadata beneath each name, allow names to wrap, and reserve a 44px menu target at the right. Preserve the same reading and tab order. No horizontal table scroll at 390px.

The list uses native table semantics when shown as columns, or a semantic list with labelled metadata when stacked. Row actions always expose a labelled menu trigger: “Actions for Build agent, token 4”. No clickable row containing nested buttons. Full names and precise timestamps remain available on keyboard focus and touch; do not rely on `title` alone.

### Shared components

Use TIK-38’s Button/IconButton, Select, DropdownMenu, Tooltip, Skeleton and pending indicator where available, plus the existing shared Modal and text field. Use a shared switch/checkbox for Show revoked; do not invent a second control system. No native `<select>` is reintroduced. No new motion library is needed. A permissions Combobox is unnecessary until backend scope work is authorized.

### Create and save

Open a centred dialog, max-width **480px**, width `calc(100% - 32px)`, max-height `calc(100dvh - 32px)`. Internal scrolling must leave the primary action reachable when the mobile keyboard is open. Title “Create API token”; autofocus the name. Helper: “Use one token for each agent or script.” Label “Token name”; placeholder “Build agent”. Preserve drafts on validation/transport errors and validate the backend’s actual UTF-8 limit.

Show “Full workspace access” with “This token can read and change all workspace data and manage API tokens.” If expiry is supported, a shared Select offers **7 days / 30 days / 90 days / No expiration**, with 30 days selected and the resulting absolute date directly underneath. Selecting No expiration adds “This token works until you revoke it.” If expiry has not shipped, show that sentence as plain information with no dummy control. Buttons: Cancel / Create token; pending: “Creating…” and a stable-width indicator. Guard both click and Enter submission.

After a successful 201, replace the dialog body in place with:

- Title: **“Save your token”**; secondary line names the token just created.
- Warning: **“Copy this token now. You won’t be able to see it again.”**
- Wrapped/selectable read-only secret block and neutral **“Copy token”** button. Accessible naming identifies the secret field without putting its value in a label or live announcement.
- On successful clipboard write: **“Copied”** with a check and a polite announcement beside Copy. Failure: **“Couldn’t copy. Select the token and copy it manually.”** Keep manual selection possible and do not disable completion just because clipboard permission failed.
- Footer acknowledgement: **“I’ve saved this token”**. This deliberate action closes and clears it; clipboard success alone never dismisses it. No second create form is available in this step.

Dismissal via Escape, close control, backdrop or internal navigation before acknowledgement must offer “Close without saving?” with **Keep token visible / Close anyway**. Explain that closing hides the value but does not revoke the token. Use one dialog state, not nested modals. For browser refresh/tab close, a best-effort unload guard may warn, but the product cannot guarantee recovery. Do not add a timed auto-dismiss that destroys an interrupted user’s work.

Keep the secret only in the dialog’s transient memory; clear on acknowledgement/discard/unmount. Never put it in storage, URLs, analytics, logs, error details or automatically generated curl examples. Do not claim secure memory erasure from browser memory. On return after losing a secret, explain replacement then revocation; there is no Reveal action. A metadata refresh failure must leave a successfully received secret visible and offer an independent list retry.

### Revoke and history

Menu action: **“Revoke token…”**; its danger treatment is local to the action. Confirmation title: **“Revoke ‘Build agent’?”** Show the preview or token ID, created date and last-used value so duplicate names are distinguishable. Body: **“Future API requests using this token will fail. Update any agents or scripts that still use it. This can’t be undone.”** Buttons: **Cancel / Revoke token**; Cancel receives initial focus.

On confirm, guard duplicate requests and show **“Revoking…”**. Do not optimistically announce revocation. When confirmed, announce **“‘Build agent’ revoked.”** Move it out of the active list without animation that shifts other controls under the pointer. Retain its record in Show revoked, including revoked date; no Delete permanently or Undo. Restore focus to the next row’s action, previous row, or Create token when empty. Failure leaves the dialog and identity intact with an inline retry. If the response is ambiguous, refetch/retry the same idempotent DELETE before reporting success.

Default list: active only, newest first. Show revoked reveals history; if expiry ships, label the control **Show inactive** and distinguish **Expired** from **Revoked** (revoked takes precedence). Expired tokens have no usable access and can still be explicitly revoked; neither state can be restored in this release. Preserve records in D1. No bulk revoke is necessary for this surface.

### State and copy contract

| State | Presentation / recovery |
| --- | --- |
| Initial loading | Three stable 52px Skeleton rows, `aria-busy`; announce “Loading API tokens…” once. Do not flash empty state. Disable creation until authentication is confirmed. |
| Empty, never created | “No API tokens yet” / “Create a token to connect an agent or script to Tickets.” / sole Create token action + API documentation link. Avoid duplicate primary actions in the header and empty body. |
| No active tokens, history exists | “No active tokens” / “Create a token to reconnect an agent or script.” / Create token and Show revoked/inactive. Do not claim there have never been tokens. |
| Loaded | Count matches the visible filter; readable names, metadata and contextual actions. Newly created metadata enters only after confirmed creation. |
| Refresh failure | Keep existing rows; “Couldn’t refresh tokens.” with Retry and an indication that the displayed list may be out of date. |
| First load failure | Inline “Couldn’t load API tokens.” with Retry; never render an empty-success state. |
| Validation | “Enter a token name.” / “Use a shorter name.” with a precise UTF-8-limit helper when needed; preserve input. |
| Ambiguous create | “We couldn’t confirm whether the token was created. Refresh the list before trying again.” Preserve name, do not auto-repeat POST; revoke an identifiable orphan before replacing it. |
| Session expired | Existing sign-in redirect with safe return path; never include the secret in navigation or restore it from storage. |
| Copy / revoke feedback | Local to the action, announced politely; clear unrelated stale messages when a new operation starts. |

### Motion and accessibility

The current inline secret introduces layout movement with no focus handoff; adding entrance effects alone would not solve it. Use CSS only for brief, meaningful feedback. [Emil’s practical tips](https://emilkowal.ski/ui/7-practical-animation-tips) and [animation standards](https://github.com/emilkowalski/skills/blob/main/skills/review-animations/STANDARDS.md) informed the choices; the explicit TIK-37 brief is authoritative where the older local motion tokens differ.

| Before | Proposed behavior |
| --- | --- |
| Secret card inserts above the list | In-place dialog step; no list displacement or height animation |
| No state-specific dialog choreography | Pointer-triggered enter 220ms, exit 160ms, opacity + scale(.97→1), centred origin; scrim uses matching timing |
| Global weaker easing token | Enter/exit `cubic-bezier(.23,1,.32,1)`; on-screen movement, if needed, `cubic-bezier(.77,0,.175,1)`; never ease-in |
| Generic press feedback | 120ms interruptible transform transition, `:active scale(.97)`; retain visible focus |
| Menus/tooltips | Menu 180ms in/125ms out, tooltip 125ms; origin at trigger; immediate subsequent tooltips |
| Existing reduced-motion durations are zero | Follow job standard: drop transforms; retain short opacity/colour feedback. Keyboard-initiated/frequent actions get zero or negligible motion. |

No stagger, bounce, blur, scale(0), animated height or page entrance. Hover effects only under `(hover:hover) and (pointer:fine)`. All motion stays below 300ms. A token refresh must not move focused controls unexpectedly.

Require visible `:focus-visible` rings, 4.5:1 normal text and 3:1 meaningful control/focus boundaries. Decorative separators can stay subtle; input boundaries cannot rely on decorative border contrast alone. Use TIK-38’s verified controls and coordinate any shared token correction there. Preserve 44px mobile hit areas with 32px compact desktop visuals, 200% zoom reflow, semantic labels and dialog focus containment. Do not announce or automatically select/copy the secret on entry; focus Copy and describe the one-time warning. Keep mobile safe-area/keyboard scrolling within the dialog.

## Phase-2 acceptance and sequencing

1. Merge/rebase the handoff against TIK-38 and TIK-40’s actual component and Settings contracts before implementation. Preserve the approved palette and reuse their controls.
2. Ship the inventory, explicit copy acknowledgement, revoke confirmation, loading/error handling and responsive states using existing fields. Preview/rename are small optional additions; expiry is a separate auth-sensitive increment. Scopes and atomic rotation are separate work.
3. Verify fake-secret lifecycle, clipboard denied/manual copy, accidental close, duplicate submits, ambiguous create, refresh failure after successful mint, confirmation Cancel/Escape, confirmed revoke and idempotent retry. Include duplicate/long/non-ASCII names, no-token and all-revoked cases.
4. Exercise keyboard/screen-reader focus and announcements, 390×844 and 1440×900 dark layouts, 200% zoom, touch target sizes, reduced motion and contrast. Capture component states with the TIK-38 harness plus the full Settings page.
5. If expiry is added, test legacy nulls, old-worker capability mismatch, timezone/boundary behavior and auth query budgets; document the live-socket limitation or resolve it in the corresponding auth work. If scopes are added later, require an endpoint coverage matrix before exposing the controls.
6. At implementation review, run Impeccable critique **and polish**, the required Rust fmt/clippy/host+wasm/test checks, first-run-check and relevant UI checks. Capture before/after evidence and then open the single TIK-41 PR. None of these implementation/PR actions is performed in phase 1.

Questions skipped: this phase’s user brief supplies the audience, aesthetic, scope, dependencies and required deliverable; the defaults above are explicit recommendations for phase-2 review, not an implementation approval request.

## Phase 2 implementation decisions

The implementation merges `origin/main` without rebasing and uses TIK-40’s
`/settings/api-tokens` and TIK-38’s shared Modal, BusyButton, icons and focus/motion
contract. The earlier recommendations are a design handoff; the shipped scope
below supersedes optional backend additions.

- Inventory first: token name, real token ID, created date and existing last-used
  metadata. Revoked history is available through Show revoked; the heading count
  follows the filter. All-revoked states retain a creation path.
- No migration or API change. Tokens retain full workspace access and no
  expiration. Preview, expiry, rename, scopes and atomic rotation are deferred;
  there is no invented masked secret. A direct named Revoke action avoids a
  one-item menu, while the confirmation names the token and its ID.
- Creation inserts returned metadata directly, so a second list request cannot
  destroy the one-time secret. A locked dialog holds the secret in memory,
  focuses Copy and requires an explicit saved acknowledgement. A deliberate
  Close without saving → Close anyway path explains that the token remains
  active. Failed clipboard access preserves a readonly field that keyboard
  users can select and copy manually. Browser unload warns during creation and
  disclosure; no secret enters storage, query strings, toasts or live regions.
- Mutation failures stay with their operation. Ambiguous creation warns that a
  token may exist and directs the user to reload the inventory before retrying.
  Initial list failures have Retry. Revocation updates only after server success;
  failed revocations preserve the row and confirmation for retry.
- Mobile controls have 44px targets. Long names wrap in inventory and dialogs;
  copy feedback reserves space. Meaningful token input borders use the same
  semantic colour mix as the composer. Existing Modal motion and reduced-motion
  behavior are reused; no new animation library or global motion changes.

Phase 2 review and verification evidence is recorded in the screenshot README.
The Phase 1 reference study above remains the source of the pattern decisions.
