# Comment formatting v2 — design

**Date:** 2026-08-28

## Problem

The v1 markdown-comments feature (0.16.0, spec `2026-08-19-markdown-comments-design.md`)
left three gaps:

1. The MCP `clickup_comment_reply` tool lacks the `markdown` argument its CLI
   twin has (#120).
2. `comment update` has no `--markdown` — v1 scoped it out because ops-on-update
   was unverified; ClickUp's docs page in fact states formatting applies to
   both adding **and updating** comments
   (https://developer.clickup.com/docs/comment-formatting), so it is
   documented-legitimate.
3. @mentions render as dead text. The API documents a native mention op:
   `{"type": "tag", "user": {"id": 1234567}}`, which notifies the user.

## Decision

Three sequenced additions, one feature branch:

1. **MCP `clickup_comment_reply`**: add `markdown: boolean` to the tool schema;
   handler switches its body through the existing
   `markdown_ops::comment_body(markdown, text)`.
2. **`comment update --markdown`** (CLI) and a matching `markdown` argument on
   the MCP `clickup_comment_update` tool. Body switches through
   `comment_body`; the existing `resolved`/`assignee` insertions apply to both
   shapes unchanged. Without the flag, bodies stay byte-identical.
3. **Mentions in markdown mode**: a CommonMark link whose destination uses the
   `user:` scheme — `[@Nick](user:81618)` — emits the documented tag op
   `{"type": "tag", "user": {"id": 81618}}` instead of a linked-text op.
   - The link's display text is dropped (ClickUp renders the member's real
     name); it exists for source readability.
   - The id must parse as an unsigned integer; otherwise the link is treated
     as a normal link (graceful, never errors — consistent with v1's
     degradation philosophy).
   - Inline styling wrapped around a mention is ignored (tag ops carry no
     attributes).
   - Applies everywhere `markdown_to_ops` is used (create/reply/update, CLI
     and MCP) with no per-caller changes.

## Testing

- Unit tests on `markdown_to_ops`: mention op shape, mention mid-sentence,
  non-numeric `user:` dest degrades to a normal link, styled mention drops
  styling, mention adjacent to text preserves surrounding ops.
- Wiremock end-to-end: MCP reply with `markdown: true` (ops body asserted),
  CLI `comment update --markdown` (ops + `resolved` in one body), MCP update
  with `markdown: true`, no-flag update byte-identical guard, mention op
  asserted in a create body.
- **Real-API smoke test** (explicit user requirement): against the user's
  configured workspace — create a throwaway task, post a markdown comment
  exercising bold/list/code/mention (self-mention via `auth whoami` id),
  update it with `--markdown`, reply with markdown via MCP, read back the
  comments to verify ClickUp accepted and structured them, then delete the
  task. Documented as a manual verification step, not committed as an
  automated test.

## Out of scope

- Emoticon shortcode conversion and toggled lists (declined by maintainer).
- Any undocumented attributes (underline/color/strikethrough/etc.).
