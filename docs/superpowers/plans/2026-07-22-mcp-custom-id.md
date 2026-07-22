# MCP `custom_id` Exposure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Surface `custom_id` in the compact responses of every task-returning MCP tool, omitting the key when the task has no custom ID (GitHub issue #82).

**Architecture:** All MCP tool responses are projected through `output::compact_items(items, fields)`, which emits every listed field with a `"-"` placeholder when missing. We teach it an optional-field convention — a trailing `?` on a field name (e.g. `"custom_id?"`) means "emit under the clean name only when present and non-null" — then add `"custom_id?"` to the six task-returning tools' field lists. No API request changes: ClickUp already returns `custom_id` on task objects.

**Tech Stack:** Rust, serde_json, wiremock + assert_cmd for integration tests.

**Spec:** `docs/superpowers/specs/2026-07-22-mcp-custom-id-design.md`

## Global Constraints

- Backward compatible: tasks without a custom ID must produce byte-identical output to today.
- Affected tools (6): `clickup_task_get`, `clickup_task_search`, `clickup_task_list`, `clickup_task_create`, `clickup_task_update`, `clickup_view_tasks`. (`clickup_task_move` was listed in the spec but returns only a `{"message": ...}` confirmation, not a task object — out of scope; Task 2 amends the spec.)
- Non-optional fields in `compact_items` keep exact current behaviour (always present, `"-"` when missing/null).
- Work on branch `feat/mcp-custom-id-issue-82`.

---

### Task 1: Optional-field marker in `compact_items`

**Files:**
- Modify: `src/output.rs:92-107` (the `compact_items` function)
- Test: `tests/test_output.rs` (append)

**Interfaces:**
- Consumes: existing `flatten_value(Option<&serde_json::Value>) -> String` in `src/output.rs`.
- Produces: `compact_items(items: &[serde_json::Value], fields: &[&str]) -> serde_json::Value` now interprets a field spelled with a trailing `?` (e.g. `"custom_id?"`) as optional: output key is the name without the `?`, and the key is omitted entirely when the source value is missing or JSON null. Task 2 relies on exactly this.

- [ ] **Step 1: Write the failing tests**

Append to `tests/test_output.rs` (note: this file currently only imports `flatten_value` and `OutputConfig`; add `compact_items` to the existing `use clickup_cli::output::{...}` line):

```rust
#[test]
fn compact_items_includes_optional_field_when_present() {
    let items = vec![json!({"id": "abc123", "name": "demo", "custom_id": "PROJ-42"})];
    let result = compact_items(&items, &["id", "name", "custom_id?"]);
    assert_eq!(result[0]["id"], json!("abc123"));
    assert_eq!(result[0]["custom_id"], json!("PROJ-42"));
    // The marker itself must never leak into the output.
    assert!(result[0].get("custom_id?").is_none());
}

#[test]
fn compact_items_omits_optional_field_when_null() {
    let items = vec![json!({"id": "abc123", "name": "demo", "custom_id": null})];
    let result = compact_items(&items, &["id", "name", "custom_id?"]);
    assert!(result[0].get("custom_id").is_none());
    assert_eq!(result[0]["name"], json!("demo"));
}

#[test]
fn compact_items_omits_optional_field_when_missing() {
    let items = vec![json!({"id": "abc123", "name": "demo"})];
    let result = compact_items(&items, &["id", "name", "custom_id?"]);
    assert!(result[0].get("custom_id").is_none());
}

#[test]
fn compact_items_required_field_still_placeholder_when_missing() {
    let items = vec![json!({"id": "abc123"})];
    let result = compact_items(&items, &["id", "name"]);
    assert_eq!(result[0]["name"], json!("-"));
}
```

- [ ] **Step 2: Run tests to verify the new ones fail**

Run: `cargo test --test test_output`
Expected: the three `optional_field` tests FAIL (current code emits key `"custom_id?"` with value `"-"` or the flattened value); `compact_items_required_field_still_placeholder_when_missing` PASSES (documents existing behaviour).

- [ ] **Step 3: Implement the marker in `compact_items`**

Replace the function in `src/output.rs` (keep `flatten_value` untouched):

```rust
/// Flatten a list of items to only include the specified fields with flattened values.
/// Returns a JSON array. Used by MCP server for token-efficient responses.
///
/// A field name with a trailing `?` (e.g. `"custom_id?"`) is optional: it is
/// emitted under the name without the marker, and only when the source value
/// is present and non-null. All other fields are always emitted, with `"-"`
/// as the placeholder for missing/null values.
pub fn compact_items(items: &[serde_json::Value], fields: &[&str]) -> serde_json::Value {
    let compacted: Vec<serde_json::Value> = items
        .iter()
        .map(|item| {
            let mut obj = serde_json::Map::new();
            for &field in fields {
                if let Some(key) = field.strip_suffix('?') {
                    match item.get(key) {
                        None | Some(serde_json::Value::Null) => {}
                        Some(v) => {
                            let val = flatten_value(Some(v));
                            obj.insert(key.to_string(), serde_json::Value::String(val));
                        }
                    }
                } else {
                    let val = flatten_value(item.get(field));
                    obj.insert(field.to_string(), serde_json::Value::String(val));
                }
            }
            serde_json::Value::Object(obj)
        })
        .collect();
    serde_json::Value::Array(compacted)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test test_output`
Expected: all tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/output.rs tests/test_output.rs
git commit -m "feat(output): optional-field '?' marker in compact_items (#82)"
```

---

### Task 2: Add `custom_id?` to the six task-returning MCP tools

**Files:**
- Modify: `src/mcp.rs` — six field lists (approx. lines 2461, 2498, 2548, 2585, 2607, 2901) and four tool descriptions (approx. lines 237, 262, 340, 592)
- Modify: `docs/superpowers/specs/2026-07-22-mcp-custom-id-design.md` (scope amendment)
- Test: `tests/test_mcp_custom_id.rs` (create)

**Interfaces:**
- Consumes: `compact_items` optional-field marker from Task 1 (`"custom_id?"` → key `custom_id` emitted only when set).
- Produces: MCP tool responses for the six tools carry `"custom_id": "<value>"` when the task has one; unchanged otherwise.

- [ ] **Step 1: Write the failing integration test**

Create `tests/test_mcp_custom_id.rs`. The MCP server handles `tools/call` statelessly line-by-line over stdio and exits cleanly when stdin closes, so a test can spawn the binary, write one JSON-RPC line, and assert on stdout. Follow the wiremock + env pattern of `tests/test_task_markdown.rs`:

```rust
use assert_cmd::Command;
use std::path::Path;
use tempfile::TempDir;
use wiremock::matchers::{method, path as path_matcher};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn mcp_serve(dir: &Path, server: &MockServer) -> Command {
    let mut cmd = Command::cargo_bin("clickup-cli").unwrap();
    cmd.current_dir(dir)
        .args(["mcp", "serve"])
        .env("CLICKUP_API_URL", server.uri())
        .env("CLICKUP_TOKEN", "pk_test")
        .env("CLICKUP_WORKSPACE", "99")
        .env_remove("CLICKUP_GIT_DETECT")
        .env_remove("CLICKUP_TASK_ID");
    cmd
}

fn rpc_call(tool: &str, arguments: serde_json::Value) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {"name": tool, "arguments": arguments}
    })
    .to_string()
        + "\n"
}

#[tokio::test]
async fn task_get_includes_custom_id_when_set() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_matcher("/v2/task/abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "abc123",
            "name": "demo",
            "custom_id": "PROJ-42",
            "status": {"status": "open"},
        })))
        .expect(1)
        .mount(&server)
        .await;

    mcp_serve(dir.path(), &server)
        .write_stdin(rpc_call(
            "clickup_task_get",
            serde_json::json!({"task_id": "abc123"}),
        ))
        .assert()
        .success()
        .stdout(predicates::str::contains("custom_id"))
        .stdout(predicates::str::contains("PROJ-42"));
}

#[tokio::test]
async fn task_get_omits_custom_id_when_null() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_matcher("/v2/task/abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "abc123",
            "name": "demo",
            "custom_id": null,
        })))
        .expect(1)
        .mount(&server)
        .await;

    mcp_serve(dir.path(), &server)
        .write_stdin(rpc_call(
            "clickup_task_get",
            serde_json::json!({"task_id": "abc123"}),
        ))
        .assert()
        .success()
        .stdout(predicates::str::contains("custom_id").not());
}

#[tokio::test]
async fn task_search_includes_custom_id_when_set() {
    let dir = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_matcher("/v2/team/99/task"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tasks": [
                {"id": "abc123", "name": "with custom", "custom_id": "PROJ-42"},
                {"id": "def456", "name": "without custom", "custom_id": null},
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    mcp_serve(dir.path(), &server)
        .write_stdin(rpc_call("clickup_task_search", serde_json::json!({})))
        .assert()
        .success()
        .stdout(predicates::str::contains("PROJ-42"));
}
```

Add `use predicates::prelude::*;` if `.not()` fails to resolve — `predicates::str::contains(...).not()` requires the `PredicateBooleanExt` trait from the prelude.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test test_mcp_custom_id`
Expected: `task_get_includes_custom_id_when_set` and `task_search_includes_custom_id_when_set` FAIL (no `custom_id`/`PROJ-42` in stdout); `task_get_omits_custom_id_when_null` PASSES (nothing emits custom_id yet).

- [ ] **Step 3: Add `"custom_id?"` to the six field lists in `src/mcp.rs`**

In each of the following handlers, insert `"custom_id?"` immediately after `"id"` in the compact field list:

1. `clickup_task_list` (~line 2461, inside `pagination::page_dispatch`):
```rust
&["id", "custom_id?", "name", "status", "priority", "assignees", "due_date"],
```
2. `clickup_task_get` (~line 2498, the `let mut fields = vec![...]`):
```rust
let mut fields = vec![
    "id",
    "custom_id?",
    "name",
    "status",
    "priority",
    "assignees",
    "due_date",
    "description",
];
```
3. `clickup_task_create` (~line 2548, inside `compact_items`):
```rust
&["id", "custom_id?", "name", "status", "priority", "assignees", "due_date"],
```
4. `clickup_task_update` (~line 2585, inside `compact_items`):
```rust
&["id", "custom_id?", "name", "status", "priority", "assignees", "due_date"],
```
5. `clickup_task_search` (~line 2607, inside `pagination::page_dispatch`):
```rust
&["id", "custom_id?", "name", "status", "priority", "assignees", "due_date"],
```
6. `clickup_view_tasks` (~line 2901, inside `pagination::page_dispatch`):
```rust
&["id", "custom_id?", "name", "status", "priority", "assignees", "due_date"],
```

- [ ] **Step 4: Update the four tool descriptions that describe returned task shapes**

All in `src/mcp.rs`. Only the quoted fragments change:

1. `clickup_task_list` (~line 237): change `Returns the first page of task objects in compact form (id, name, status, assignees, due_date).` to `Returns the first page of task objects in compact form (id, name, status, assignees, due_date; custom_id when the task has one).`
2. `clickup_task_get` (~line 262): after `Returns the task object.` add ` Includes the task's custom_id when one is set.`
3. `clickup_task_search` (~line 340): change `Returns a compact array of task objects.` to `Returns a compact array of task objects (custom_id included when set).`
4. `clickup_view_tasks` (~line 592): change `Returns a compact array of task objects.` to `Returns a compact array of task objects (custom_id included when set).`

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --test test_mcp_custom_id`
Expected: all 3 tests PASS.

- [ ] **Step 6: Amend the spec's scope note**

In `docs/superpowers/specs/2026-07-22-mcp-custom-id-design.md`, change the affected-tools line to six tools and note why `clickup_task_move` dropped out:

Replace:
```
**Affected tools (7):** `clickup_task_get`, `clickup_task_search`,
`clickup_task_list`, `clickup_task_create`, `clickup_task_update`,
`clickup_view_tasks`, `clickup_task_move`.
```
With:
```
**Affected tools (6):** `clickup_task_get`, `clickup_task_search`,
`clickup_task_list`, `clickup_task_create`, `clickup_task_update`,
`clickup_view_tasks`. (`clickup_task_move` was originally in scope but
returns only a `{"message": ...}` confirmation, not a task object, so
there is nothing to add a field to.)
```

- [ ] **Step 7: Commit**

```bash
git add src/mcp.rs tests/test_mcp_custom_id.rs docs/superpowers/specs/2026-07-22-mcp-custom-id-design.md
git commit -m "feat(mcp): include custom_id in task tool responses when set (#82)"
```

---

### Task 3: Changelog + full verification

**Files:**
- Modify: `CHANGELOG.md` (the `## [Unreleased]` section)

**Interfaces:**
- Consumes: the finished behaviour from Tasks 1–2.
- Produces: release notes; a fully green test suite.

- [ ] **Step 1: Add the changelog entry**

Under `## [Unreleased]` in `CHANGELOG.md`, add:

```markdown
### Added
- MCP task tools now include the task's `custom_id` in their compact responses (#82). Applies to `clickup_task_get`, `clickup_task_search`, `clickup_task_list`, `clickup_task_create`, `clickup_task_update`, and `clickup_view_tasks`. The field is only present when the task actually has a custom ID, so workspaces that don't use custom task IDs see byte-identical output (and pay no extra tokens). The ClickUp API already returns `custom_id` on task objects — no request changes; the `custom_task_ids=true` query parameter remains input-side only (addressing a task *by* its custom ID), which the MCP server already supported.
```

(If a later task has already created an `### Added` heading under `[Unreleased]`, append the bullet to it instead of adding a second heading.)

- [ ] **Step 2: Run the full test suite and lints**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: all tests PASS, no clippy warnings, no formatting diffs. If `cargo fmt --check` fails, run `cargo fmt` and include the reformat in the commit.

- [ ] **Step 3: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs: changelog entry for MCP custom_id exposure (#82)"
```
