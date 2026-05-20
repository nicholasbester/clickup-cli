//! MCP pagination helpers.
//!
//! ClickUp's paginated endpoints come in two flavours: v2 page-based (`?page=N`,
//! response carries `last_page: bool`) and v3 cursor-based (`?cursor=X`, response
//! carries `next_cursor: string` or null). This module hides the loop logic and
//! response-shape glue so each MCP tool dispatch can be a one-liner.
//!
//! ## Contract
//!
//! **Schema.** Every paginated tool's inputSchema gains either:
//! - page style: `page` (int ≥0), `limit` (int ≥1), `all` (bool); or
//! - cursor style: `cursor` (opaque string), `limit` (int ≥1), `all` (bool).
//!
//! **Output.** The contract is _opt-in_:
//! - If the caller passes NO pagination arg, the response is unchanged from
//!   pre-pagination: a bare compact array. Back-compat for existing clients.
//! - If the caller passes ANY pagination arg (`page`, `cursor`, `limit`, `all`),
//!   the response becomes an envelope:
//!
//!   ```json
//!   {
//!     "items": [...],
//!     "pagination": {
//!       "style": "page" | "cursor",
//!       "page": 0,            // page style only
//!       "last_page": false,   // page style only
//!       "next_cursor": "...", // cursor style only, omitted when exhausted
//!       "has_more": true,
//!       "returned": 42,
//!       "all": false
//!     }
//!   }
//!   ```
//!
//! Calling code uses [`PageArgs::from_args`] / [`CursorArgs::from_args`] to
//! parse pagination input, then [`page_dispatch`] / [`cursor_dispatch`] to run
//! the fetch loop.

use crate::client::ClickUpClient;
use crate::output::compact_items;
use serde_json::{json, Value};

/// Hard cap on how many pages a single `all=true` call will fetch. Guards
/// against runaway loops on misbehaving cursor endpoints.
const MAX_PAGES: usize = 100;

/// Parsed page-based pagination input.
#[derive(Debug, Clone, Copy, Default)]
pub struct PageArgs {
    pub page: Option<u64>,
    pub limit: Option<usize>,
    pub all: bool,
    /// True if the caller passed ANY of the pagination args. Drives the
    /// opt-in envelope shape.
    pub requested: bool,
}

impl PageArgs {
    pub fn from_args(args: &Value) -> Self {
        let page = args.get("page").and_then(|v| v.as_u64());
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        let all = args.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
        let requested = page.is_some() || limit.is_some() || args.get("all").is_some();
        Self {
            page,
            limit,
            all,
            requested,
        }
    }
}

/// Parsed cursor-based pagination input.
#[derive(Debug, Clone, Default)]
pub struct CursorArgs {
    pub cursor: Option<String>,
    pub limit: Option<usize>,
    pub all: bool,
    pub requested: bool,
}

impl CursorArgs {
    pub fn from_args(args: &Value) -> Self {
        let cursor = args
            .get("cursor")
            .and_then(|v| v.as_str())
            .map(String::from);
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        let all = args.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
        let requested = cursor.is_some() || limit.is_some() || args.get("all").is_some();
        Self {
            cursor,
            limit,
            all,
            requested,
        }
    }
}

/// Run a page-based pagination loop and return either a bare compact array
/// (no pagination args) or a `{items, pagination}` envelope (any pagination
/// arg). `build_path(page)` should return a path including any `?page=N`
/// query, e.g. `/v2/list/123/task?page=2`. `items_key` is the response field
/// holding the array (e.g. `"tasks"`).
pub async fn page_dispatch<F>(
    args: &PageArgs,
    client: &ClickUpClient,
    items_key: &str,
    compact_fields: &[&str],
    build_path: F,
) -> Result<Value, String>
where
    F: Fn(u64) -> String,
{
    let start_page = args.page.unwrap_or(0);
    let mut collected: Vec<Value> = Vec::new();
    let mut current_page = start_page;
    // Initial value is overwritten on the first loop iteration; the variable
    // exists so the value survives the loop and feeds the pagination envelope.
    #[allow(unused_assignments)]
    let mut last_page = false;
    let mut pages_fetched = 0usize;

    loop {
        let path = build_path(current_page);
        let resp = client.get(&path).await.map_err(|e| e.to_string())?;

        let items = extract_array(&resp, &[items_key, "data"]).unwrap_or_default();

        last_page = resp
            .get("last_page")
            .and_then(|v| v.as_bool())
            .unwrap_or(items.is_empty());

        collected.extend(items);
        pages_fetched += 1;

        // Stop conditions: page-only mode (no `all`), reached `last_page`,
        // exceeded `limit`, or hit the page-cap guard.
        if !args.all {
            break;
        }
        if last_page || pages_fetched >= MAX_PAGES {
            break;
        }
        if let Some(limit) = args.limit {
            if collected.len() >= limit {
                break;
            }
        }
        current_page += 1;
    }

    // Honour caller-provided `limit` after collection.
    if let Some(limit) = args.limit {
        collected.truncate(limit);
    }

    let compact = compact_items(&collected, compact_fields);

    if !args.requested {
        // Back-compat: return the bare array exactly like the pre-pagination
        // codepath did, so existing clients see no shape change.
        return Ok(compact);
    }

    let compact_arr = compact.as_array().cloned().unwrap_or_default();
    let returned = compact_arr.len();
    let has_more = !last_page && args.limit.is_none_or(|l| returned < l);
    let last_observed_page = if args.all { current_page } else { start_page };
    Ok(json!({
        "items": compact_arr,
        "pagination": {
            "style": "page",
            "page": last_observed_page,
            "last_page": last_page,
            "has_more": has_more,
            "returned": returned,
            "all": args.all,
        }
    }))
}

/// Run a cursor-based pagination loop. `build_path(cursor)` should return a
/// path including any `?cursor=...` query when `cursor` is Some, or no cursor
/// query when None. `items_keys` is the list of candidate response keys to
/// extract the array from; first match wins. Typical: `&["data", "<legacy>"]`
/// where `<legacy>` is the pre-v3 key (`"channels"`, `"replies"`, etc.) for
/// back-compat with any older envelope shape.
pub async fn cursor_dispatch<F>(
    args: &CursorArgs,
    client: &ClickUpClient,
    items_keys: &[&str],
    compact_fields: &[&str],
    build_path: F,
) -> Result<Value, String>
where
    F: Fn(Option<&str>) -> String,
{
    let mut cursor = args.cursor.clone();
    let mut collected: Vec<Value> = Vec::new();
    // Initial value is overwritten on the first loop iteration; the variable
    // exists so the value survives the loop and feeds the pagination envelope.
    #[allow(unused_assignments)]
    let mut next_cursor: Option<String> = None;
    let mut pages_fetched = 0usize;

    loop {
        let path = build_path(cursor.as_deref());
        let resp = client.get(&path).await.map_err(|e| e.to_string())?;

        let items = extract_array(&resp, items_keys).unwrap_or_default();

        next_cursor = resp
            .get("next_cursor")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from);

        collected.extend(items);
        pages_fetched += 1;

        if !args.all {
            break;
        }
        if next_cursor.is_none() || pages_fetched >= MAX_PAGES {
            break;
        }
        if let Some(limit) = args.limit {
            if collected.len() >= limit {
                break;
            }
        }
        cursor = next_cursor.clone();
    }

    if let Some(limit) = args.limit {
        collected.truncate(limit);
    }

    let compact = compact_items(&collected, compact_fields);

    if !args.requested {
        return Ok(compact);
    }

    let compact_arr = compact.as_array().cloned().unwrap_or_default();
    let returned = compact_arr.len();
    let has_more = next_cursor.is_some() && args.limit.is_none_or(|l| returned < l);

    let mut pagination = serde_json::Map::new();
    pagination.insert("style".into(), json!("cursor"));
    pagination.insert("has_more".into(), json!(has_more));
    pagination.insert("returned".into(), json!(returned));
    pagination.insert("all".into(), json!(args.all));
    if let Some(c) = next_cursor {
        pagination.insert("next_cursor".into(), json!(c));
    }
    Ok(json!({
        "items": compact_arr,
        "pagination": Value::Object(pagination),
    }))
}

/// Extract an array from a JSON response, trying multiple candidate keys in
/// order. Returns `None` if no candidate key holds an array. Used to resolve
/// the v3 `"data"` envelope vs older list keys.
fn extract_array(resp: &Value, keys: &[&str]) -> Option<Vec<Value>> {
    for key in keys {
        if let Some(arr) = resp.get(key).and_then(|v| v.as_array()) {
            return Some(arr.clone());
        }
    }
    // The response itself may be a bare array (some endpoints).
    if let Some(arr) = resp.as_array() {
        return Some(arr.clone());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_client(server: &MockServer) -> ClickUpClient {
        ClickUpClient::new("pk_test", 30)
            .expect("client")
            .with_base_url(&server.uri())
    }

    #[tokio::test]
    async fn page_dispatch_no_pagination_args_returns_bare_array() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/list/L1/task"))
            .and(query_param("page", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "tasks": [{"id": "t1", "name": "A"}, {"id": "t2", "name": "B"}],
                "last_page": true,
            })))
            .mount(&server)
            .await;
        let client = test_client(&server);
        let args = PageArgs::from_args(&json!({}));
        let result = page_dispatch(&args, &client, "tasks", &["id", "name"], |p| {
            format!("/v2/list/L1/task?page={}", p)
        })
        .await
        .unwrap();
        // No pagination requested: bare array, same shape as pre-pagination code.
        assert!(result.is_array(), "expected bare array, got {}", result);
        assert_eq!(result.as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn page_dispatch_envelope_when_requested() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/list/L1/task"))
            .and(query_param("page", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "tasks": [{"id": "t1", "name": "A"}],
                "last_page": false,
            })))
            .mount(&server)
            .await;
        let client = test_client(&server);
        let args = PageArgs::from_args(&json!({"page": 0}));
        let result = page_dispatch(&args, &client, "tasks", &["id", "name"], |p| {
            format!("/v2/list/L1/task?page={}", p)
        })
        .await
        .unwrap();
        // Pagination requested: envelope shape.
        assert!(result.is_object(), "expected envelope, got {}", result);
        let items = result.get("items").and_then(|v| v.as_array()).unwrap();
        assert_eq!(items.len(), 1);
        let p = result.get("pagination").unwrap();
        assert_eq!(p.get("style").and_then(|v| v.as_str()), Some("page"));
        assert_eq!(p.get("last_page").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(p.get("has_more").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(p.get("returned").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(p.get("all").and_then(|v| v.as_bool()), Some(false));
    }

    #[tokio::test]
    async fn page_dispatch_all_true_walks_pages_until_last() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/list/L1/task"))
            .and(query_param("page", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "tasks": [{"id": "t1"}, {"id": "t2"}],
                "last_page": false,
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/list/L1/task"))
            .and(query_param("page", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "tasks": [{"id": "t3"}],
                "last_page": true,
            })))
            .mount(&server)
            .await;
        let client = test_client(&server);
        let args = PageArgs::from_args(&json!({"all": true}));
        let result = page_dispatch(&args, &client, "tasks", &["id"], |p| {
            format!("/v2/list/L1/task?page={}", p)
        })
        .await
        .unwrap();
        let items = result.get("items").and_then(|v| v.as_array()).unwrap();
        assert_eq!(items.len(), 3, "expected 3 items across 2 pages");
        let p = result.get("pagination").unwrap();
        assert_eq!(p.get("last_page").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(p.get("has_more").and_then(|v| v.as_bool()), Some(false));
    }

    #[tokio::test]
    async fn cursor_dispatch_follows_next_cursor() {
        let server = MockServer::start().await;
        // First call: no cursor; respond with one item + next_cursor=ABC.
        Mock::given(method("GET"))
            .and(path("/v3/workspaces/2648001/docs"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{"id": "d1", "name": "First"}],
                "next_cursor": "ABC",
            })))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        // Second call: cursor=ABC; respond with one more item + no cursor.
        Mock::given(method("GET"))
            .and(path("/v3/workspaces/2648001/docs"))
            .and(query_param("cursor", "ABC"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{"id": "d2", "name": "Second"}],
                "next_cursor": null,
            })))
            .mount(&server)
            .await;
        let client = test_client(&server);
        let args = CursorArgs::from_args(&json!({"all": true}));
        let result = cursor_dispatch(&args, &client, &["data"], &["id", "name"], |c| match c {
            Some(c) => format!("/v3/workspaces/2648001/docs?cursor={}", c),
            None => "/v3/workspaces/2648001/docs".to_string(),
        })
        .await
        .unwrap();
        let items = result.get("items").and_then(|v| v.as_array()).unwrap();
        assert_eq!(items.len(), 2, "expected 2 items across 2 pages");
        let p = result.get("pagination").unwrap();
        assert_eq!(p.get("has_more").and_then(|v| v.as_bool()), Some(false));
        // next_cursor should be absent (exhausted) per the contract docs.
        assert!(p.get("next_cursor").is_none());
    }

    #[test]
    fn page_args_empty() {
        let p = PageArgs::from_args(&json!({}));
        assert!(!p.requested);
        assert_eq!(p.page, None);
        assert_eq!(p.limit, None);
        assert!(!p.all);
    }

    #[test]
    fn page_args_full() {
        let p = PageArgs::from_args(&json!({"page": 2, "limit": 50, "all": true}));
        assert!(p.requested);
        assert_eq!(p.page, Some(2));
        assert_eq!(p.limit, Some(50));
        assert!(p.all);
    }

    #[test]
    fn page_args_just_all_flag() {
        let p = PageArgs::from_args(&json!({"all": false}));
        // Passing `all: false` is still an explicit pagination intent.
        assert!(p.requested);
    }

    #[test]
    fn cursor_args_empty() {
        let c = CursorArgs::from_args(&json!({}));
        assert!(!c.requested);
        assert!(c.cursor.is_none());
    }

    #[test]
    fn cursor_args_full() {
        let c = CursorArgs::from_args(&json!({"cursor": "abc", "limit": 10}));
        assert!(c.requested);
        assert_eq!(c.cursor.as_deref(), Some("abc"));
        assert_eq!(c.limit, Some(10));
    }

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
        assert_eq!(arr.len(), 2);
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
