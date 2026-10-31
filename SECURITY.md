# Security policy

## Supported versions

Only the current `main` branch receives security fixes. There are no supported
older release lines.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting for
[0x63616c/tickets](https://github.com/0x63616c/tickets/security/advisories/new).
If that form is unavailable, ask the maintainers to enable private vulnerability
reporting through the repository's Security tab without disclosing details.
Do not report vulnerabilities in public issues or pull requests.

Include the affected commit, reproduction steps, expected and observed behavior,
and likely impact. Use disposable local data and redact credentials. Do not probe
other people's instances or include their data in a report. Coordinate disclosure
privately through the security advisory.

## Scope and security model

Reports may cover the Worker API, login and tokens, frontend rendering, D1 access,
Durable Object live transport, GitHub integration, importer and repository tooling.
Include dependency vulnerabilities when you can explain their impact on Tickets.
Deployment credentials and Cloudflare/GitHub account security are also part of
an operator's responsibility; see [self-hosting](docs/SELF-HOSTING.md).

Tickets currently has a single bootstrap login and no signup, invitation or
project-level access controls. Valid sessions and personal API tokens have full
workspace access; tokens are not scoped or read-only. Deploy separate instances
for separate trust boundaries.

## Protections in place

- Login uses a salted PBKDF2-SHA256 hash held in a Worker secret. New hashes use
  100,000 iterations; [ADR 0003](docs/adr/0003-auth.md) explains the runtime limit
  and tradeoff. The password hash is not stored in D1.
- Login attempts are limited to five per hashed source IP per 15-minute window,
  including successful attempts. Credential comparisons use constant-time helpers.
- Sessions and personal API tokens are stored as SHA-256 hashes. Sessions expire
  after seven days; personal tokens stay active until revoked. Raw API tokens are
  shown once on creation.
- Session cookies are HttpOnly, Secure and SameSite=Lax. Cookie-authenticated
  mutations and WebSocket upgrades require a matching Origin.
- GitHub webhooks verify HMAC-SHA256 signatures over raw request bytes. App
  credentials are encrypted with AES-256-GCM in D1 using a Worker secret key;
  manifest registration requires a secret setup state.
- User Markdown is sanitized before rendering. Private API routes and live
  subscriptions share authentication checks.
- CI runs gitleaks over repository history, alongside Rust checks and integration
  checks. Secret scanning does not replace credential rotation after exposure.
