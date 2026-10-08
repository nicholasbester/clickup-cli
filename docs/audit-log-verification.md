# Enterprise audit-log verification (#59)

Status: **live verification outstanding**. Offline request regressions do not establish the Enterprise response contract. This checklist is for an Enterprise Workspace owner with authorized access; no credentials or live Workspace calls were used to prepare it.

## Published contract checked on 2026-10-08

Sources: [audit endpoint reference](https://developer.clickup.com/reference/queryauditlog), [OpenAPI index](https://developer.clickup.com/docs/open-api-spec), and its [v3 specification](https://developer.clickup.com/openapi/ClickUp_PUBLIC_API_V3.yaml). Retrieved v3 SHA-256: `167e0b99e0c2218312d1318fff613f180dccdfcb8decb724ce08559aa04329f2`.

- Applicability has six categories, including `auth-and-security`; `WORKSPACE`, `TEAMS`, and `USERS` are not documented values.
- User IDs/emails are string arrays. Event types are specific events such as `USER_LOGIN`; status values are lowercase (`success`, `failed`, `warn`, `skipped`, `started`, `completed`, `error`, `system_error`).
- `startTime`, `endTime`, `pageRows`, and `pageTimestamp` have numeric request schemas; the request example uses millisecond timestamps. No inclusive/exclusive filter-boundary promise is documented.
- `pageDirection` documents `before`/`after`, recommending `before`. First-page timestamp is current time; continuation uses the previous page's last-row timestamp. Legacy CLI/MCP aliases map `PREVIOUS` → `before`, `NEXT` → `after`. Omission does not insert a default.
- `eventType` has a **string schema but an array example**. Existing scalar handling follows the schema; array support is unresolved. `filter.workspaceId` also has conflicting schema/example types and is not exposed; Workspace selection remains in the URL.
- HTTP 200 contains no response schema. The [Help event-log example](https://help.clickup.com/hc/en-us/articles/21929900448535-Workspace-audit-logs) shows ISO `startTime`/`endTime` strings for an internal event display. That is not proof of the API response or grounds to change numeric request types.

## Existing response assumptions to verify

CLI extraction tries `data`, then `audit_logs`; MCP tries `data`, then `events`; both also accept a top-level array. These are existing candidates, not confirmed provider fields. CLI's default columns include `createdAt`; MCP projects `id`, `eventType`, `eventStatus`, `userId`, `eventTime`. Neither projection proves those fields exist.

Both walkers advance using the last item's numeric or numeric-string `eventTime`, `timestamp`, or `date`. ISO timestamps and `startTime` are not recognized. No new response fields have been guessed in this fix.

Unknown response containers currently become empty lists; missing timestamps stop walking. MCP can report `has_more: false` in those cases without proving exhaustion. Repeated boundaries can run to the 100-page cap, and limit truncation reports the last *fetched* boundary, potentially skipping unreturned items on continuation. Do not treat an empty result, `has_more: false`, or a capped walk as a complete audit export without comparing the raw provider response. These are explicit limitations pending #59 evidence.

## Read-only owner checklist

Use existing events; do not create users/tasks/events for this check. Record CLI version and commit, test time, Enterprise access/owner role, sanitized request arguments, HTTP status, and result counts. Use local configured credentials; never paste tokens, authorization headers, emails, IPs, Workspace IDs, or raw audit events into a report.

Start MCP with only this query tool:

```sh
clickup-cli mcp serve --read-only --tools clickup_audit_log_query
```

Use the following arguments with `clickup_audit_log_query` (default Workspace already configured). Replace `NOW_MS` with the current integer Unix timestamp in milliseconds. Keep the same boundary for comparable calls.

1. **Bare-array compatibility:** `{"applicability":"auth-and-security"}`. Check whether the client returns an array and whether compact entries retain meaningful fields. This checks the existing no-pagination client contract; an error from omitted pagination is evidence to retain.
2. **Envelope:** `{"applicability":"auth-and-security","page_rows":10,"page_timestamp":NOW_MS,"page_direction":"before"}`. Record `items` and all `pagination` keys/types, returned count, extracted next boundary, and raw page count. Continue once using the returned numeric `next_page_timestamp`, only if it matches the actual last-row timestamp.
3. **All with limit:** `{"applicability":"auth-and-security","page_rows":10,"page_timestamp":NOW_MS,"page_direction":"before","all":true,"limit":25}`. Use a category/window with more than 25 existing events. Check counts, duplicate IDs/boundaries, ordering, and whether multiple pages were fetched. If fewer events exist, mark multi-page/limit behavior inconclusive. A second bounded run with a limit above the known window count can check natural termination. The helper's 100-page cap is a guard, not completeness evidence.
4. **PREVIOUS compatibility:** repeat step 3 with `"page_direction":"PREVIOUS"`. Verify the request uses `before` and the same starting boundary produces consistent results. The envelope retains the caller's direction spelling. Check that continuation advances toward older events and terminates; do not infer this from the alias name alone.

Also compare a CLI single page (`--output json`) with the same arguments. CLI JSON retains extracted event objects but **not the raw provider envelope**. To capture the latter, an authorized owner can submit the corresponding query in their API client to `POST /api/v3/workspaces/{workspace_id}/auditlogs` with this body (replace `NOW_MS`):

```json
{
  "applicability": "auth-and-security",
  "pagination": {"pageRows": 10, "pageTimestamp": NOW_MS, "pageDirection": "before"}
}
```

Capture a sanitized **structural** response: top-level JSON type, exact container/metadata key names and types, array length, event field names/types, timestamp field/representation/unit, ordering, and whether page boundaries repeat. Preserve null/missing distinctions. Use consistent pseudonyms for IDs to identify duplicates; for timestamps, preserve format, precision, relative order, and equality while shifting actual dates consistently. Report an error's sanitized status/code without treating it as a successful empty page. Do not share the unredacted response.

Record PASS / FAIL / INCONCLUSIVE separately for each step and include the structural sample alongside client outputs. #59 remains open until real Enterprise evidence establishes the container, event fields, timestamp extraction, and forward/backward continuation. Mock-based tests and this documentation update cannot close it.
