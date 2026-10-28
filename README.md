# Tickets

A Linear-style ticket tracker written in Rust. It is the central place Calum and his agents
track and do work across all projects.

- Backend: Rust on Cloudflare Workers (`workers-rs`) with D1.
- Frontend: Rust (see `docs/adr/` for the framework decision).
- Production target: `tickets.worldwidewebb.co` (Cloudflare Workers, free tier).
  **Nothing is deployed to production without Calum's explicit OK.**

Features: projects (linked to GitHub Projects), tickets with dependencies (blocks / blocked-by,
cycle prevention), labels, Kanban and list views, dark mode throughout, login plus personal API
tokens so agents can work fully through the API, and an agent skill in `.agents/skills/tickets/`.

See `CONTRIBUTING.md` and `AGENTS.md` for how work is done here. The v1 spec is issue #1.
