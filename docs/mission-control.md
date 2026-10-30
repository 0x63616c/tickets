# Mission Control v1

`/mission` is a dark, cross-project operating queue, linked beside Home. It shows
Needs Calum, Ready to test, Running jobs, Open PRs, Blocked, Recent merges and
Builds, in that order. It refreshes every 30 seconds while visible and on focus
or visibility restoration. Failed refreshes retain the last snapshot with Retry.

## Report ingest

`PUT /api/v1/mission/reports/{source}` uses normal bearer/session authentication;
cookie writes require the matching Origin. Source is 1–40 ASCII characters from
`[a-z0-9-]`. Body must be a JSON object, at most 524288 UTF-8 bytes (512 KiB).
Oversized reports return 413; invalid JSON, source or list shapes return 422.
The raw JSON replaces the source's entire previous report, including omitted
lists. Unknown fields are retained but ignored by v1. Success is 200:

```json
{"source":"box","received_at":"2026-10-30T20:00:00.000Z"}
```

Every list is optional; present lists must be arrays of objects. Producers use:

```json
{
  "generated_at":"2026-10-30T20:00:00Z",
  "jobs":[{"repo":"tickets","ticket":"TIK-77","branch":"TIK-77-mission-control","kind":"run","state":"running","pr":97,"ci":"pass","age":"41m","detail":"Building"}],
  "pull_requests":[{"repo":"example/tickets","number":97,"title":"Mission Control","url":"https://github.com/example/tickets/pull/97","state":"open","ci":"pass","created_at":"2026-10-30T19:00:00Z","updated_at":"2026-10-30T20:00:00Z","ticket_key":"TIK-77","auto_merge":true}],
  "merges":[{"repo":"example/tickets","number":96,"title":"History","url":"https://github.com/example/tickets/pull/96","merged_at":"2026-10-30T19:00:00Z","ticket_key":"TIK-60"}],
  "builds":[{"app":"Tickets","kind":"deploy","build_number":"130","status":"success","url":"https://github.com/example/tickets/actions/runs/1","finished_at":"2026-10-30T19:00:00Z","commit":"abc123","title":"Deploy ready"}],
  "blocked":[{"repo":"tickets","pr":94,"reason":"Merge conflict","url":"https://github.com/example/tickets/pull/94","since":"2026-10-30T19:00:00Z"}],
  "needs_calum":[{"title":"Approve installation","detail":"Review access","url":"https://github.com/settings/installations","ticket_key":"TIK-70"}]
}
```

CI values: `pass|fail|pending|none`. Build kinds: `testflight|deploy`; statuses:
`success|failure|in_progress`. Missing optional item metadata remains readable;
items link to their HTTP(S) URL or ticket key when supplied. Jobs in running,
queued, starting or in_progress state appear in Running jobs. Only open/draft
PRs appear in Open PRs. PRs are deduplicated by URL against ticket-linked PRs.

## Aggregated read

`GET /api/v1/mission` returns:

- `generated_at`: server UTC timestamp.
- `reports`: `{source, received_at, stale, report}` for every stored source. A
  report is stale strictly after ten minutes by server receipt time. `report` is
  the parsed original object, including unknown fields.
- `waiting_on_calum`: tickets labelled `waiting-on-calum`, case-insensitively.
- `in_review`: tickets in review, including `pull_requests` from
  `ticket_pull_requests`, with `ci_status` for the matching current head SHA
  from `github_pull_request_checks` (`none` if unknown).
- `recently_done`: current Done tickets whose last observed transition to Done
  occurred in the last 48 hours (including tickets created Done), newest first, maximum 30. Later edits do not
  extend this window. Historical completion dates are not inferred from edits.

All ticket sections exclude archived projects and include `id`, `key`, `title`,
`project` (project name), `status`, `updated_at`, `labels` (`id`, `name`) and
`pull_requests`. Reads use one D1 batch with three statements, plus normal auth.
Ready to test combines recently_done with successful TestFlight/deploy builds
finished within the last 48 hours. Stale reports remain visible with a warning.

## Approval and fixtures

Approve optimistically hides a waiting ticket, DELETEs its waiting label and
POSTs `{"body":"Approved from Mission Control."}` to its comments endpoint.
If the comment fails, the label is restored where possible and the error is
shown. A failed restoration explicitly asks the user to reconcile the ticket.
Box-reported items only link out. No other dashboard actions are provided.

`/design/mission` and `/design/c/mission` use sample data from `fixtures.rs` with
no backend. `cargo xtask mission-check` verifies approval persistence, comment-failure
compensation, focus refresh and retained content after a failed refresh.
`cargo xtask component-shot mission` captures the component plus
desktop/mobile full-page fixtures. The shared `.page-container` uses
`--page-max-width` for the centred page measure.
