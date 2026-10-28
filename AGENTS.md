# Agent guide

Read `CONTRIBUTING.md` first; everything there applies.

- Pick the model from the issue's label: `model:astra` (design only) uses
  `-m gpt-6-astra -c model_reasoning_effort=xhigh`; `model:sol` (implementation) uses
  `-m gpt-6.1-sol`.
- Work in a git worktree per issue. Never delete folders you did not create.
- Never run `wrangler deploy` (or any production deploy) without Calum's explicit OK.
- Never commit secrets, API tokens, or personal info. Tokens are stored hashed only.
- Before opening a PR run: `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- UI changes: attach before/after dark-mode screenshots to the PR.
- If a convention is missing, add it to `CONTRIBUTING.md` / this file in the same PR.
- The agent skill for driving Tickets lives in `.agents/skills/tickets/SKILL.md`; keep it
  current when the API changes.
