# ADR 0002: Versioned SQL with D1 migration tracking

Status: Accepted

## Context

The Worker accesses SQLite through the D1 binding, rather than a database socket.
The native SQL drivers used by sqlx and refinery cannot connect directly to D1.
A separate migration framework would duplicate Wrangler's migration tracking.

## Decision

Commit sequential, zero-padded SQL files in `migrations/`, starting at
`0001_scaffold.sql`. Use `cargo xtask migrate` to invoke
`wrangler d1 migrations apply DB --local`. Wrangler records applied filenames in
`d1_migrations` and applies remaining migrations in order. See the
[Cloudflare migration reference](https://developers.cloudflare.com/d1/reference/migrations/).

Applied files are immutable: add a new forward migration for subsequent changes.
CI applies the complete chain to an empty local database on every PR and main
push. The scaffold migration deliberately creates no domain tables; #3 owns them.

## Consequences

SQL stays reviewable and independent of Rust runtime dependencies. Local commands
and CI use D1's own migration semantics. Local D1 state is ignored by Git.
Production migration execution requires separately authorized deployment work;
this scaffold includes no remote migration or deployment command.
