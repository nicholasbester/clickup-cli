# MCP: surface `custom_id` in task responses — design

**Date:** 2026-07-22
**Issue:** [#82 — MCP: clickup_task_get / clickup_task_search omit custom_id metadata](https://github.com/nicholasbester/clickup-cli/issues/82)

## Problem

The ClickUp API returns `custom_id` on every task object (null when the
workspace doesn't use custom task IDs). The MCP server drops it: every task
tool projects the API response through a fixed field whitelist via
`output::compact_items`, and `custom_id` is not in any of those whitelists.
MCP consumers therefore cannot display or prefer custom IDs, breaking parity
with the CLI, where `--fields custom_id` or `--output json` exposes the value.

No request-side change is needed. The `custom_task_ids=true&team_id=<ws>`
query parameter only affects *addressing* a task by its custom ID on input,
which `resolve_task` in `src/mcp.rs` already handles.

## Decision

Include `custom_id` in the compact output of every MCP tool that returns task
objects, omitting the key entirely when the task has no custom ID so
workspaces that don't use the feature pay zero token overhead.

**Affected tools (6):** `clickup_task_get`, `clickup_task_search`,
`clickup_task_list`, `clickup_task_create`, `clickup_task_update`,
`clickup_view_tasks`. (`clickup_task_move` was originally in scope but
returns only a `{"message": ...}` confirmation, not a task object, so
there is nothing to add a field to.) (`clickup_template_apply_task` also
returns a task object but keeps its deliberately minimal id/name
projection — possible follow-up.)

## Approach

Teach `compact_items` an optional-field convention: a field spelled with a
trailing `?` (e.g. `"custom_id?"`) is stripped to its clean name and included
in the output object only when the source value is present and non-null.
Fields without the marker keep today's behaviour exactly (always present,
`"-"` placeholder when missing).

Alternatives rejected:

- **Separate `compact_items_opt` / extra `optional_fields` parameter** —
  requires threading a new parameter through all four pagination dispatch
  functions (`page_dispatch`, `cursor_dispatch`, `start_id_dispatch`,
  `body_dispatch`) and every call site. High churn for one field.
- **Post-processing re-injection** — raw items are not accessible after
  paginated dispatch; fragile.
- **Always include with `"-"` placeholder** — uniform schema but adds ~6
  tokens per task row for every workspace, against the server's
  token-efficiency goal.

## Changes

1. `src/output.rs` — `compact_items`: detect trailing `?` on a field name;
   strip it for the output key; skip insertion when the value is missing or
   JSON null. Document the convention in the doc comment.
2. `src/mcp.rs` — append `"custom_id?"` to the compact field lists of the 6
   tools above. Update tool descriptions that enumerate returned fields
   (e.g. `clickup_task_list`) to note `custom_id` is included when set.
3. Tests:
   - `compact_items` unit tests: optional field present → emitted under the
     clean key; null/missing → key omitted; non-optional fields unchanged.
   - MCP-level assertions that the task tools' field projections include
     `custom_id` when the API payload carries one and omit it when null.

## Backward compatibility

Purely additive. Tasks without a custom ID produce byte-identical output to
today. Existing keys, their order semantics, and placeholder behaviour are
unchanged.
