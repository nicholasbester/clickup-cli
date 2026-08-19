# Markdown comment support — design

**Date:** 2026-08-19

## Problem

ClickUp's v2 comment API stores `comment_text` as literal text — markdown
syntax is not rendered, as the CLI's help text has long warned. ClickUp does,
however, document a rich format: a Quill-style ops array
(`comment: [{text, attributes}, ...]`) supporting inline `bold`, `italic`,
`code`, and `link` attributes, plus block attributes carried on newline ops:
`code-block`, `list` (`bullet`/`ordered`/`checked`/`unchecked`/`toggled`),
and `indent` (see https://developer.clickup.com/docs/comment-formatting).
Users and agents writing markdown into comments get raw asterisks today.

This feature was independently proven valuable by a community fork; this
design and implementation are clean-room — built from ClickUp's public docs
and this repo's own conventions, without reference to the fork's source.

## Decision

Add a `--markdown` flag to `comment create` and `comment reply`, and a
`markdown: boolean` argument to the MCP `clickup_comment_create` tool. When
set, the `--text` value (including `@file`/`@-` input) is parsed as
CommonMark and submitted as the documented ops array instead of
`comment_text`.

## Approach

- **Parser: `pulldown-cmark`** (new direct dependency). The standard
  lightweight pure-Rust CommonMark parser; its pull-based event stream maps
  naturally onto a flat ops array. Extensions enabled: `ENABLE_TASKLISTS`
  (`- [ ]` → checked/unchecked lists) and `ENABLE_STRIKETHROUGH` (detected
  only so it can be deliberately degraded). Rejected: `comrak` (heavier
  dependency tree, full AST we don't need — the target format can't express
  its extra constructs anyway); hand-rolled parsing (markdown edge cases).

- **New module `src/markdown_ops.rs`** with a single public function:
  `pub fn markdown_to_ops(text: &str) -> Vec<serde_json::Value>`.
  Pure (no I/O), independently unit-testable. Walks pulldown-cmark events
  maintaining an inline-attribute state (bold/italic/code/link nest by
  attribute merging) and emits block-terminator `\n` ops with the
  appropriate block attributes per the documented Quill convention. Nested
  lists map to the `indent` attribute (depth − 1).

- **Degradation rules** (never errors; never emits undocumented API
  surface — consistent with the #104 precedent):
  | Markdown construct | Rendered as |
  |---|---|
  | Heading (any level) | Bold text line |
  | Blockquote | `indent: 1` block |
  | Strikethrough | Plain text (delimiters dropped) |
  | Table / raw HTML / image | Literal text passthrough |
  | Horizontal rule | Line of `---` text |

- **CLI wiring** (`src/commands/comment.rs`): `--markdown` on `Create` and
  `Reply`. When set, POST body is `{"comment": ops, "notify_all": ...}`
  (assignee still included when given) instead of
  `{"comment_text": ...}`. The existing help-text note "markdown syntax is
  stored as literal text" gains "unless --markdown is set".

- **MCP wiring** (`src/mcp.rs`): `clickup_comment_create` gains
  `markdown: boolean` (schema + handler), same body switch.

- **Error handling:** CommonMark parsing is total (never fails). Empty
  text keeps failing upstream exactly as today. If the ops array would be
  empty (e.g. input was only unexpressible constructs reduced to nothing),
  fall back to `comment_text` with the raw input so the comment is never
  silently empty.

## Testing

- Unit tests on `markdown_to_ops` (in-module): each supported construct,
  nesting (bold+italic, links containing code), each degradation rule,
  multi-block documents, and the exact JSON shapes from ClickUp's docs.
- Wiremock end-to-end: CLI `comment create --markdown` and
  `comment reply --markdown` assert the exact `comment` ops body (and that
  `comment_text` is absent); MCP `clickup_comment_create` with
  `markdown: true` likewise; a no-flag regression test pins the plain
  `comment_text` body unchanged.

## Out of scope (v1)

- `comment update --markdown` (Update Comment's acceptance of the ops
  array is unverified; revisit on demand).
- Tables via undocumented table-embed ops; emoticon (`type: "emoticon"`)
  and @mention (`type: "tag"`) ops.
- Doc-page or task-description markdown (already handled elsewhere via
  `markdown_content`).
