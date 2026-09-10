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

/// ClickUp's `indent` attribute (used for both nested lists and the
/// blockquote degradation) tops out at 8 levels; deeper nesting clamps
/// rather than emitting an ever-growing number the UI won't render further.
const MAX_INDENT: usize = 8;

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
    // True while inside a `[text](user:<id>)` mention link: the tag op has
    // already been emitted and the display text is dropped (ClickUp renders
    // the member's real name).
    let mut in_mention = false;
    // The link (if any) that was in scope when an image started — images
    // clobber `link` with an internal `__IMG__` marker while reconstructing
    // themselves as literal text, so the enclosing link (e.g.
    // `[![alt](img.png)](https://dest)`) must be parked here and restored
    // on `TagEnd::Image`, and used in place of the marker while the image
    // is being reconstructed.
    let mut saved_link: Option<Option<String>> = None;
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
                attrs.insert(
                    "indent".into(),
                    json!((list_stack.len() - 1).min(MAX_INDENT)),
                );
            }
        } else if blockquote_depth > 0 {
            attrs.insert(
                "indent".into(),
                json!(blockquote_depth.min(MAX_INDENT as u32)),
            );
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
                Tag::Link { dest_url, .. } => {
                    if let Some(id) = dest_url
                        .strip_prefix("user:")
                        .and_then(|rest| rest.parse::<u64>().ok())
                        .filter(|id| *id > 0)
                    {
                        // Documented mention op; notifies the user. Inline
                        // styling around it is ignored (tag ops carry no
                        // attributes).
                        ops.push(json!({"type": "tag", "user": {"id": id}}));
                        terminator_pending = true;
                        in_mention = true;
                    } else {
                        link = Some(dest_url.to_string());
                    }
                }
                Tag::Image { dest_url, .. } if !in_mention => {
                    // Save whatever link is currently in scope (e.g. the
                    // outer `https://dest` of `[![alt](img.png)](https://dest)`)
                    // so it survives the `__IMG__` marker below and can be
                    // restored — and reused on the reconstructed text — on
                    // TagEnd::Image.
                    saved_link = Some(link.clone());
                    // Literal passthrough: reconstruct as markdown text,
                    // carrying the outer link (if any) along with it.
                    push_text(&mut ops, "![", bold, italic, &link, false, heading_depth);
                    // alt text arrives as Text events; the closing is
                    // emitted on TagEnd::Image below.
                    link = Some(format!("__IMG__{}", dest_url));
                }
                Tag::Heading { .. } => heading_depth += 1,
                Tag::BlockQuote(_) => blockquote_depth += 1,
                Tag::CodeBlock(kind) => {
                    // A loose list item's paragraph is followed by a
                    // sibling code block (not another Paragraph), so
                    // TagEnd::Paragraph never fires the flush (it's
                    // suppressed inside lists, deferred to TagEnd::Item) —
                    // without this, the pending text run merges straight
                    // into the code block's first line. As with the
                    // Tag::Paragraph flush above, this is a plain "\n": the
                    // code block terminates its own lines with its own
                    // code-block attribute, so this separator must not
                    // carry the list's bullet.
                    if !list_stack.is_empty() && terminator_pending {
                        ops.push(json!({"text": "\n"}));
                        terminator_pending = false;
                    }
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
                Tag::Paragraph => {
                    // A loose list item's second (and later) paragraph
                    // needs a separator from the first — a plain "\n" with
                    // no attributes, since a list-attributed terminator
                    // here would read as starting a new bullet.
                    //
                    // Known trade-off: the ClickUp ops format has exactly
                    // one `list` attribute slot, carried on the "\n" that
                    // terminates a line — it can't be attached to an
                    // earlier line and "held over" for a later one. So in
                    // a multi-block list item, the *last* line is the one
                    // that ends up carrying the list attribute, meaning the
                    // bullet visually renders next to the final block while
                    // the earlier paragraph(s) show up as plain lines above
                    // it with no bullet at all. This is the best available
                    // approximation given the format, not a bug to fix.
                    if !list_stack.is_empty() && terminator_pending {
                        ops.push(json!({"text": "\n"}));
                        terminator_pending = false;
                    }
                }
                Tag::HtmlBlock => {}
                _ => {}
            },
            Event::End(tag_end) => match tag_end {
                TagEnd::Strong => bold = bold.saturating_sub(1),
                TagEnd::Emphasis => italic = italic.saturating_sub(1),
                TagEnd::Strikethrough => {}
                TagEnd::Link => {
                    if in_mention {
                        in_mention = false;
                    } else {
                        link = None;
                    }
                }
                TagEnd::Image if !in_mention => {
                    // close the literal image reconstruction
                    let url = link
                        .take()
                        .and_then(|l| l.strip_prefix("__IMG__").map(str::to_string))
                        .unwrap_or_default();
                    // Restore whatever link was in scope before the image
                    // (None if the image wasn't itself inside a link), and
                    // carry it onto the closing text run too.
                    let outer = saved_link.take().flatten();
                    push_text(
                        &mut ops,
                        &format!("]({})", url),
                        bold,
                        italic,
                        &outer,
                        false,
                        heading_depth,
                    );
                    link = outer;
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
                // Inside a list item the Item end emits the terminator;
                // a bare paragraph terminates its own line.
                TagEnd::Paragraph if list_stack.is_empty() => {
                    line_end(&mut ops, &list_stack, None, blockquote_depth);
                    terminator_pending = false;
                }
                _ => {}
            },
            Event::Text(t) => {
                if in_mention {
                    // Mention display text is informational only; dropped.
                } else if in_code_block {
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
                    // marker; substitute the outer link (if any) so text
                    // wrapped like `[![alt](img.png)](https://dest)` keeps
                    // it, instead of dropping it as part of the literal form.
                    let effective_link = match &link {
                        Some(l) if l.starts_with("__IMG__") => saved_link.clone().flatten(),
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
                if !in_mention {
                    push_text(&mut ops, &t, bold, italic, &link, true, heading_depth);
                    terminator_pending = true;
                }
            }
            Event::Html(t) | Event::InlineHtml(t) if !in_mention => {
                push_text(&mut ops, &t, bold, italic, &None, false, heading_depth);
                terminator_pending = true;
            }
            Event::SoftBreak if !in_mention => {
                push_text(&mut ops, " ", bold, italic, &None, false, heading_depth);
                terminator_pending = true;
            }
            // Deliberately leaves `terminator_pending` untouched: a hard
            // break is a mid-line/mid-item visual break, not a block
            // terminator, so it must not suppress (or fake) the item's
            // own pending line-end bookkeeping.
            Event::HardBreak if !in_mention => ops.push(json!({"text": "\n"})),
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

/// A workspace member who can be @mentioned in a comment.
#[derive(Clone, Debug)]
pub struct MentionUser {
    pub id: i64,
    pub username: String,
    pub email: String,
}

/// Pull mentionable users out of `GET /v2/team`. Prefer the named workspace
/// when several teams are returned.
pub fn users_from_teams(resp: &Value, workspace_id: Option<&str>) -> Vec<MentionUser> {
    let teams = resp
        .get("teams")
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default();
    let selected: Vec<&Value> = if let Some(id) = workspace_id {
        let hit: Vec<&Value> = teams
            .iter()
            .filter(|t| t.get("id").and_then(|v| v.as_str()) == Some(id))
            .collect();
        if hit.is_empty() {
            teams.iter().collect()
        } else {
            hit
        }
    } else {
        teams.iter().collect()
    };
    let mut out = Vec::new();
    for team in selected {
        let members = match team.get("members").and_then(|m| m.as_array()) {
            Some(m) => m,
            None => continue,
        };
        for member in members {
            let user = member.get("user").unwrap_or(member);
            let id = user
                .get("id")
                .and_then(|v| v.as_i64())
                .or_else(|| user.get("id").and_then(|v| v.as_u64()).map(|u| u as i64));
            let username = user
                .get("username")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let email = user
                .get("email")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if let Some(id) = id {
                if !username.is_empty() {
                    out.push(MentionUser {
                        id,
                        username,
                        email,
                    });
                }
            }
        }
    }
    out
}

fn tag_op(id: i64, display: &str) -> Value {
    json!({
        "type": "tag",
        "text": display,
        "user": { "id": id }
    })
}

fn is_boundary(c: char) -> bool {
    !c.is_alphanumeric()
}

/// ClickUp does not turn `@Name` in `comment_text` into a mention. Mentions
/// are `type: "tag"` ops with a numeric user id
/// (https://developer.clickup.com/docs/comment-formatting).
///
/// Recognised forms, longest username first:
/// - `@Display Name` (workspace `username`, case-insensitive)
/// - `@user@email` (full email)
/// - `@123456` when `123456` is a known user id
///
/// `<@123456>` is handled in `split_text_mentions` so the brackets are
/// consumed with the token.
fn match_mention(rest: &str, users: &[MentionUser]) -> Option<(i64, String, usize)> {
    let digit_len = rest.bytes().take_while(|b| b.is_ascii_digit()).count();
    if digit_len > 0 {
        if let Ok(id) = rest[..digit_len].parse::<i64>() {
            if users.iter().any(|u| u.id == id) {
                let next_ok = rest[digit_len..]
                    .chars()
                    .next()
                    .map(is_boundary)
                    .unwrap_or(true);
                if next_ok {
                    let display = users
                        .iter()
                        .find(|u| u.id == id)
                        .map(|u| format!("@{}", u.username))
                        .unwrap_or_else(|| format!("@{}", id));
                    return Some((id, display, digit_len));
                }
            }
        }
    }

    let mut ranked: Vec<&MentionUser> = users.iter().collect();
    ranked.sort_by_key(|u| std::cmp::Reverse(u.username.len()));
    for u in ranked {
        if u.username.len() < 2 {
            continue;
        }
        let n = u.username.len();
        if rest.len() >= n && rest[..n].eq_ignore_ascii_case(&u.username) {
            let next_ok = rest[n..].chars().next().map(is_boundary).unwrap_or(true);
            if next_ok {
                return Some((u.id, format!("@{}", u.username), n));
            }
        }
        if !u.email.is_empty() {
            let n = u.email.len();
            if rest.len() >= n && rest[..n].eq_ignore_ascii_case(&u.email) {
                let next_ok = rest[n..].chars().next().map(is_boundary).unwrap_or(true);
                if next_ok {
                    return Some((u.id, format!("@{}", u.username), n));
                }
            }
        }
    }
    None
}

fn split_text_mentions(text: &str, users: &[MentionUser]) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    let mut last = 0usize;
    let mut i = 0usize;
    while i < text.len() {
        let ch = text[i..].chars().next().unwrap();
        // `<@123>` is one token — do not match the inner `@123` and leave
        // the angle brackets as leftover text.
        if text[i..].starts_with("<@") {
            let prev_ok = i == 0
                || text[..i]
                    .chars()
                    .next_back()
                    .map(is_boundary)
                    .unwrap_or(true);
            if prev_ok {
                if let Some(end) = text[i + 2..].find('>') {
                    let id_str = &text[i + 2..i + 2 + end];
                    if let Ok(id) = id_str.parse::<i64>() {
                        if last < i {
                            out.push(json!({ "text": &text[last..i] }));
                        }
                        let display = users
                            .iter()
                            .find(|u| u.id == id)
                            .map(|u| format!("@{}", u.username))
                            .unwrap_or_else(|| format!("@{}", id_str));
                        out.push(tag_op(id, &display));
                        i = i + 2 + end + 1;
                        last = i;
                        continue;
                    }
                }
            }
        }
        if ch == '@' {
            let prev_ok = i == 0
                || text[..i]
                    .chars()
                    .next_back()
                    .map(is_boundary)
                    .unwrap_or(true);
            if prev_ok {
                if let Some((id, display, consumed)) = match_mention(&text[i + 1..], users) {
                    if last < i {
                        out.push(json!({ "text": &text[last..i] }));
                    }
                    out.push(tag_op(id, &display));
                    i = i + 1 + consumed;
                    last = i;
                    continue;
                }
            }
        }
        i += ch.len_utf8();
    }
    if last < text.len() {
        out.push(json!({ "text": &text[last..] }));
    }
    if out.is_empty() {
        out.push(json!({ "text": text }));
    }
    out
}

fn op_is_code_context(ops: &[Value], i: usize) -> bool {
    let op = &ops[i];
    if op.get("type").and_then(|t| t.as_str()) == Some("tag")
        || op.get("type").and_then(|t| t.as_str()) == Some("emoticon")
    {
        return true;
    }
    if op
        .get("attributes")
        .and_then(|a| a.get("code"))
        .and_then(|v| v.as_bool())
        == Some(true)
    {
        return true;
    }
    if let Some(next) = ops.get(i + 1) {
        if next
            .get("attributes")
            .and_then(|a| a.get("code-block"))
            .is_some()
        {
            return true;
        }
    }
    false
}

/// Rewrite text ops that contain `@mentions` into ClickUp `type: "tag"` ops.
/// Leaves unmatched `@foo` as plain text. Skips inline code and fenced-code
/// lines.
pub fn apply_mentions(ops: Vec<Value>, users: &[MentionUser]) -> Vec<Value> {
    if users.is_empty() {
        return ops;
    }
    let mut out = Vec::new();
    for i in 0..ops.len() {
        // Already a mention (e.g. from a `[@Name](user:id)` markdown link):
        // its text is display-only, never re-resolved.
        if ops[i].get("type").and_then(|t| t.as_str()) == Some("tag") {
            out.push(ops[i].clone());
            continue;
        }
        if op_is_code_context(&ops, i) {
            out.push(ops[i].clone());
            continue;
        }
        let Some(text) = ops[i].get("text").and_then(|t| t.as_str()) else {
            out.push(ops[i].clone());
            continue;
        };
        if !text.contains('@') {
            out.push(ops[i].clone());
            continue;
        }
        let attrs = ops[i].get("attributes").cloned();
        let pieces = split_text_mentions(text, users);
        for piece in pieces {
            if piece.get("type").and_then(|t| t.as_str()) == Some("tag") {
                out.push(piece);
            } else if let Some(ref a) = attrs {
                let mut obj = piece;
                obj["attributes"] = a.clone();
                out.push(obj);
            } else {
                out.push(piece);
            }
        }
    }
    out
}

/// Attach tag ops to a comment POST body. If the body is still
/// `comment_text` and a mention resolves, it is converted to a `comment`
/// ops array (required — ClickUp ignores `@Name` in `comment_text`).
pub fn apply_mentions_to_body(mut body: Value, users: &[MentionUser]) -> Value {
    if users.is_empty() {
        return body;
    }
    if let Some(ops) = body.get("comment").and_then(|c| c.as_array()).cloned() {
        body["comment"] = Value::Array(apply_mentions(ops, users));
        return body;
    }
    if let Some(text) = body.get("comment_text").and_then(|t| t.as_str()) {
        if text.contains('@') {
            let ops = apply_mentions(vec![json!({ "text": text })], users);
            if ops
                .iter()
                .any(|o| o.get("type").and_then(|t| t.as_str()) == Some("tag"))
            {
                if let Some(obj) = body.as_object_mut() {
                    obj.remove("comment_text");
                    obj.insert("comment".into(), Value::Array(ops));
                }
            }
        }
    }
    body
}

/// Build the comment POST body: rich ops when markdown is set and the
/// input is expressible, plain comment_text otherwise.
pub fn comment_body(markdown: bool, text: &str) -> Value {
    if markdown {
        let ops = markdown_to_ops(text);
        if !ops.is_empty() {
            return json!({ "comment": ops });
        }
    }
    json!({ "comment_text": text })
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
    fn multi_paragraph_list_item_gets_line_break_separator() {
        assert_eq!(
            markdown_to_ops("- a\n\n  b"),
            vec![
                json!({"text": "a"}),
                json!({"text": "\n"}),
                json!({"text": "b"}),
                json!({"text": "\n", "attributes": {"list": {"list": "bullet"}}}),
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

    #[test]
    fn loose_item_paragraph_then_code_block_separated() {
        assert_eq!(
            markdown_to_ops("- para\n\n  ```\n  code\n  ```"),
            vec![
                json!({"text": "para"}),
                json!({"text": "\n"}),
                json!({"text": "code"}),
                json!({"text": "\n", "attributes": {"code-block": {"code-block": "plain"}}}),
            ]
        );
    }

    #[test]
    fn image_inside_link_keeps_link_url() {
        assert_eq!(
            markdown_to_ops("[![alt](img.png)](https://dest)"),
            vec![
                json!({"text": "![", "attributes": {"link": "https://dest"}}),
                json!({"text": "alt", "attributes": {"link": "https://dest"}}),
                json!({"text": "](img.png)", "attributes": {"link": "https://dest"}}),
                json!({"text": "\n"}),
            ]
        );
    }

    #[test]
    fn deep_blockquote_indent_clamped_to_eight() {
        let bq = "> ".repeat(10) + "deep";
        assert_eq!(
            markdown_to_ops(&bq),
            vec![
                json!({"text": "deep"}),
                json!({"text": "\n", "attributes": {"indent": 8}}),
            ]
        );
    }

    fn ada() -> MentionUser {
        MentionUser {
            id: 111111,
            username: "Ada Lovelace".into(),
            email: "ada@example.com".into(),
        }
    }

    fn alan() -> MentionUser {
        MentionUser {
            id: 222222,
            username: "Alan Turing".into(),
            email: "alan@example.com".into(),
        }
    }

    #[test]
    fn mention_display_name_becomes_tag_op() {
        let ops = markdown_to_ops("hey @Ada Lovelace");
        let tagged = apply_mentions(ops, &[ada()]);
        assert_eq!(
            tagged,
            vec![
                json!({"text": "hey "}),
                json!({"type": "tag", "text": "@Ada Lovelace", "user": {"id": 111111}}),
                json!({"text": "\n"}),
            ]
        );
    }

    #[test]
    fn two_mentions_in_one_run() {
        let tagged = apply_mentions(
            vec![json!({"text": "@Ada Lovelace @Alan Turing"})],
            &[ada(), alan()],
        );
        assert_eq!(
            tagged,
            vec![
                json!({"type": "tag", "text": "@Ada Lovelace", "user": {"id": 111111}}),
                json!({"text": " "}),
                json!({"type": "tag", "text": "@Alan Turing", "user": {"id": 222222}}),
            ]
        );
    }

    #[test]
    fn angle_and_bare_user_id_mentions() {
        let users = [ada()];
        let tagged = apply_mentions(vec![json!({"text": "<@111111> and @111111"})], &users);
        assert_eq!(
            tagged,
            vec![
                json!({"type": "tag", "text": "@Ada Lovelace", "user": {"id": 111111}}),
                json!({"text": " and "}),
                json!({"type": "tag", "text": "@Ada Lovelace", "user": {"id": 111111}}),
            ]
        );
    }

    #[test]
    fn unmatched_at_stays_text() {
        let tagged = apply_mentions(vec![json!({"text": "see @nobody here"})], &[ada()]);
        assert_eq!(tagged, vec![json!({"text": "see @nobody here"})]);
    }

    #[test]
    fn inline_code_is_not_mentioned() {
        let ops = markdown_to_ops("use `@Ada Lovelace`");
        let tagged = apply_mentions(ops, &[ada()]);
        assert!(
            tagged
                .iter()
                .all(|o| o.get("type").and_then(|t| t.as_str()) != Some("tag")),
            "got {tagged:?}"
        );
    }

    #[test]
    fn email_local_at_is_not_a_mention() {
        let tagged = apply_mentions(vec![json!({"text": "write ada@example.com"})], &[ada()]);
        assert_eq!(tagged, vec![json!({"text": "write ada@example.com"})]);
    }

    #[test]
    fn comment_text_body_promotes_to_ops_when_tagged() {
        let body = json!({"comment_text": "ping @Ada Lovelace"});
        let out = apply_mentions_to_body(body, &[ada()]);
        assert!(out.get("comment_text").is_none());
        let ops = out.get("comment").and_then(|c| c.as_array()).unwrap();
        let tag = ops
            .iter()
            .find(|o| o.get("type").and_then(|t| t.as_str()) == Some("tag"))
            .expect("expected a tag op");
        assert_eq!(tag["user"]["id"], 111111);
    }
}

#[cfg(test)]
mod mention_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn user_scheme_link_emits_tag_op() {
        assert_eq!(
            markdown_to_ops("hi [@Nick](user:81618)!"),
            vec![
                json!({"text": "hi "}),
                json!({"type": "tag", "user": {"id": 81618}}),
                json!({"text": "!"}),
                json!({"text": "\n"}),
            ]
        );
    }

    #[test]
    fn non_numeric_user_scheme_degrades_to_normal_link() {
        assert_eq!(
            markdown_to_ops("[x](user:abc)"),
            vec![
                json!({"text": "x", "attributes": {"link": "user:abc"}}),
                json!({"text": "\n"}),
            ]
        );
    }

    #[test]
    fn styled_mention_drops_styling() {
        // Tag ops carry no attributes; bold wrapping is ignored.
        assert_eq!(
            markdown_to_ops("**[@Nick](user:7)**"),
            vec![
                json!({"type": "tag", "user": {"id": 7}}),
                json!({"text": "\n"}),
            ]
        );
    }

    #[test]
    fn mention_in_list_item() {
        assert_eq!(
            markdown_to_ops("- ping [@N](user:5) today"),
            vec![
                json!({"text": "ping "}),
                json!({"type": "tag", "user": {"id": 5}}),
                json!({"text": " today"}),
                json!({"text": "\n", "attributes": {"list": {"list": "bullet"}}}),
            ]
        );
    }

    #[test]
    fn normal_links_unaffected_by_mention_support() {
        assert_eq!(
            markdown_to_ops("[site](https://x.io)"),
            vec![
                json!({"text": "site", "attributes": {"link": "https://x.io"}}),
                json!({"text": "\n"}),
            ]
        );
    }
}

#[cfg(test)]
mod mention_hardening_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn user_zero_degrades_to_normal_link() {
        assert_eq!(
            markdown_to_ops("[x](user:0)"),
            vec![
                json!({"text": "x", "attributes": {"link": "user:0"}}),
                json!({"text": "\n"}),
            ]
        );
    }

    #[test]
    fn breaks_and_html_inside_mention_text_are_dropped() {
        // Soft break, hard break, and inline HTML in the display text must
        // not leak literal ops — the tag op is the whole mention.
        assert_eq!(
            markdown_to_ops("[@a\nb](user:5)"),
            vec![
                json!({"type": "tag", "user": {"id": 5}}),
                json!({"text": "\n"}),
            ]
        );
        assert_eq!(
            markdown_to_ops("[@<b>N</b>](user:5)"),
            vec![
                json!({"type": "tag", "user": {"id": 5}}),
                json!({"text": "\n"}),
            ]
        );
    }

    #[test]
    fn image_inside_mention_text_is_dropped() {
        assert_eq!(
            markdown_to_ops("[![a](x.png)](user:5)"),
            vec![
                json!({"type": "tag", "user": {"id": 5}}),
                json!({"text": "\n"}),
            ]
        );
    }
}
