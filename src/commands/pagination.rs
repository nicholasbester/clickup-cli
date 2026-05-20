//! CLI-side pagination walkers.
//!
//! Mirror of `crate::mcp::pagination`, but tuned for the CLI's output model:
//! these helpers walk pages and return the accumulated raw items as a
//! `Vec<Value>` so the existing `OutputConfig::print_items` machinery can
//! format them as tables, JSON, or CSV. No compaction, no envelope — just
//! the loop logic and termination handling shared across every paginated
//! CLI command.
//!
//! ## Contract
//!
//! Every walker respects the global pagination flags on [`crate::Cli`]:
//! - `--all`: auto-fetch pages until natural termination or `--limit` hit.
//! - `--limit N`: cap total items returned across pages.
//! - `--page N` (page-style only): manual page selection. With `--all`,
//!   the starting page.
//! - `--cursor X` (cursor-style only): manual cursor. With `--all`, the
//!   starting cursor.
//! - `--start MS` + `--start-id ID` (start-id-style only): manual boundary
//!   pair. With `--all`, the starting boundary.
//!
//! `--limit` is enforced **after** walking, so `--all --limit 500` returns
//! up to 500 items across N pages rather than truncating the first page to
//! 500. A hard cap of 100 pages prevents runaway loops.

use crate::client::ClickUpClient;
use crate::error::CliError;
use crate::Cli;
use serde_json::Value;

/// Hard cap on how many pages a single `--all` invocation will fetch.
/// Guards against runaway loops on misbehaving cursor endpoints.
const MAX_PAGES: usize = 100;

/// Extract an array from a JSON response, trying multiple candidate keys
/// in order. Returns `None` if no candidate key holds an array, falling
/// back to checking whether the whole response is itself a bare array.
fn extract_array(resp: &Value, keys: &[&str]) -> Option<Vec<Value>> {
    for key in keys {
        if let Some(arr) = resp.get(key).and_then(|v| v.as_array()) {
            return Some(arr.clone());
        }
    }
    if let Some(arr) = resp.as_array() {
        return Some(arr.clone());
    }
    None
}

/// Page-based walker for v2 endpoints (`?page=N`, response carries
/// `last_page: bool`). `build_path(page)` returns the URL for a given page.
/// `items_key` is the response field holding the array (e.g. `"tasks"`).
pub async fn walk_page<F>(
    cli: &Cli,
    client: &ClickUpClient,
    items_key: &str,
    build_path: F,
) -> Result<Vec<Value>, CliError>
where
    F: Fn(u32) -> String,
{
    let start_page = cli.page.unwrap_or(0);
    let mut collected: Vec<Value> = Vec::new();
    let mut current_page = start_page;
    let mut pages_fetched = 0usize;

    loop {
        let resp = client.get(&build_path(current_page)).await?;
        let items = extract_array(&resp, &[items_key, "data"]).unwrap_or_default();
        let last_page = resp
            .get("last_page")
            .and_then(|v| v.as_bool())
            .unwrap_or(items.is_empty());
        collected.extend(items);
        pages_fetched += 1;

        if !cli.all {
            break;
        }
        if last_page || pages_fetched >= MAX_PAGES {
            break;
        }
        if let Some(limit) = cli.limit {
            if collected.len() >= limit {
                break;
            }
        }
        current_page += 1;
    }

    if let Some(limit) = cli.limit {
        collected.truncate(limit);
    }
    Ok(collected)
}

/// Cursor-based walker for v3 endpoints (`?cursor=X`, response carries
/// `next_cursor: string` or empty). `build_path(Option<&str>)` returns the
/// URL given an optional cursor. `items_keys` is the priority list of
/// candidate response keys (e.g. `&["data", "channels"]`).
pub async fn walk_cursor<F>(
    cli: &Cli,
    client: &ClickUpClient,
    items_keys: &[&str],
    build_path: F,
) -> Result<Vec<Value>, CliError>
where
    F: Fn(Option<&str>) -> String,
{
    let mut cursor = cli.cursor.clone();
    let mut collected: Vec<Value> = Vec::new();
    let mut pages_fetched = 0usize;

    loop {
        let resp = client.get(&build_path(cursor.as_deref())).await?;
        let items = extract_array(&resp, items_keys).unwrap_or_default();
        let next_cursor: Option<String> = resp
            .get("next_cursor")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from);
        collected.extend(items);
        pages_fetched += 1;

        if !cli.all {
            break;
        }
        if next_cursor.is_none() || pages_fetched >= MAX_PAGES {
            break;
        }
        if let Some(limit) = cli.limit {
            if collected.len() >= limit {
                break;
            }
        }
        cursor = next_cursor;
    }

    if let Some(limit) = cli.limit {
        collected.truncate(limit);
    }
    Ok(collected)
}

/// Start-id-based walker for v2 comment endpoints
/// (`?start=<ms>&start_id=<id>` paired, response carries `{comments: [...]}`
/// with termination inferred from short page). `build_path(Option<i64>,
/// Option<&str>)` returns the URL given an optional boundary pair.
/// `items_key` is the response key (typically `"comments"`).
pub async fn walk_start_id<F>(
    cli: &Cli,
    client: &ClickUpClient,
    items_key: &str,
    build_path: F,
) -> Result<Vec<Value>, CliError>
where
    F: Fn(Option<i64>, Option<&str>) -> String,
{
    const PAGE_HINT: usize = 25;
    let mut current_start = cli.start;
    let mut current_start_id = cli.start_id.clone();
    let mut collected: Vec<Value> = Vec::new();
    let mut pages_fetched = 0usize;

    loop {
        let resp = client
            .get(&build_path(current_start, current_start_id.as_deref()))
            .await?;
        let items = extract_array(&resp, &[items_key, "data"]).unwrap_or_default();
        let count = items.len();

        // Derive next boundary from the last item BEFORE consuming items.
        let next_boundary = items.last().and_then(|last| {
            let date = last
                .get("date")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<i64>().ok())
                .or_else(|| last.get("date").and_then(|v| v.as_i64()));
            let id = last.get("id").and_then(|v| v.as_str()).map(String::from);
            match (date, id) {
                (Some(d), Some(i)) => Some((d, i)),
                _ => None,
            }
        });

        collected.extend(items);
        pages_fetched += 1;

        if !cli.all {
            break;
        }
        if count < PAGE_HINT || pages_fetched >= MAX_PAGES {
            break;
        }
        if let Some(limit) = cli.limit {
            if collected.len() >= limit {
                break;
            }
        }
        match next_boundary {
            Some((d, i)) => {
                current_start = Some(d);
                current_start_id = Some(i);
            }
            None => break,
        }
    }

    if let Some(limit) = cli.limit {
        collected.truncate(limit);
    }
    Ok(collected)
}

/// Body-based walker for the v3 audit-log endpoint. POST to `path` with
/// `base_body() + pagination block`; advance via `next_timestamp(last_item)`.
/// `extra_pagination` lets the caller pre-populate `pageRows` /
/// `pageDirection` fields the helper doesn't manage directly.
///
/// The 8-arg signature is intentional: collapsing the four
/// caller-provided behaviours (URL, body, pagination state, advance) into
/// a struct would make every call site noisier without adding clarity.
#[allow(clippy::too_many_arguments)]
pub async fn walk_body<BB, NT>(
    cli: &Cli,
    client: &ClickUpClient,
    path: &str,
    items_keys: &[&str],
    base_body: BB,
    extra_pagination: serde_json::Map<String, Value>,
    start_timestamp: Option<i64>,
    next_timestamp: NT,
) -> Result<Vec<Value>, CliError>
where
    BB: Fn() -> Value,
    NT: Fn(&Value) -> Option<i64>,
{
    let mut current_timestamp = start_timestamp;
    let mut collected: Vec<Value> = Vec::new();
    let mut pages_fetched = 0usize;

    loop {
        let mut body = base_body();
        let mut pagination = extra_pagination.clone();
        if let Some(t) = current_timestamp {
            pagination.insert("pageTimestamp".into(), serde_json::json!(t));
        }
        if !pagination.is_empty() {
            body["pagination"] = Value::Object(pagination);
        }

        let resp = client.post(path, &body).await?;
        let items = extract_array(&resp, items_keys).unwrap_or_default();
        let count = items.len();
        let next = items.last().and_then(&next_timestamp);
        collected.extend(items);
        pages_fetched += 1;

        if !cli.all {
            break;
        }
        if count == 0 || next.is_none() || pages_fetched >= MAX_PAGES {
            break;
        }
        if let Some(limit) = cli.limit {
            if collected.len() >= limit {
                break;
            }
        }
        current_timestamp = next;
    }

    if let Some(limit) = cli.limit {
        collected.truncate(limit);
    }
    Ok(collected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_array_prefers_first_key() {
        let resp = json!({"data": [1, 2], "tasks": [3, 4]});
        let arr = extract_array(&resp, &["data", "tasks"]).unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0], json!(1));
    }

    #[test]
    fn extract_array_falls_back_to_second_key() {
        let resp = json!({"tasks": [3, 4]});
        let arr = extract_array(&resp, &["data", "tasks"]).unwrap();
        assert_eq!(arr[0], json!(3));
    }

    #[test]
    fn extract_array_falls_back_to_bare_array() {
        let resp = json!([1, 2, 3]);
        let arr = extract_array(&resp, &["data"]).unwrap();
        assert_eq!(arr.len(), 3);
    }

    #[test]
    fn extract_array_returns_none_when_no_match() {
        let resp = json!({"foo": "bar"});
        assert!(extract_array(&resp, &["data", "tasks"]).is_none());
    }
}
