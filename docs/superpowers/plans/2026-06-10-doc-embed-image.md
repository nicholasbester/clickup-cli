# `doc embed-image` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `doc embed-image` CLI command (and matching `clickup_doc_embed_image` MCP tool) that uploads a local image as a task attachment and embeds the returned CDN URL inline into a ClickUp doc page as a markdown image.

**Architecture:** Composite operation over two existing API calls: `POST /v2/task/{id}/attachment` (multipart upload via the existing `client.upload_file()`), then `PUT /v3/workspaces/{ws}/docs/{doc}/pages/{page}` with `content = "\n![alt](url)\n"` and `content_edit_mode = append|prepend`. ClickUp converts the markdown into a native inline image block (verified live 2026-06-10; see spec `docs/superpowers/specs/2026-06-10-doc-embed-image-design.md`). The host task resolves like every other task-scoped command: `--via-task` flag → `CLICKUP_TASK_ID` env → git branch.

**Tech Stack:** Rust, clap 4 (derive), serde_json, tokio/reqwest (existing `ClickUpClient`), assert_cmd/predicates for CLI tests.

**Background you need (read before Task 1):**
- `src/commands/doc.rs` — all doc subcommands live in one `DocCommands` enum + one `execute()` that already resolves `token`, `client`, `ws_id`, `output`, and `base = /v3/workspaces/{ws}/docs`.
- `src/git.rs:252` — `git::require_task(cli, explicit: Option<&str>, allow_branch: bool) -> Result<ResolvedTask, CliError>`. `ResolvedTask { id, raw, is_custom, source }`. When `is_custom` is true the v2 URL needs `?custom_task_ids=true&team_id={ws}` (see `src/commands/task.rs:505` for the pattern).
- `src/client.rs:214` — `upload_file(path, &Path)` posts multipart field `attachment`, returns `serde_json::Value`.
- `src/output.rs` — `output.print_single(&value, fields, quiet_key)`; quiet mode (`-q`) prints only `quiet_key`.
- MCP: tool schemas are JSON literals in a big array in `src/mcp.rs` (~line 1149 for `clickup_doc_edit_page`); handlers are match arms in the same file (~line 3675 for `clickup_doc_edit_page`, ~line 5113 for `clickup_attachment_upload` which shows `resolve_task(args, "task_id")` returning `(task_id, custom_q)`). Tool classification for `--profile`/`--groups` filtering is in `src/mcp/classify.rs`; `embed_image` doesn't match any verb list so it MUST get an `OVERRIDES` entry or `classify()` returns `None`.
- Tool count is asserted as `143` in `tests/test_mcp_filter.rs:51` and `:70` — both become `144`.

---

### Task 1: `embed_snippet` helper in doc.rs

**Files:**
- Modify: `src/commands/doc.rs`

- [ ] **Step 1: Write the failing tests**

Append at the bottom of `src/commands/doc.rs`:

```rust
/// Markdown snippet ClickUp converts into a native inline image block.
/// Surrounding newlines keep the image out of adjacent paragraphs.
pub(crate) fn embed_snippet(alt: &str, url: &str) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippet_wraps_image_in_newlines() {
        assert_eq!(
            embed_snippet("chart", "https://example.com/i.png"),
            "\n![chart](https://example.com/i.png)\n"
        );
    }

    #[test]
    fn snippet_allows_empty_alt() {
        assert_eq!(embed_snippet("", "https://x.test/a.png"), "\n![](https://x.test/a.png)\n");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib commands::doc::tests -- --nocapture`
Expected: FAIL (panic at `todo!()`) — both tests.

- [ ] **Step 3: Implement**

Replace the `todo!()` body:

```rust
pub(crate) fn embed_snippet(alt: &str, url: &str) -> String {
    format!("\n![{}]({})\n", alt, url)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib commands::doc::tests`
Expected: `test result: ok. 2 passed`

- [ ] **Step 5: Commit**

```bash
git add src/commands/doc.rs
git commit -m "feat(doc): add embed_snippet helper for inline image markdown"
```

---

### Task 2: CLI `doc embed-image` command

**Files:**
- Modify: `src/commands/doc.rs`
- Test: `tests/test_cli.rs`

- [ ] **Step 1: Write the failing CLI tests**

In `tests/test_cli.rs`, add (near the other doc tests; grep `doc` to find them — if a `test_doc_help` exists asserting the subcommand list, also add `.stdout(predicate::str::contains("embed-image"))` to it):

```rust
#[test]
fn test_doc_embed_image_help() {
    clickup()
        .args(["doc", "embed-image", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--via-task"))
        .stdout(predicate::str::contains("--alt"))
        .stdout(predicate::str::contains("--mode"))
        .stdout(predicate::str::contains("append"));
}

#[test]
fn test_doc_embed_image_rejects_replace_mode() {
    // clap rejects values outside append|prepend before any network/config access
    clickup()
        .args(["doc", "embed-image", "d1", "p1", "img.png", "--mode", "replace"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test test_cli test_doc_embed_image -- --nocapture`
Expected: FAIL — `error: unrecognized subcommand 'embed-image'`.

- [ ] **Step 3: Add the clap variant**

In `src/commands/doc.rs`, add to the `DocCommands` enum after `EditPage`:

```rust
    /// Upload an image and embed it inline in a doc page.
    ///
    /// The ClickUp API has no doc-level upload, so the image is stored as an
    /// attachment on a host task, then referenced from the page as markdown.
    #[command(name = "embed-image")]
    EmbedImage {
        /// Doc ID
        doc_id: String,
        /// Page ID
        page_id: String,
        /// Path to the image file to upload
        file: std::path::PathBuf,
        /// Host task that stores the image binary (auto-detected from git branch if omitted)
        #[arg(long)]
        via_task: Option<String>,
        /// Alt text for the image (defaults to the file name)
        #[arg(long)]
        alt: Option<String>,
        /// Where to insert the image relative to existing content
        #[arg(long, default_value = "append", value_parser = ["append", "prepend"])]
        mode: String,
    },
```

And add the import at the top of the file (doc.rs does not currently use git):

```rust
use crate::git;
```

- [ ] **Step 4: Add the handler match arm**

In `execute()`, after the `DocCommands::EditPage` arm:

```rust
        DocCommands::EmbedImage {
            doc_id,
            page_id,
            file,
            via_task,
            alt,
            mode,
        } => {
            let task = git::require_task(cli, via_task.as_deref(), true)?;
            let upload_path = if task.is_custom {
                format!(
                    "/v2/task/{}/attachment?custom_task_ids=true&team_id={}",
                    task.id, ws_id
                )
            } else {
                format!("/v2/task/{}/attachment", task.id)
            };
            let uploaded = client.upload_file(&upload_path, &file).await?;
            let url = uploaded
                .get("url")
                .and_then(|u| u.as_str())
                .ok_or_else(|| CliError::ServerError {
                    message: "Upload succeeded but the response contained no attachment URL"
                        .into(),
                })?
                .to_string();
            let alt = alt.unwrap_or_else(|| {
                file.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            });
            let body = serde_json::json!({
                "content": embed_snippet(&alt, &url),
                "content_edit_mode": mode,
            });
            let edit = client
                .put(&format!("{}/{}/pages/{}", base, doc_id, page_id), &body)
                .await;
            if let Err(e) = edit {
                // The binary is already on the CDN; tell the caller how to
                // finish the embed without re-uploading.
                eprintln!(
                    "Image uploaded to {} but embedding it in page {} failed.\n\
                     Retry without re-uploading: clickup-cli doc edit-page {} {} \
                     --content \"![{}]({})\" --mode {}",
                    url, page_id, doc_id, page_id, alt, url, mode
                );
                return Err(e);
            }
            let result = serde_json::json!({
                "url": url,
                "page_id": page_id,
                "mode": mode,
            });
            output.print_single(&result, &["url", "page_id", "mode"], "url");
            Ok(())
        }
```

Note: `CliError::ServerError` has only a `message` field (`src/error.rs:22`). `ws_id`, `base`, `client`, `output` are already in scope from the top of `execute()`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test test_cli test_doc_embed_image`
Expected: PASS (2 tests).

- [ ] **Step 6: Run the full suite + clippy**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: all green. (If `test_output.rs` has a table asserting the full command tree, update it per its failure message. No new output-shape test is needed: the command prints through the shared `print_single` path, which `tests/test_output.rs` already covers — this consciously narrows the spec's testing section.)

- [ ] **Step 7: Commit**

```bash
git add src/commands/doc.rs tests/test_cli.rs
git commit -m "feat(doc): add doc embed-image command (upload via host task + markdown embed)"
```

---

### Task 3: MCP tool `clickup_doc_embed_image`

**Files:**
- Modify: `src/mcp.rs` (schema array entry + handler match arm)
- Modify: `src/mcp/classify.rs` (OVERRIDES entry)
- Test: `tests/test_mcp_filter.rs` (tool count 143 → 144)

- [ ] **Step 1: Update the count assertions first (failing tests)**

In `tests/test_mcp_filter.rs` change both `143` literals (lines 51 and 70) to `144`.

Run: `cargo test --test test_mcp_filter`
Expected: FAIL — count is still 143.

- [ ] **Step 2: Add the tool schema**

In `src/mcp.rs`, insert into the tools array immediately after the `clickup_doc_edit_page` entry (its closing `},` is around line 1163):

```rust
        {
            "name": "clickup_doc_embed_image",
            "description": "Upload a local image file and embed it inline in a ClickUp doc page. The ClickUp API has no doc-level upload, so the image is first stored as an attachment on a host task (task_id), then the returned CDN URL is appended or prepended to the page as a markdown image, which ClickUp converts into a native inline image block. Returns the attachment url and page id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "team_id": {"type": "string", "description": "Workspace (team) ID. Omit to use the default workspace from config."},
                    "doc_id": {"type": "string", "description": "ID of the parent doc. Obtain from clickup_doc_list (field: id)."},
                    "page_id": {"type": "string", "description": "ID of the page to embed the image into. Obtain from clickup_doc_pages (field: id)."},
                    "file_path": {"type": "string", "description": "Absolute path to a readable image file on the server running this MCP."},
                    "task_id": {"type": "string", "description": "Host task that stores the image binary (the API only accepts uploads to tasks). The attachment also appears on this task."},
                    "alt": {"type": "string", "description": "Alt text for the image. Defaults to the file name."},
                    "mode": {"type": "string", "description": "Where to insert the image relative to existing page content: 'append' (default) or 'prepend'."}
                },
                "required": ["doc_id", "page_id", "file_path", "task_id"]
            }
        },
```

- [ ] **Step 3: Add the handler match arm**

In `src/mcp.rs`, insert after the `"clickup_doc_edit_page"` arm (closing brace around line 3714):

```rust
        "clickup_doc_embed_image" => {
            let team_id = resolve_workspace(args)?;
            let doc_id = args
                .get("doc_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: doc_id")?;
            let page_id = args
                .get("page_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: page_id")?;
            let file_path = args
                .get("file_path")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: file_path")?;
            let (task_id, custom_q) = resolve_task(args, "task_id")?;
            let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("append");
            if !["append", "prepend"].contains(&mode) {
                return Err(format!(
                    "Invalid mode '{}'. Valid values: append, prepend",
                    mode
                ));
            }
            let upload_path = match custom_q {
                Some(q) => format!("/v2/task/{}/attachment?{}", task_id, q),
                None => format!("/v2/task/{}/attachment", task_id),
            };
            let uploaded = client
                .upload_file(&upload_path, std::path::Path::new(file_path))
                .await
                .map_err(|e| e.to_string())?;
            let url = uploaded
                .get("url")
                .and_then(|u| u.as_str())
                .ok_or("Upload succeeded but the response contained no attachment URL")?
                .to_string();
            let alt = args
                .get("alt")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    std::path::Path::new(file_path)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                });
            let body = json!({
                "content": crate::commands::doc::embed_snippet(&alt, &url),
                "content_edit_mode": mode,
            });
            client
                .put(
                    &format!(
                        "/v3/workspaces/{}/docs/{}/pages/{}",
                        team_id, doc_id, page_id
                    ),
                    &body,
                )
                .await
                .map_err(|e| {
                    format!(
                        "Image uploaded to {} but embedding it in page {} failed: {}. \
                         Retry via clickup_doc_edit_page with content '![{}]({})' and mode '{}' \
                         instead of re-uploading.",
                        url, page_id, e, alt, url, mode
                    )
                })?;
            Ok(json!({"message": "Image embedded", "url": url, "page_id": page_id, "mode": mode}))
        }
```

`resolve_task` and `resolve_workspace` are the mcp.rs-local helpers already used by `clickup_attachment_upload` (line 5114) — no new imports needed.

- [ ] **Step 4: Classify the tool**

In `src/mcp/classify.rs`, add to the `OVERRIDES` table (line ~77) — required because `embed_image` matches no verb list and would otherwise return `None`:

```rust
    ("clickup_doc_embed_image", Class::Write, "doc"),
```

- [ ] **Step 5: Run tests**

Run: `cargo test --test test_mcp_filter && cargo test`
Expected: PASS, including both `144` assertions. If classify has its own exhaustiveness test, it must pass too.

- [ ] **Step 6: Commit**

```bash
git add src/mcp.rs src/mcp/classify.rs tests/test_mcp_filter.rs
git commit -m "feat(mcp): add clickup_doc_embed_image tool (144 tools)"
```

---

### Task 4: Documentation sweep

**Files:**
- Modify: `CLAUDE.md` (command list line 57, mcp line 73 + 121, embedded agent block line 145)
- Modify: `src/agent_config.rs` (`AGENT_REFERENCE` const — same text as CLAUDE.md embedded block)
- Modify: `docs/commands.md` (doc section, ~line 409)
- Modify: `docs/mcp.md` (all `143` counts + Docs tool-table row, line ~197)
- Modify: `README.md` (all `143` counts)
- Modify: `CHANGELOG.md` (`## [Unreleased]` section)

- [ ] **Step 1: CLAUDE.md command list (line 57)**

```markdown
- `doc` — list, create, get, pages, add-page, page, edit-page, embed-image
```

- [ ] **Step 2: Tool-count bumps**

Run `grep -rn "143" CLAUDE.md README.md docs/mcp.md` and change every MCP-tool-count occurrence to `144` (CLAUDE.md lines 73 & 121; README.md ~lines 312, 316, 454; docs/mcp.md lines 4, 12, 117, 165, 175, 282, 286 — verify each is a tool count, not coincidental).

- [ ] **Step 3: docs/mcp.md Docs tool row (~line 197)**

```markdown
| **Docs (v3)** | doc_list, doc_get, doc_create, doc_pages, doc_get_page, doc_add_page, doc_edit_page, doc_embed_image | 8 |
```

If the table has a totals row, bump it by one as well.

- [ ] **Step 4: docs/commands.md doc section (after the edit-page entry, ~line 415)**

```markdown
### embed-image

Upload a local image and embed it inline in a doc page. The ClickUp API has no doc-level upload, so the image is stored as an attachment on a host task, then referenced from the page as markdown (which ClickUp converts into a native inline image block).

```bash
clickup-cli doc embed-image <DOC_ID> <PAGE_ID> <FILE> [--via-task TASK_ID] [--alt TEXT] [--mode append|prepend]
```

- `--via-task` — host task that stores the image binary. Auto-detected from the git branch if omitted (same resolution as other task-scoped commands).
- `--alt` — image alt text; defaults to the file name.
- `--mode` — `append` (default) or `prepend`. `replace` is intentionally not supported; use `edit-page` if you really want to overwrite a page.

Output: the attachment `url`, `page_id`, and `mode`. With `-q`, just the URL.
```

(Match the surrounding heading level/format in the file — adjust if the doc section uses a different per-command layout.)

- [ ] **Step 5: Embedded agent block — CLAUDE.md line 145 and `AGENT_REFERENCE` in src/agent_config.rs**

Both contain the same one-line command reference. In BOTH places, change the doc segment:

from:
```
doc list|create --name N|get ID|pages ID [--content]|add-page DOC --name N [--content T]|page DOC PAGE|edit-page DOC PAGE --content T [--mode replace|append|prepend];
```
to:
```
doc list|create --name N|get ID|pages ID [--content]|add-page DOC --name N [--content T]|page DOC PAGE|edit-page DOC PAGE --content T [--mode replace|append|prepend]|embed-image DOC PAGE FILE [--via-task ID] [--alt T] [--mode append|prepend];
```

These two MUST stay in sync (known past drift — see git history of #52).

- [ ] **Step 6: CHANGELOG.md under `## [Unreleased]`**

```markdown
### Added
- `doc embed-image` CLI command and `clickup_doc_embed_image` MCP tool (144 tools): upload a local image and embed it inline in a ClickUp doc page. The ClickUp API has no doc-level upload endpoint (the v3 attachment endpoint rejects `docs`/`pages` entity types), so the image is stored as an attachment on a host task (`--via-task`, auto-detected from the git branch like other task-scoped commands), and the returned public CDN URL is appended/prepended to the page as `![alt](url)` markdown, which ClickUp converts into a native inline image block.
```

- [ ] **Step 7: Verify and commit**

Run: `cargo test` (agent_config.rs change must still compile; some tests assert AGENT_REFERENCE content).
Run: `grep -c "embed-image" CLAUDE.md` — expected ≥ 2 (command list + embedded block).

```bash
git add CLAUDE.md README.md docs/commands.md docs/mcp.md src/agent_config.rs CHANGELOG.md
git commit -m "docs: document doc embed-image across CLI/MCP references"
```

---

### Task 5: Live end-to-end verification + cleanup

Pre-existing live fixtures from the design investigation (workspace 2648001):
- Test doc `2gty1-60995` ("clkup-cli-test-image-embed (safe to delete)"), page `2gty1-104915`
- Host task `86ca71ynd` ("clkup-cli image-host test (safe to delete)") in list `901523023065`
- Local test image `/tmp/clkup-test-image.png` (regenerate if missing: any small PNG works)

- [ ] **Step 1: Build and run the real command**

```bash
cargo build
./target/debug/clickup-cli doc embed-image 2gty1-60995 2gty1-104915 /tmp/clkup-test-image.png \
    --via-task 86ca71ynd --alt "e2e verification image"
```

Expected: success; output shows a `https://t2648001.p.clickup-attachments.com/...` URL, page id, mode `append`.

- [ ] **Step 2: Verify the page content round-trip**

```bash
./target/debug/clickup-cli doc page 2gty1-60995 2gty1-104915 --output json
```

Expected: `content` contains a `![](https://t2648001.p.clickup-attachments.com/...)` line for the new upload (ClickUp drops the alt text when converting to a native block — that confirms conversion happened).

- [ ] **Step 3: Quiet mode check**

```bash
./target/debug/clickup-cli doc embed-image 2gty1-60995 2gty1-104915 /tmp/clkup-test-image.png \
    --via-task 86ca71ynd -q
```

Expected: stdout is exactly one line — the attachment URL.

- [ ] **Step 4: Clean up live fixtures**

```bash
./target/debug/clickup-cli task delete 86ca71ynd
# doc delete is not exposed by the CLI (no public API endpoint) — archive instead is fine;
# if `doc` has no delete/archive, leave the doc (it is named "safe to delete") and note it.
rm -f /tmp/clkup-test-image.png /tmp/r.json /tmp/r1.json /tmp/r2.json
```

- [ ] **Step 5: Final full suite**

Run: `cargo test && cargo clippy -- -D warnings && cargo build --release`
Expected: all green.

- [ ] **Step 6: Done — hand back for branch/PR decision**

All commits are local. Use superpowers:finishing-a-development-branch (or open a PR per repo convention) — note the repo works off `main` with PRs (#50, #62, #63 pattern).
