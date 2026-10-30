# TIK-72 comment thread evidence

Dark fixture data only. `cargo xtask detail-check` seeds a signed-in Sample User,
a Build agent token, an edited user comment and a deleted comment, then captures
1440px desktop / 390px mobile and the activity section at 2×.

Before assets reuse the TIK-65 baseline built from `07343d7`; the ticket detail,
activity and history component sources are identical to this issue's base
`ada33c3`. The stylesheet is restored from `ada33c3` for matching shared styles.
After captures use this branch. Both asset versions read the same seeded local
API fixture, including tombstones. Old assets do not understand bot badges,
edited markers or deleted stubs; that is the baseline being extended.

Functional verification additionally covers Cmd+Enter posting with a held
response (pending row and disabled controls), Ctrl+Enter saving, Esc cancel and
focus restoration, confirmation cancel/Esc/delete, persisted sanitized Markdown,
comment/history merging, and retained drafts after rejected posts.

The Impeccable critique used independent design and detector assessments. Its
Rust source scan reported zero findings but does not support Leptos `.rs` files;
source review and rendered browser checks supply the relevant evidence. The
polish pass addressed keyboard/touch timestamps, action focus, mutually exclusive
edit/delete states, inline errors, touch targets and long author wrapping.
