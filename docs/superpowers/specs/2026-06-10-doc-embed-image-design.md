# `doc embed-image` — inline images in ClickUp doc pages

**Date:** 2026-06-10
**Status:** Approved

## Problem

The ClickUp API has no endpoint for uploading images to docs or doc pages. Attachments can only be uploaded to tasks (`POST /v2/task/{id}/attachment`) or file-type custom fields (v3). `doc edit-page` accepts markdown text only. Users (primarily AI agents) cannot programmatically drop PNGs inline into a doc page the way drag-and-drop works in the UI.

## Verified facts (live-tested 2026-06-10, workspace 2648001)

- The v3 attachment endpoint (`POST /api/v3/workspaces/{ws}/{entity_type}/{id}/attachments`) rejects `docs`, `pages`, and `tasks` entity types with 404. Only `attachments` (tasks) and `custom_fields` are live, as documented. (The response schema's `parent_entity_type` enum includes `docs`, so native support may ship later.)
- Task attachment upload returns a public, unauthenticated CDN URL (`https://t{ws}.p.clickup-attachments.com/...`).
- `PUT /v3/.../pages/{page}` with markdown `![alt](url)` and `content_edit_mode=append` causes ClickUp to convert the markdown into a **native inline image block**. Re-export shows `![](url)` (alt dropped), confirming block conversion rather than literal text storage.

Conclusion: inline doc images are achievable today as a composite operation — upload to a host task, embed the returned URL as markdown.

## Design

### Command

```
clickup-cli doc embed-image <DOC_ID> <PAGE_ID> <FILE> [--via-task ID] [--alt TEXT] [--mode append|prepend]
```

### Flow

1. Resolve host task: `--via-task` flag → `CLICKUP_TASK_ID` env → git branch detection (same convention as other task-scoped commands; the upload is non-destructive so auto-detect is safe). If nothing resolves, fail with a message explaining the ClickUp API requires a task to host the image binary.
2. Upload `FILE` via existing `client.upload_file()` to `POST /v2/task/{task_id}/attachment`.
3. Take `url` from the upload response.
4. `PUT /v3/workspaces/{ws}/docs/{doc_id}/pages/{page_id}` with body `{"content": "\n![{alt}]({url})\n", "content_edit_mode": "{mode}"}`.

### Defaults and constraints

- `--alt` defaults to the file's name.
- `--mode` accepts `append` (default) or `prepend`. No `replace` — replacing an entire page with one image is almost certainly a mistake, and `edit-page` already covers it.
- Single file per invocation (multi-file embed is a possible follow-up).

### Output

Standard output modes (table/json/json-compact/csv): attachment `url`, `page_id`, `mode`. `-q` prints just the attachment URL.

### Error handling

If the upload succeeds but the page edit fails, report the orphaned attachment URL in the error message so the caller can retry the embed (e.g. via `doc edit-page`) without re-uploading.

### MCP parity

Add a matching `clickup_doc_embed_image` MCP tool in `src/mcp.rs` (params: `doc_id`, `page_id`, `file_path`, `task_id`, optional `alt`, `content_edit_mode`). Agents are the primary consumer of this feature.

### Documentation

Update the usual four places: CLAUDE.md command list, CLAUDE.md embedded agent block, `AGENT_REFERENCE` in `src/agent_config.rs`, `docs/commands.md`; plus `docs/mcp.md` tool count (143 → 144).

### Testing

TDD per house style:
- Unit tests for markdown snippet construction (alt defaulting, mode plumbing).
- CLI smoke tests in `tests/test_cli` (arg parsing, missing-task error message).
- Output-shape test in `tests/test_output`.

## Out of scope (follow-ups)

- Migrating `attachment list` from scraping `GET /v2/task/{id}` to the new v3 `Get Attachments` endpoint (cursor-paginated, thumbnails).
- Multi-file embed.
- Native doc attachment upload if/when ClickUp opens the `docs` entity type on the v3 attachment endpoint.
