# Tickets

A Linear-style ticket tracker written in Rust. It is the central place Calum and his agents
track and do work across all projects.

- Backend: Rust on Cloudflare Workers (`workers-rs`) with D1.
- Frontend: Rust (see `docs/adr/` for the framework decision).
- Dogfooding: Tickets tracks its own work in Tickets; see `docs/DOGFOOD.md`.
- v1 verification evidence: `docs/V1-VERIFICATION.md`.
- Production target: `tickets.worldwidewebb.co` (Cloudflare Workers, free tier).
  Merges to `main` deploy automatically to https://tickets.worldwidewebb.co (behind login).

Features: projects (linked to GitHub Projects), tickets with dependencies (blocks / blocked-by,
cycle prevention), labels, Kanban and list views, dark mode throughout, login plus personal API
tokens so agents can work fully through the API, a one-way GitHub issue mirror (signed webhooks plus cron reconciliation), and an agent skill in `.agents/skills/tickets/`.

See `CONTRIBUTING.md` and `AGENTS.md` for how work is done here. The v1 spec is issue #1.
