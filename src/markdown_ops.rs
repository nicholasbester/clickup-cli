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

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
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
    // Tracks whether the current line has content awaiting a terminator.
    // Needed because a nested list's items close (`TagEnd::Item`) only
    // after the *entire* nested list closes, which is after the parent
    // item's own text — so a nested `Tag::List` start must flush the
    // parent's line eagerly, and `TagEnd::Item` must not double-flush it.
    let mut terminator_pending = false;

    fn push_text(
        ops: &mut Vec<Value>,
        s: &str,
        bold: u32,
        italic: u32,
        link: &Option<String>,
        code: bool,
        heading: u32,
    ) {
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
    }

    // Terminate the current line with the block attributes in scope.
    fn line_end(
        ops: &mut Vec<Value>,
        list_stack: &[ListKind],
        item_task_state: Option<bool>,
        blockquote_depth: u32,
    ) {
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
    }

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
                    // A list nested inside a running item never sees that
                    // item's `TagEnd::Item` before it closes itself, so
                    // flush the parent item's line here instead.
                    if !list_stack.is_empty() && terminator_pending {
                        line_end(&mut ops, &list_stack, item_task_state, blockquote_depth);
                        terminator_pending = false;
                        item_task_state = None;
                    }
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
                    terminator_pending = false;
                }
                TagEnd::BlockQuote(_) => blockquote_depth = blockquote_depth.saturating_sub(1),
                TagEnd::CodeBlock => in_code_block = false,
                TagEnd::List(_) => {
                    list_stack.pop();
                }
                TagEnd::Item => {
                    if terminator_pending {
                        line_end(&mut ops, &list_stack, item_task_state, blockquote_depth);
                        terminator_pending = false;
                    }
                    item_task_state = None;
                }
                TagEnd::Paragraph => {
                    // Inside a list item the Item end emits the terminator;
                    // a bare paragraph terminates its own line.
                    if list_stack.is_empty() {
                        line_end(&mut ops, &list_stack, None, blockquote_depth);
                        terminator_pending = false;
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
                    terminator_pending = true;
                }
            }
            Event::Code(t) => {
                push_text(&mut ops, &t, bold, italic, &link, true, heading_depth);
                terminator_pending = true;
            }
            Event::Html(t) | Event::InlineHtml(t) => {
                push_text(&mut ops, &t, bold, italic, &None, false, heading_depth);
                terminator_pending = true;
            }
            Event::SoftBreak => {
                push_text(&mut ops, " ", bold, italic, &None, false, heading_depth);
                terminator_pending = true;
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
