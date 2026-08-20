# Markdown Comment Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `comment create --markdown` / `comment reply --markdown` (and MCP `clickup_comment_create` `markdown: true`) parse the comment text as CommonMark and submit ClickUp's documented rich-comment ops array instead of plain `comment_text`.

**Architecture:** A new pure module `src/markdown_ops.rs` converts markdown to `Vec<serde_json::Value>` ops by walking pulldown-cmark's event stream with an inline-attribute state machine (bold/italic/link nest by merging; inline code arrives atomically) and Quill-convention block terminators (`\n` ops carrying `code-block`/`list`/`indent` attributes). Callers switch the POST body between `comment_text` and `comment` based on the flag.

**Tech Stack:** Rust, pulldown-cmark 0.13 (new direct dependency, `default-features = false`), serde_json, wiremock + assert_cmd for integration tests.

**Spec:** `docs/superpowers/specs/2026-08-19-markdown-comments-design.md`

## Global Constraints

- Only ClickUp-documented ops may be emitted: inline `bold`, `italic`, `code`, `link`; block (on `\n` ops) `code-block: {"code-block": "plain"}`, `list: {"list": "bullet"|"ordered"|"checked"|"unchecked"}`, `indent: N`. (https://developer.clickup.com/docs/comment-formatting)
- Degradation rules (verbatim from spec): heading → bold text line; blockquote → `indent: 1` block; strikethrough → plain text (delimiters dropped); table / raw HTML / image → literal text passthrough; horizontal rule → line of `---` text.
- Conversion never errors. If the resulting ops array is empty, the caller falls back to plain `comment_text` with the raw input.
- Without `--markdown`, request bodies must be byte-identical to today.
- Clean-room constraint: do not consult any fork's implementation; ClickUp's docs page and this repo are the only references.
- Work on branch `feat/markdown-comments` (already checked out; spec committed at its tip).
- pulldown-cmark extensions enabled: `ENABLE_TASKLISTS`, `ENABLE_STRIKETHROUGH` only (tables NOT enabled — table syntax then flows through as plain paragraph text, which is exactly the required passthrough).

---

### Task 1: `markdown_ops` module

**Files:**
- Modify: `Cargo.toml` (add `pulldown-cmark = { version = "0.13", default-features = false }` to `[dependencies]`)
- Create: `src/markdown_ops.rs`
- Modify: `src/lib.rs` (add `pub mod markdown_ops;` beside the existing `pub mod` lines)
- Test: in-module `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: nothing from other tasks.
- Produces: `pub fn markdown_to_ops(text: &str) -> Vec<serde_json::Value>` in `clickup_cli::markdown_ops`. Each element is either `{"text": "..."}` (plain), `{"text": "...", "attributes": {...}}` (attributed), and line/block terminators are `{"text": "\n"}` or `{"text": "\n", "attributes": {<block attrs>}}`. Tasks 2–3 call exactly this function.

- [ ] **Step 1: Write the failing tests**

Create `src/markdown_ops.rs` containing ONLY the test module for now (the implementer adds the implementation in Step 3; the tests reference the not-yet-written function):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn plain_paragraph() {
        assert_eq!(
            markdown_to_ops("hello world"),
            vec![json!({"text": "hello world"}), json!({"text": "\n"})]
        );
    }

    #[test]
    fn bold_italic_and_nesting() {
        assert_eq!(
            markdown_to_ops("a **b** *c* **_d_**"),
            vec![
                json!({"text": "a "}),
                json!({"text": "b", "attributes": {"bold": true}}),
                json!({"text": " "}),
                json!({"text": "c", "attributes": {"italic": true}}),
                json!({"text": " "}),
                json!({"text": "d", "attributes": {"bold": true, "italic": true}}),
                json!({"text": "\n"}),
            ]
        );
    }

    #[test]
    fn inline_code_and_link() {
        assert_eq!(
            markdown_to_ops("run `ls` at [site](https://x.io)"),
            vec![
                json!({"text": "run "}),
                json!({"text": "ls", "attributes": {"code": true}}),
                json!({"text": " at "}),
                json!({"text": "site", "attributes": {"link": "https://x.io"}}),
                json!({"text": "\n"}),
            ]
        );
    }

    #[test]
    fn fenced_code_block_lines_carry_block_attr() {
        assert_eq!(
            markdown_to_ops("```\nlet x = 1;\nlet y = 2;\n```"),
            vec![
                json!({"text": "let x = 1;"}),
                json!({"text": "\n", "attributes": {"code-block": {"code-block": "plain"}}}),
                json!({"text": "let y = 2;"}),
                json!({"text": "\n", "attributes": {"code-block": {"code-block": "plain"}}}),
            ]
        );
    }

    #[test]
    fn bullet_and_ordered_lists() {
        assert_eq!(
            markdown_to_ops("- one\n- two"),
            vec![
                json!({"text": "one"}),
                json!({"text": "\n", "attributes": {"list": {"list": "bullet"}}}),
                json!({"text": "two"}),
                json!({"text": "\n", "attributes": {"list": {"list": "bullet"}}}),
            ]
        );
        assert_eq!(
            markdown_to_ops("1. one\n2. two"),
            vec![
                json!({"text": "one"}),
                json!({"text": "\n", "attributes": {"list": {"list": "ordered"}}}),
                json!({"text": "two"}),
                json!({"text": "\n", "attributes": {"list": {"list": "ordered"}}}),
            ]
        );
    }

    #[test]
    fn task_lists_map_to_checked_unchecked() {
        assert_eq!(
            markdown_to_ops("- [ ] todo\n- [x] done"),
            vec![
                json!({"text": "todo"}),
                json!({"text": "\n", "attributes": {"list": {"list": "unchecked"}}}),
                json!({"text": "done"}),
                json!({"text": "\n", "attributes": {"list": {"list": "checked"}}}),
            ]
        );
    }

    #[test]
    fn nested_list_uses_indent() {
        assert_eq!(
            markdown_to_ops("- top\n  - nested"),
            vec![
                json!({"text": "top"}),
                json!({"text": "\n", "attributes": {"list": {"list": "bullet"}}}),
                json!({"text": "nested"}),
                json!({"text": "\n", "attributes": {"list": {"list": "bullet"}, "indent": 1}}),
            ]
        );
    }

    #[test]
    fn heading_degrades_to_bold_line() {
        assert_eq!(
            markdown_to_ops("## Title"),
            vec![
                json!({"text": "Title", "attributes": {"bold": true}}),
                json!({"text": "\n"}),
            ]
        );
    }

    #[test]
    fn blockquote_degrades_to_indent() {
        assert_eq!(
            markdown_to_ops("> quoted"),
            vec![
                json!({"text": "quoted"}),
                json!({"text": "\n", "attributes": {"indent": 1}}),
            ]
        );
    }

    #[test]
    fn strikethrough_degrades_to_plain() {
        assert_eq!(
            markdown_to_ops("~~gone~~"),
            vec![json!({"text": "gone"}), json!({"text": "\n"})]
        );
    }

    #[test]
    fn rule_degrades_to_dashes() {
        assert_eq!(
            markdown_to_ops("a\n\n---\n\nb"),
            vec![
                json!({"text": "a"}),
                json!({"text": "\n"}),
                json!({"text": "---"}),
                json!({"text": "\n"}),
                json!({"text": "b"}),
                json!({"text": "\n"}),
            ]
        );
    }

    #[test]
    fn table_syntax_passes_through_as_text() {
        // Tables extension is NOT enabled, so pipe rows are plain paragraph
        // text (single paragraph with soft breaks rendered as spaces).
        let ops = markdown_to_ops("| a | b |\n|---|---|\n| 1 | 2 |");
        let joined: String = ops
            .iter()
            .filter_map(|o| o.get("text").and_then(|t| t.as_str()))
            .collect();
        assert!(joined.contains("| a | b |"), "got: {joined}");
        assert!(joined.contains("| 1 | 2 |"), "got: {joined}");
    }

    #[test]
    fn hard_break_is_newline_soft_break_is_space() {
        assert_eq!(
            markdown_to_ops("a  \nb\nc"),
            vec![
                json!({"text": "a"}),
                json!({"text": "\n"}),
                json!({"text": "b"}),
                json!({"text": " "}),
                json!({"text": "c"}),
                json!({"text": "\n"}),
            ]
        );
    }

    #[test]
    fn empty_input_yields_empty_ops() {
        assert!(markdown_to_ops("").is_empty());
        assert!(markdown_to_ops("   \n").is_empty());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail to compile**

Run: `cargo test --lib markdown_ops`
Expected: compile error — `markdown_to_ops` not found (module has tests only). Add the dependency first (`Cargo.toml` `[dependencies]` section, alphabetical position):

```toml
pulldown-cmark = { version = "0.13", default-features = false }
```

and `pub mod markdown_ops;` to `src/lib.rs`, then confirm the compile failure names the missing function.

- [ ] **Step 3: Write the implementation**

Prepend to `src/markdown_ops.rs` (above the test module):

```rust
//! Markdown → ClickUp rich-comment ops (Quill-style), per ClickUp's
//! documented comment-formatting contract:
//! https://developer.clickup.com/docs/comment-formatting
//!
//! Inline attributes (`bold`, `italic`, `code`, `link`) attach to text
//! ops; block formatting (`code-block`, `list`, `indent`) attaches to the
//! `"\n"` op terminating each line. Constructs the format cannot express
//! degrade per the design spec (headings → bold, blockquotes → indent,
//! strikethrough → plain, tables/HTML/images → literal text). Only
//! documented ops are emitted.

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use serde_json::{json, Map, Value};

#[derive(Clone, Copy, PartialEq)]
enum ListKind {
    Bullet,
    Ordered,
}

/// Convert CommonMark to ClickUp comment ops. Never fails; an input with
/// no expressible content yields an empty vec (callers fall back to
/// comment_text).
pub fn markdown_to_ops(text: &str) -> Vec<Value> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(text, options);

    let mut ops: Vec<Value> = Vec::new();
    // Inline state
    let mut bold: u32 = 0;
    let mut italic: u32 = 0;
    let mut link: Option<String> = None;
    // Block state
    let mut list_stack: Vec<ListKind> = Vec::new();
    let mut item_task_state: Option<bool> = None; // Some(checked) from TaskListMarker
    let mut in_code_block = false;
    let mut blockquote_depth: u32 = 0;
    let mut heading_depth: u32 = 0;

    let mut push_text = |ops: &mut Vec<Value>,
                         s: &str,
                         bold: u32,
                         italic: u32,
                         link: &Option<String>,
                         code: bool,
                         heading: u32| {
        if s.is_empty() {
            return;
        }
        let mut attrs = Map::new();
        if bold > 0 || heading > 0 {
            attrs.insert("bold".into(), Value::Bool(true));
        }
        if italic > 0 {
            attrs.insert("italic".into(), Value::Bool(true));
        }
        if code {
            attrs.insert("code".into(), Value::Bool(true));
        }
        if let Some(url) = link {
            attrs.insert("link".into(), Value::String(url.clone()));
        }
        if attrs.is_empty() {
            ops.push(json!({"text": s}));
        } else {
            ops.push(json!({"text": s, "attributes": Value::Object(attrs)}));
        }
    };

    // Terminate the current line with the block attributes in scope.
    let line_end = |ops: &mut Vec<Value>,
                    list_stack: &[ListKind],
                    item_task_state: Option<bool>,
                    blockquote_depth: u32| {
        let mut attrs = Map::new();
        if let Some(kind) = list_stack.last() {
            let list_name = match (item_task_state, kind) {
                (Some(true), _) => "checked",
                (Some(false), _) => "unchecked",
                (None, ListKind::Bullet) => "bullet",
                (None, ListKind::Ordered) => "ordered",
            };
            attrs.insert("list".into(), json!({ "list": list_name }));
            if list_stack.len() > 1 {
                attrs.insert("indent".into(), json!(list_stack.len() - 1));
            }
        } else if blockquote_depth > 0 {
            attrs.insert("indent".into(), json!(blockquote_depth));
        }
        if attrs.is_empty() {
            ops.push(json!({"text": "\n"}));
        } else {
            ops.push(json!({"text": "\n", "attributes": Value::Object(attrs)}));
        }
    };

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Strong => bold += 1,
                Tag::Emphasis => italic += 1,
                Tag::Strikethrough => {} // degrade: text passes through plain
                Tag::Link { dest_url, .. } => link = Some(dest_url.to_string()),
                Tag::Image { dest_url, .. } => {
                    // Literal passthrough: reconstruct as markdown text.
                    push_text(&mut ops, "![", bold, italic, &None, false, heading_depth);
                    // alt text arrives as Text events; the closing is
                    // emitted on TagEnd::Image below.
                    link = Some(format!("__IMG__{}", dest_url));
                }
                Tag::Heading { .. } => heading_depth += 1,
                Tag::BlockQuote(_) => blockquote_depth += 1,
                Tag::CodeBlock(kind) => {
                    in_code_block = true;
                    let _ = kind; // language fences degrade to "plain"
                }
                Tag::List(start) => {
                    list_stack.push(if start.is_some() {
                        ListKind::Ordered
                    } else {
                        ListKind::Bullet
                    });
                }
                Tag::Item => item_task_state = None,
                Tag::Paragraph | Tag::HtmlBlock => {}
                _ => {}
            },
            Event::End(tag_end) => match tag_end {
                TagEnd::Strong => bold = bold.saturating_sub(1),
                TagEnd::Emphasis => italic = italic.saturating_sub(1),
                TagEnd::Strikethrough => {}
                TagEnd::Link => link = None,
                TagEnd::Image => {
                    // close the literal image reconstruction
                    let url = link
                        .take()
                        .and_then(|l| l.strip_prefix("__IMG__").map(str::to_string))
                        .unwrap_or_default();
                    push_text(
                        &mut ops,
                        &format!("]({})", url),
                        bold,
                        italic,
                        &None,
                        false,
                        heading_depth,
                    );
                }
                TagEnd::Heading(_) => {
                    heading_depth = heading_depth.saturating_sub(1);
                    line_end(&mut ops, &list_stack, item_task_state, 0);
                }
                TagEnd::BlockQuote(_) => {
                    blockquote_depth = blockquote_depth.saturating_sub(1)
                }
                TagEnd::CodeBlock => in_code_block = false,
                TagEnd::List(_) => {
                    list_stack.pop();
                }
                TagEnd::Item => {
                    line_end(&mut ops, &list_stack, item_task_state, blockquote_depth);
                    item_task_state = None;
                }
                TagEnd::Paragraph => {
                    // Inside a list item the Item end emits the terminator;
                    // a bare paragraph terminates its own line.
                    if list_stack.is_empty() {
                        line_end(&mut ops, &list_stack, None, blockquote_depth);
                    }
                }
                _ => {}
            },
            Event::Text(t) => {
                if in_code_block {
                    // Code block text can contain multiple lines; each line
                    // gets its own op + code-block terminator.
                    for line in t.lines() {
                        if !line.is_empty() {
                            ops.push(json!({"text": line}));
                        }
                        ops.push(json!({
                            "text": "\n",
                            "attributes": {"code-block": {"code-block": "plain"}}
                        }));
                    }
                } else {
                    // Image alt-text arrives while `link` holds the __IMG__
                    // marker; render it plain (part of the literal form).
                    let effective_link = match &link {
                        Some(l) if l.starts_with("__IMG__") => None,
                        other => other.clone(),
                    };
                    push_text(
                        &mut ops,
                        &t,
                        bold,
                        italic,
                        &effective_link,
                        false,
                        heading_depth,
                    );
                }
            }
            Event::Code(t) => {
                push_text(&mut ops, &t, bold, italic, &link, true, heading_depth)
            }
            Event::Html(t) | Event::InlineHtml(t) => {
                push_text(&mut ops, &t, bold, italic, &None, false, heading_depth)
            }
            Event::SoftBreak => {
                push_text(&mut ops, " ", bold, italic, &None, false, heading_depth)
            }
            Event::HardBreak => ops.push(json!({"text": "\n"})),
            Event::Rule => {
                ops.push(json!({"text": "---"}));
                ops.push(json!({"text": "\n"}));
            }
            Event::TaskListMarker(checked) => item_task_state = Some(checked),
            _ => {}
        }
    }
    ops
}
```

Implementation notes for the engineer:
- Verify names against pulldown-cmark 0.13 docs (`docs.rs/pulldown-cmark/0.13`): `Tag::Link { dest_url, .. }`, `TagEnd::Heading(_)`, `TagEnd::BlockQuote(_)`, `Event::InlineHtml` all exist in 0.13; adjust pattern arity if the compiler disagrees — the semantics above are the contract, exact enum shapes may need touch-ups.
- The `push_text` closure takes params (not captured state) so borrows stay simple; if the borrow checker objects, convert both closures to free functions taking `&mut Vec<Value>`.
- Heading degradation = treat heading scope as bold (`heading_depth` folded into the bold check).
- The `__IMG__` marker never reaches output: alt text renders plain between the literal `![` and `](url)` fragments.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib markdown_ops`
Expected: all 14 tests PASS. If a specific expectation mismatches pulldown-cmark's actual event order, fix the IMPLEMENTATION to produce the specified ops (the test vectors are the contract — they encode ClickUp's documented format); only reshape a test if it contradicts CommonMark semantics itself, and say so in the report.

- [ ] **Step 5: Full-suite sanity + commit**

Run: `cargo test --lib && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: green/clean. Then:

```bash
git add Cargo.toml Cargo.lock src/lib.rs src/markdown_ops.rs
git commit -m "feat: markdown_ops module — CommonMark to ClickUp comment ops"
```

---

### Task 2: CLI `--markdown` on `comment create` and `comment reply`

**Files:**
- Modify: `src/commands/comment.rs` (Create and Reply variants + handlers; help-text notes)
- Test: create `tests/test_comment_markdown.rs`

**Interfaces:**
- Consumes: `clickup_cli::markdown_ops::markdown_to_ops(&str) -> Vec<serde_json::Value>` from Task 1.
- Produces: CLI behavior only (no new interfaces).

- [ ] **Step 1: Write the failing tests**

Create `tests/test_comment_markdown.rs`:

```rust
//! `comment create --markdown` / `comment reply --markdown` submit
//! ClickUp's documented rich-comment ops array instead of comment_text.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;
use tempfile::TempDir;
use wiremock::matchers::{body_partial_json, method, path as path_matcher};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn clickup(dir: &Path, server: &MockServer) -> Command {
    let mut cmd = Command::cargo_bin("clickup-cli").unwrap();
    cmd.current_dir(dir)
        .env("CLICKUP_API_URL", server.uri())
        .env("CLICKUP_TOKEN", "pk_test")
        .env("CLICKUP_WORKSPACE", "99")
        .env("CLICKUP_GIT_DETECT", "0")
        .env_remove("CLICKUP_TASK_ID");
    cmd
}

#[tokio::test]
async fn comment_create_markdown_sends_ops_array() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/task/t1/comment"))
        .and(body_partial_json(serde_json::json!({
            "comment": [
                {"text": "hello "},
                {"text": "bold", "attributes": {"bold": true}},
                {"text": "\n"}
            ],
            "notify_all": false
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "c1"})),
        )
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "comment", "create", "--task", "t1", "--markdown", "--text",
            "hello **bold**",
        ])
        .assert()
        .success();
}

/// The markdown body must NOT contain comment_text (wiremock's
/// body_partial_json can't assert absence, so parse the received request).
#[tokio::test]
async fn comment_create_markdown_omits_comment_text() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/task/t1/comment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "c1"})),
        )
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "comment", "create", "--task", "t1", "--markdown", "--text", "*x*",
        ])
        .assert()
        .success();

    let requests = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert!(body.get("comment_text").is_none(), "body: {body}");
    assert!(body.get("comment").is_some(), "body: {body}");
}

#[tokio::test]
async fn comment_reply_markdown_sends_ops_array() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/comment/c1/reply"))
        .and(body_partial_json(serde_json::json!({
            "comment": [
                {"text": "ls", "attributes": {"code": true}},
                {"text": "\n"}
            ]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "c2"})),
        )
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["comment", "reply", "c1", "--markdown", "--text", "`ls`"])
        .assert()
        .success();
}

/// Without --markdown the body is byte-identical to today: comment_text
/// carries the raw string, no `comment` key.
#[tokio::test]
async fn comment_create_without_flag_unchanged() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/task/t1/comment"))
        .and(body_partial_json(serde_json::json!({
            "comment_text": "hello **bold**"
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "c1"})),
        )
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["comment", "create", "--task", "t1", "--text", "hello **bold**"])
        .assert()
        .success();

    let requests = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert!(body.get("comment").is_none(), "body: {body}");
}

/// Ops-empty fallback: input that reduces to nothing still posts a
/// non-empty comment via comment_text.
#[tokio::test]
async fn comment_create_markdown_empty_ops_falls_back_to_text() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/task/t1/comment"))
        .and(body_partial_json(
            serde_json::json!({"comment_text": "   "}),
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "c1"})),
        )
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args(["comment", "create", "--task", "t1", "--markdown", "--text", "   "])
        .assert()
        .success();
}

// Silence unused-import warnings if predicates ends up unused after edits.
#[allow(unused)]
fn _keep(p: impl Predicate<str>) {}
```

Note: if `predicates::prelude::*` ends up genuinely unused, DELETE the import and the `_keep` helper rather than shipping the workaround — clippy runs with `-D warnings`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test test_comment_markdown`
Expected: the three `--markdown` tests FAIL (clap: unexpected argument '--markdown'); `comment_create_without_flag_unchanged` PASSES (regression guard).

- [ ] **Step 3: Implement**

In `src/commands/comment.rs`:

(a) Add to the `Create` variant (after `notify_all`):

```rust
        /// Parse --text as markdown and submit ClickUp rich formatting
        /// (bold/italic/code/links, lists, code blocks; headings render
        /// bold, blockquotes indent, tables/strikethrough degrade to text)
        #[arg(long)]
        markdown: bool,
```

and the same field/doc on the `Reply` variant (after `assignee`).

(b) Update both doc comments that say "Note: ClickUp's v2 comment API does not render markdown; markdown syntax is stored as literal text." on the `text` args of Create and Reply to end with "…stored as literal text unless --markdown is set."

(c) In the `Create` handler, destructure `markdown` and replace the task-branch body construction:

```rust
            } else if let Some(resolved) = git::resolve_task(cli, task.as_deref(), true)? {
                let mut body = if markdown {
                    let ops = crate::markdown_ops::markdown_to_ops(&text);
                    if ops.is_empty() {
                        serde_json::json!({ "comment_text": text, "notify_all": notify_all })
                    } else {
                        serde_json::json!({ "comment": ops, "notify_all": notify_all })
                    }
                } else {
                    serde_json::json!({ "comment_text": text, "notify_all": notify_all })
                };
                if let Some(a) = assignee {
                    body["assignee"] = serde_json::json!(a);
                }
```

Apply the same `if markdown { ... }` body selection to the list- and view-comment branches of Create (they build `{"comment_text": text}` today — same switch, no `notify_all` there, preserving current shape otherwise).

(d) In the `Reply` handler:

```rust
        CommentCommands::Reply {
            id,
            text,
            assignee,
            markdown,
        } => {
            let mut body = if markdown {
                let ops = crate::markdown_ops::markdown_to_ops(&text);
                if ops.is_empty() {
                    serde_json::json!({ "comment_text": text })
                } else {
                    serde_json::json!({ "comment": ops })
                }
            } else {
                serde_json::json!({ "comment_text": text })
            };
            if let Some(a) = assignee {
                body["assignee"] = serde_json::json!(a);
            }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test test_comment_markdown`
Expected: all 5 PASS.

- [ ] **Step 5: Commit**

```bash
git add src/commands/comment.rs tests/test_comment_markdown.rs
git commit -m "feat: --markdown on comment create/reply submits rich ops"
```

---

### Task 3: MCP `clickup_comment_create` `markdown` argument

**Files:**
- Modify: `src/mcp.rs` (tool schema + handler)
- Test: append to `tests/test_comment_markdown.rs`

**Interfaces:**
- Consumes: `clickup_cli::markdown_ops::markdown_to_ops` (Task 1) — note `src/mcp.rs` uses `crate::markdown_ops::markdown_to_ops` (or add a `use` at the top matching existing import style).

- [ ] **Step 1: Write the failing test**

Append to `tests/test_comment_markdown.rs`:

```rust
#[tokio::test]
async fn mcp_comment_create_markdown_sends_ops() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/task/t1/comment"))
        .and(body_partial_json(serde_json::json!({
            "comment": [
                {"text": "x", "attributes": {"bold": true}},
                {"text": "\n"}
            ]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "c1"})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("clickup-cli").unwrap();
    cmd.current_dir(dir.path())
        .args(["mcp", "serve"])
        .env("CLICKUP_API_URL", server.uri())
        .env("CLICKUP_TOKEN", "pk_test")
        .env("CLICKUP_WORKSPACE", "99");
    cmd.write_stdin(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": "clickup_comment_create", "arguments": {
                "task_id": "t1", "text": "**x**", "markdown": true
            }}
        })
        .to_string()
            + "\n",
    )
    .assert()
    .success()
    .stdout(predicates::str::contains("Comment created"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test test_comment_markdown mcp_comment_create_markdown`
Expected: FAIL — the mounted `.expect(1)` mock never matches (handler sends `comment_text`), so the CLI hits an unmatched-request 404 path or the expect panics on drop.

- [ ] **Step 3: Implement**

In `src/mcp.rs`:

(a) Schema — in the `clickup_comment_create` tool definition's `properties`, after the `text` property, add:

```json
"markdown": {"type": "boolean", "description": "true = parse `text` as markdown and submit ClickUp rich formatting (bold/italic/code/links, lists, code blocks; headings render bold, blockquotes indent, unsupported constructs degrade to plain text). false or omitted = literal text."},
```

(b) Handler — in the `"clickup_comment_create"` arm (currently `let mut body = json!({"comment_text": text});`):

```rust
            let markdown = args
                .get("markdown")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let mut body = if markdown {
                let ops = crate::markdown_ops::markdown_to_ops(text);
                if ops.is_empty() {
                    json!({"comment_text": text})
                } else {
                    json!({"comment": ops})
                }
            } else {
                json!({"comment_text": text})
            };
```

(`text` is already bound as `&str` in that arm; keep the existing assignee/notify_all insertions below it unchanged.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test test_comment_markdown`
Expected: all 6 PASS.

- [ ] **Step 5: Commit**

```bash
git add src/mcp.rs tests/test_comment_markdown.rs
git commit -m "feat(mcp): markdown argument on clickup_comment_create"
```

---

### Task 4: Docs, changelog, full verification

**Files:**
- Modify: `CLAUDE.md` (comment command summary + trailing agent-reference paragraph), `src/commands/agent_config.rs` (AGENT_REFERENCE), `docs/commands.md` (comment section), `CHANGELOG.md`

**Interfaces:**
- Consumes: the finished behavior from Tasks 1–3.

- [ ] **Step 1: Update the three command-reference surfaces**

In `CLAUDE.md` (both the `### Collaboration (v0.2)` comment line context and the trailing `<!-- clickup-cli:begin -->` paragraph), `src/commands/agent_config.rs` (AGENT_REFERENCE), and `docs/commands.md` (comment syntax lines): change

- `create [--task ID]|--list ID|--view ID --text T [--notify-all]` → `create [--task ID]|--list ID|--view ID --text T [--notify-all] [--markdown]`
- `reply ID --text T` → `reply ID --text T [--markdown]`

(docs/commands.md uses its own `clickup-cli comment create ...` syntax lines — apply the same two flag additions there.) Also add one sentence to the trailing agent-reference paragraphs (both copies) after the existing comment-API note: `comment create/reply --markdown converts markdown to ClickUp rich comment formatting (bold/italic/code/links/lists/code blocks; headings render bold, blockquotes indent, tables/strikethrough degrade to plain text).`

- [ ] **Step 2: Changelog**

Under `## [Unreleased]`, inside the existing `### Added` section (create it if a release roll has emptied Unreleased), append:

```markdown
- `--markdown` on `comment create` and `comment reply`, plus a `markdown` argument on the MCP `clickup_comment_create` tool: the text is parsed as CommonMark and submitted via ClickUp's documented rich-comment ops format (https://developer.clickup.com/docs/comment-formatting) — bold, italic, inline code, links, bullet/ordered/checked lists (with nesting via indent), and fenced code blocks render natively. Constructs the format cannot express degrade gracefully: headings render as bold lines, blockquotes as indentation, strikethrough/tables/raw HTML pass through as plain text. Only documented API surface is used; without the flag, requests are byte-identical to before. Implemented clean-room from ClickUp's docs (new dependency: pulldown-cmark).
```

- [ ] **Step 3: Full verification**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: all suites pass (29 test binaries incl. the two new files), clippy exit 0 (verify the exit code explicitly, not via a piped tail), fmt clean.

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md src/commands/agent_config.rs docs/commands.md CHANGELOG.md
git commit -m "docs: markdown comment support across command references + changelog"
```
