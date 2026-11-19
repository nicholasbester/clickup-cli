//! MCP pagination helpers.
//!
//! ClickUp's paginated endpoints come in four flavours:
//! - **page-based** (v2): `?page=N`, response carries `last_page: bool`.
//! - **cursor-based** (v3): `?cursor=X`, response carries `next_cursor: string` or null.
//! - **start-id-based** (v2 comments): `?start=<unix_ms>&start_id=<id>` pair,
//!   response is a bare array, termination inferred from page-size hint.
//! - **body-based** (v3 audit-log): pagination state lives inside the POST
//!   body as `pagination: { pageRows, pageTimestamp, pageDirection }`.
//!
//! This module hides the loop logic and response-shape glue so each MCP tool
//! dispatch can be a one-liner.
//!
//! ## Contract
//!
//! **Schema.** Every paginated tool's inputSchema gains one of:
//! - page style: `page` (int ≥0), `limit` (int ≥1), `all` (bool); or
//! - cursor style: `cursor` (opaque string), `limit` (int ≥1), `all` (bool); or
//! - start-id style: `start` (int ms), `start_id` (string), `limit` (int ≥1), `all` (bool); or
//! - body style: `page_rows`, `page_timestamp` (int ms), `page_direction`
//!   (`"NEXT"`/`"PREVIOUS"`), `limit`, `all`.
//!
//! **Output.** The contract is _opt-in_:
//! - If the caller passes NO pagination arg, the response is unchanged from
//!   pre-pagination: a bare compact array. Back-compat for existing clients.
//! - If the caller passes ANY pagination arg, the response becomes an envelope:
//!
//!   ```json
//!   {
//!     "items": [...],
//!     "pagination": {
//!       "style": "page" | "cursor" | "start_id" | "body",
//!       "page": 0,                       // page style only
//!       "last_page": false,              // page style only
//!       "next_cursor": "...",            // cursor style only, omitted when exhausted
//!       "next_start": 1700000000,        // start_id style only, omitted when exhausted
//!       "next_start_id": "...",          // start_id style only, omitted when exhausted
//!       "next_page_timestamp": 1700000,  // body style only, omitted when exhausted
//!       "page_direction": "NEXT",        // body style only, echoes caller input
//!       "has_more": true,
//!       "returned": 42,
//!       "all": false
//!     }
//!   }
//!   ```
//!
//! Calling code uses [`PageArgs::from_args`] / [`CursorArgs::from_args`] /
//! [`StartIdArgs::from_args`] / [`BodyPaginationArgs::from_args`] to parse
//! pagination input, then [`page_dispatch`] / [`cursor_dispatch`] /
//! [`start_id_dispatch`] / [`body_pagination_dispatch`] to run the fetch loop.

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

/// Parsed start-id-based pagination input. ClickUp's v2 comment endpoints
/// (`/v2/task/{id}/comment`, `/v2/list/{id}/comment`, `/v2/view/{id}/comment`,
/// `/v2/comment/{id}/reply`) use this style: pass `?start=<unix_ms>&start_id=<id>`
/// to retrieve items older than that boundary. Both params are required as a
/// pair when paginating; omitting them returns the first page (newest first).
/// The response is a bare `{ "comments": [...] }` array — no pagination
/// metadata — so termination is inferred when the returned array is shorter
/// than the endpoint's page size (25 for ClickUp's comment endpoints).
#[derive(Debug, Clone, Default)]
pub struct StartIdArgs {
    pub start: Option<i64>,
    pub start_id: Option<String>,
    pub limit: Option<usize>,
    pub all: bool,
    pub requested: bool,
}

impl StartIdArgs {
    pub fn from_args(args: &Value) -> Self {
        let start = args.get("start").and_then(|v| v.as_i64());
        let start_id = args
            .get("start_id")
            .and_then(|v| v.as_str())
            .map(String::from);
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        let all = args.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
        let requested =
            start.is_some() || start_id.is_some() || limit.is_some() || args.get("all").is_some();
        Self {
            start,
            start_id,
            limit,
            all,
            requested,
        }
    }
}

/// Parsed body-pagination input. ClickUp's v3 audit-log endpoint
/// (`POST /v3/workspaces/{ws}/auditlogs`) puts pagination state inside the
/// request **body** as `pagination: { pageRows, pageTimestamp, pageDirection }`,
/// not in query params. `pageDirection` is `"NEXT"` or `"PREVIOUS"` —
/// `--all` walks in whatever direction the caller passes, defaulting to
/// `"NEXT"` (newer events) if unspecified.
#[derive(Debug, Clone, Default)]
pub struct BodyPaginationArgs {
    pub page_rows: Option<i64>,
    pub page_timestamp: Option<i64>,
    pub page_direction: Option<String>,
    pub limit: Option<usize>,
    pub all: bool,
    pub requested: bool,
}

impl BodyPaginationArgs {
    pub fn from_args(args: &Value) -> Self {
        let page_rows = args.get("page_rows").and_then(|v| v.as_i64());
        let page_timestamp = args.get("page_timestamp").and_then(|v| v.as_i64());
        let page_direction = args
            .get("page_direction")
            .and_then(|v| v.as_str())
            .map(String::from);
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        let all = args.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
        let requested = page_rows.is_some()
            || page_timestamp.is_some()
            || page_direction.is_some()
            || limit.is_some()
            || args.get("all").is_some();
        Self {
            page_rows,
            page_timestamp,
            page_direction,
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
    // `has_more` reports whether the SERVER has additional pages — purely a
    // function of `last_page`. Don't conjoin with limit-truncation: when the
    // caller passes `limit` and we hit the cap, the server may still have
    // more pages they could retrieve via a higher limit or a subsequent
    // page= request. Telling them has_more=false in that case is misleading.
    let has_more = !last_page;
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
    // `has_more` reports whether the SERVER has additional pages — purely a
    // function of `next_cursor`. Don't conjoin with limit-truncation: see
    // the matching comment in `page_dispatch`.
    let has_more = next_cursor.is_some();

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

/// Default page size used by ClickUp's v2 comment endpoints. The endpoints
/// don't expose a `last_page` flag and don't accept a custom page size, so
/// callers infer "more results exist" by checking if the returned array is
/// shorter than this value. The 25 figure comes from ClickUp's published
/// API docs; if their server-side default ever changes the helper's only
/// failure mode is "stops one page too early when `all=true`" — survivable.
const START_ID_PAGE_HINT: usize = 25;

/// Run a start-id-based pagination loop. `build_path(start, start_id)` should
/// return a path including the `?start=...&start_id=...` query when both are
/// Some, or no pagination query when both are None. (ClickUp requires the
/// params as a pair — passing only one is a caller bug.) `items_key` is the
/// response array's field name (`"comments"` for both comment endpoints).
///
/// The next-page boundary is derived from the LAST item in the returned
/// array: its `date` field (a stringified unix-ms timestamp) becomes the
/// next `start`, and its `id` field becomes the next `start_id`. Termination
/// happens when the array is empty OR shorter than `START_ID_PAGE_HINT`.
pub async fn start_id_dispatch<F>(
    args: &StartIdArgs,
    client: &ClickUpClient,
    items_key: &str,
    compact_fields: &[&str],
    build_path: F,
) -> Result<Value, String>
where
    F: Fn(Option<i64>, Option<&str>) -> String,
{
    let mut current_start = args.start;
    let mut current_start_id = args.start_id.clone();
    let mut collected: Vec<Value> = Vec::new();
    // The boundary for the FOLLOWING page after the most recent fetch. Holds
    // the (date_ms, comment_id) extracted from the last item in the response.
    // The initial `None` is overwritten on the first iteration; the variable
    // survives the loop so the value feeds the pagination envelope.
    #[allow(unused_assignments)]
    let mut next_boundary: Option<(i64, String)> = None;
    // Whether the loop terminated because we reached the natural end (empty
    // response or short page). False when we stopped due to limit/MAX_PAGES
    // and there might still be more on the server. Drives `has_more`.
    let mut reached_end = false;
    let mut pages_fetched = 0usize;

    loop {
        let path = build_path(current_start, current_start_id.as_deref());
        let resp = client.get(&path).await.map_err(|e| e.to_string())?;
        let items = extract_array(&resp, &[items_key, "data"]).unwrap_or_default();
        let count = items.len();

        // Extract next-page boundary from the last item before consuming the
        // vec. ClickUp returns `date` as a stringified ms integer.
        if let Some(last) = items.last() {
            let date_ms = last
                .get("date")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<i64>().ok())
                .or_else(|| last.get("date").and_then(|v| v.as_i64()));
            let id = last.get("id").and_then(|v| v.as_str()).map(String::from);
            next_boundary = match (date_ms, id) {
                (Some(d), Some(i)) => Some((d, i)),
                _ => None,
            };
        } else {
            next_boundary = None;
        }

        collected.extend(items);
        pages_fetched += 1;

        // A short or empty page means the server has no more results — record
        // that so the pagination envelope can report `has_more: false` even
        // though we still have a boundary from the last item we did receive.
        if count < START_ID_PAGE_HINT {
            reached_end = true;
        }

        if !args.all {
            break;
        }
        if reached_end || pages_fetched >= MAX_PAGES {
            break;
        }
        if let Some(limit) = args.limit {
            if collected.len() >= limit {
                break;
            }
        }

        // Advance to next page. If we somehow have no boundary (e.g. last item
        // was missing date/id), bail to avoid an infinite loop with the same
        // start.
        match next_boundary.clone() {
            Some((d, i)) => {
                current_start = Some(d);
                current_start_id = Some(i);
            }
            None => break,
        }
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
    // `has_more` reports whether the SERVER has additional pages — purely a
    // function of whether we reached a short page AND we still have a usable
    // boundary. Don't conjoin with limit-truncation: see the matching
    // comment in `page_dispatch`.
    let has_more = !reached_end && next_boundary.is_some();

    let mut pagination = serde_json::Map::new();
    pagination.insert("style".into(), json!("start_id"));
    pagination.insert("has_more".into(), json!(has_more));
    pagination.insert("returned".into(), json!(returned));
    pagination.insert("all".into(), json!(args.all));
    if let Some((d, i)) = next_boundary {
        pagination.insert("next_start".into(), json!(d));
        pagination.insert("next_start_id".into(), json!(i));
    }
    Ok(json!({
        "items": compact_arr,
        "pagination": Value::Object(pagination),
    }))
}

/// Run a body-pagination POST loop for endpoints like the v3 audit-log query.
/// Pagination state lives inside the request body as
/// `pagination: { pageRows, pageTimestamp, pageDirection }`.
///
/// Caller responsibilities:
/// - `path`: POST target, fixed across all iterations.
/// - `items_keys`: candidate response keys for the items array, in priority
///   order (e.g. `&["data"]` for v3 audit-log).
/// - `compact_fields`: fields to project per item via `compact_items`.
/// - `base_body`: closure returning a fresh non-pagination body each call.
///   The helper merges the `pagination` block into it.
/// - `next_timestamp`: closure that takes the last item of the current
///   response and returns the next request's `pageTimestamp` (typically the
///   event's own timestamp field). `None` means "no further boundary
///   available" — the loop ends.
///
/// Termination: empty response, `next_timestamp` returns None, `limit` cap
/// reached, or `MAX_PAGES` reached.
pub async fn body_pagination_dispatch<BB, NT>(
    args: &BodyPaginationArgs,
    client: &ClickUpClient,
    path: &str,
    items_keys: &[&str],
    compact_fields: &[&str],
    base_body: BB,
    next_timestamp: NT,
) -> Result<Value, String>
where
    BB: Fn() -> Value,
    NT: Fn(&Value) -> Option<i64>,
{
    let mut current_timestamp = args.page_timestamp;
    let mut collected: Vec<Value> = Vec::new();
    // Whether the loop terminated because we ran out of server-side results
    // (empty response or no next boundary). Drives `has_more`.
    let mut reached_end = false;
    // The next-page boundary surfaced in the envelope when caller passed
    // pagination args. Initial None is overwritten on the first iteration.
    #[allow(unused_assignments)]
    let mut next_boundary: Option<i64> = None;
    let mut pages_fetched = 0usize;

    loop {
        // Build the body fresh each iteration: the base body, plus a
        // pagination block reflecting current state. We rebuild on every
        // iteration rather than mutating in place so the base_body closure
        // is the single source of truth for the non-pagination fields.
        let mut body = base_body();
        let mut pagination = serde_json::Map::new();
        if let Some(n) = args.page_rows {
            pagination.insert("pageRows".into(), json!(n));
        }
        if let Some(t) = current_timestamp {
            pagination.insert("pageTimestamp".into(), json!(t));
        }
        if let Some(d) = args.page_direction.as_deref() {
            pagination.insert("pageDirection".into(), json!(d));
        }
        if !pagination.is_empty() {
            body["pagination"] = Value::Object(pagination);
        }

        let resp = client.post(path, &body).await.map_err(|e| e.to_string())?;
        let items = extract_array(&resp, items_keys).unwrap_or_default();
        let count = items.len();

        // Derive next-page boundary from the last item BEFORE consuming it
        // into the collected vec.
        next_boundary = items.last().and_then(&next_timestamp);

        collected.extend(items);
        pages_fetched += 1;

        if count == 0 {
            reached_end = true;
        }

        if !args.all {
            break;
        }
        if reached_end || pages_fetched >= MAX_PAGES {
            break;
        }
        if let Some(limit) = args.limit {
            if collected.len() >= limit {
                break;
            }
        }

        // Advance: walk in caller's chosen direction by updating
        // `pageTimestamp` from the boundary we just extracted. If the
        // extractor returned None, bail to avoid an infinite loop.
        match next_boundary {
            Some(t) => current_timestamp = Some(t),
            None => {
                reached_end = true;
                break;
            }
        }
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
    let has_more = !reached_end && next_boundary.is_some();

    let mut pagination = serde_json::Map::new();
    pagination.insert("style".into(), json!("body"));
    pagination.insert("has_more".into(), json!(has_more));
    pagination.insert("returned".into(), json!(returned));
    pagination.insert("all".into(), json!(args.all));
    if let Some(t) = next_boundary {
        pagination.insert("next_page_timestamp".into(), json!(t));
    }
    if let Some(d) = args.page_direction.as_deref() {
        pagination.insert("page_direction".into(), json!(d));
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
    use wiremock::matchers::{method, path, query_param, query_param_is_missing};
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

    #[tokio::test]
    async fn start_id_dispatch_no_pagination_args_returns_bare_array() {
        // Caller passes no start/start_id/limit/all -> bare array, matching
        // the existing pre-pagination shape for comment list/replies.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/task/T1/comment"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "comments": [
                    {"id": "c1", "date": "1700000000000", "comment_text": "a"},
                    {"id": "c2", "date": "1700000005000", "comment_text": "b"},
                ],
            })))
            .mount(&server)
            .await;
        let client = test_client(&server);
        let args = StartIdArgs::from_args(&json!({}));
        let result = start_id_dispatch(
            &args,
            &client,
            "comments",
            &["id", "comment_text"],
            |start, start_id| match (start, start_id) {
                (Some(s), Some(i)) => format!("/v2/task/T1/comment?start={}&start_id={}", s, i),
                _ => "/v2/task/T1/comment".to_string(),
            },
        )
        .await
        .unwrap();
        assert!(result.is_array(), "expected bare array, got {}", result);
        assert_eq!(result.as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn start_id_dispatch_all_true_walks_pages_via_last_item_boundary() {
        // First page: 25 items (the page-size hint) -> caller knows to keep
        // walking. Helper derives boundary from last item's date+id and
        // requests the next page. Second page: 2 items (< 25) -> termination.
        let server = MockServer::start().await;

        let mut first_page = Vec::new();
        for i in 0..25 {
            first_page.push(json!({
                "id": format!("c{}", i),
                "date": format!("{}", 1_700_000_000_000_u64 + (i as u64) * 1000),
                "comment_text": format!("comment {}", i),
            }));
        }
        // The last item in page 1 will be the boundary for page 2:
        //   start = 1700000024000 (date of c24), start_id = "c24"
        let last_first = &first_page[24];
        let boundary_date = last_first["date"].as_str().unwrap();
        let boundary_id = last_first["id"].as_str().unwrap();

        Mock::given(method("GET"))
            .and(path("/v2/task/T1/comment"))
            // First call has no start/start_id query.
            .and(query_param_is_missing("start"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "comments": first_page,
            })))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/task/T1/comment"))
            .and(query_param("start", boundary_date))
            .and(query_param("start_id", boundary_id))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "comments": [
                    {"id": "c25", "date": "1700000025000"},
                    {"id": "c26", "date": "1700000026000"},
                ],
            })))
            .mount(&server)
            .await;

        let client = test_client(&server);
        let args = StartIdArgs::from_args(&json!({"all": true}));
        let result = start_id_dispatch(
            &args,
            &client,
            "comments",
            &["id"],
            |start, start_id| match (start, start_id) {
                (Some(s), Some(i)) => format!("/v2/task/T1/comment?start={}&start_id={}", s, i),
                _ => "/v2/task/T1/comment".to_string(),
            },
        )
        .await
        .unwrap();
        let items = result.get("items").and_then(|v| v.as_array()).unwrap();
        assert_eq!(items.len(), 27, "expected 25 + 2 across 2 pages");
        let p = result.get("pagination").unwrap();
        assert_eq!(p.get("style").and_then(|v| v.as_str()), Some("start_id"));
        // Has_more should be false: 2nd page returned < page-size hint.
        assert_eq!(p.get("has_more").and_then(|v| v.as_bool()), Some(false));
        // next_start / next_start_id reflect the LAST seen item (c26),
        // because boundary extraction runs on every fetch.
        assert_eq!(p.get("next_start_id").and_then(|v| v.as_str()), Some("c26"));
    }

    // ---- Regression tests for the limit-truncation `has_more` bug ----
    //
    // Caught by live smoke-testing PR #44 against ClickUp: when the caller
    // passes `limit` and the helper truncates a non-terminal page, has_more
    // was reported as false (because the original logic conjoined "server
    // has more" with "we didn't hit the cap"). That misleads users into
    // thinking they got everything when the server still has pages.
    // has_more must report ONLY whether the server has additional pages.

    #[tokio::test]
    async fn page_dispatch_limit_truncates_but_has_more_reflects_server() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/list/L1/task"))
            .and(query_param("page", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "tasks": [{"id": "t1"}, {"id": "t2"}, {"id": "t3"}],
                "last_page": false,
            })))
            .mount(&server)
            .await;
        let client = test_client(&server);
        // limit=2 truncates a 3-item response from a page that ISN'T the last.
        let args = PageArgs::from_args(&json!({"limit": 2}));
        let result = page_dispatch(&args, &client, "tasks", &["id"], |p| {
            format!("/v2/list/L1/task?page={}", p)
        })
        .await
        .unwrap();
        let p = result.get("pagination").unwrap();
        assert_eq!(
            p.get("has_more").and_then(|v| v.as_bool()),
            Some(true),
            "limit-truncated page with last_page=false should report has_more=true"
        );
    }

    #[tokio::test]
    async fn cursor_dispatch_limit_truncates_but_has_more_reflects_server() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v3/workspaces/2648001/chat/channels"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{"id": "c1"}, {"id": "c2"}, {"id": "c3"}],
                "next_cursor": "MORE",
            })))
            .mount(&server)
            .await;
        let client = test_client(&server);
        let args = CursorArgs::from_args(&json!({"limit": 2}));
        let result = cursor_dispatch(&args, &client, &["data"], &["id"], |c| match c {
            Some(c) => format!("/v3/workspaces/2648001/chat/channels?cursor={}", c),
            None => "/v3/workspaces/2648001/chat/channels".to_string(),
        })
        .await
        .unwrap();
        let p = result.get("pagination").unwrap();
        assert_eq!(
            p.get("has_more").and_then(|v| v.as_bool()),
            Some(true),
            "limit-truncated page with non-empty next_cursor should report has_more=true"
        );
        assert_eq!(
            p.get("next_cursor").and_then(|v| v.as_str()),
            Some("MORE"),
            "next_cursor must still be exposed so caller can fetch more if they want"
        );
    }

    #[tokio::test]
    async fn start_id_dispatch_limit_truncates_but_has_more_reflects_server() {
        // Server returns a FULL page (25 items, matches the page-size hint),
        // so reached_end stays false. With limit=10 we truncate to 10. The
        // helper should still report has_more=true because the next page
        // would have more items.
        let server = MockServer::start().await;
        let mut page = Vec::new();
        for i in 0..25 {
            page.push(json!({
                "id": format!("c{}", i),
                "date": format!("{}", 1_700_000_000_000_u64 + (i as u64) * 1000),
            }));
        }
        Mock::given(method("GET"))
            .and(path("/v2/task/T1/comment"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "comments": page,
            })))
            .mount(&server)
            .await;
        let client = test_client(&server);
        let args = StartIdArgs::from_args(&json!({"limit": 10}));
        let result = start_id_dispatch(
            &args,
            &client,
            "comments",
            &["id"],
            |start, start_id| match (start, start_id) {
                (Some(s), Some(i)) => format!("/v2/task/T1/comment?start={}&start_id={}", s, i),
                _ => "/v2/task/T1/comment".to_string(),
            },
        )
        .await
        .unwrap();
        let items = result.get("items").and_then(|v| v.as_array()).unwrap();
        assert_eq!(items.len(), 10, "limit cap honoured");
        let p = result.get("pagination").unwrap();
        assert_eq!(
            p.get("has_more").and_then(|v| v.as_bool()),
            Some(true),
            "limit-truncated full page should report has_more=true (server has more)"
        );
        // next_start / next_start_id should still be the last item of the
        // UNTRUNCATED response (c24, the 25th item), so the caller can
        // continue from where the server left off, not from where we cut.
        assert_eq!(p.get("next_start_id").and_then(|v| v.as_str()), Some("c24"));
    }

    // ---- body_pagination_dispatch tests ----

    fn audit_event(id: &str, ts: i64) -> Value {
        json!({"id": id, "eventTime": ts, "eventType": "auth"})
    }

    #[tokio::test]
    async fn body_dispatch_no_pagination_args_returns_bare_array() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v3/workspaces/W1/auditlogs"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [audit_event("e1", 1700000000), audit_event("e2", 1700000005)],
            })))
            .mount(&server)
            .await;
        let client = test_client(&server);
        let args = BodyPaginationArgs::from_args(&json!({}));
        let result = body_pagination_dispatch(
            &args,
            &client,
            "/v3/workspaces/W1/auditlogs",
            &["data"],
            &["id", "eventTime"],
            || json!({"applicability": "AUTH"}),
            |item| item.get("eventTime").and_then(|v| v.as_i64()),
        )
        .await
        .unwrap();
        assert!(result.is_array(), "expected bare array, got {}", result);
        assert_eq!(result.as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn body_dispatch_all_true_walks_pages_via_timestamp_boundary() {
        // First page: 3 events. Boundary = last event's eventTime (1700000020).
        // Second page (with pageTimestamp=1700000020): 2 events. Boundary moves.
        // Third page (with pageTimestamp=1700000035): empty -> stop.
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v3/workspaces/W1/auditlogs"))
            .and(wiremock::matchers::body_partial_json(
                json!({"applicability": "AUTH"}),
            ))
            .and(wiremock::matchers::body_partial_json(
                json!({"pagination": {"pageTimestamp": 1700000020_i64}}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [
                    audit_event("e4", 1700000030),
                    audit_event("e5", 1700000035),
                ],
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v3/workspaces/W1/auditlogs"))
            .and(wiremock::matchers::body_partial_json(
                json!({"pagination": {"pageTimestamp": 1700000035_i64}}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [],
            })))
            .mount(&server)
            .await;
        // First call has no pageTimestamp in body; lowest priority match so
        // the more-specific matches above are tried first.
        Mock::given(method("POST"))
            .and(path("/v3/workspaces/W1/auditlogs"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [
                    audit_event("e1", 1700000010),
                    audit_event("e2", 1700000015),
                    audit_event("e3", 1700000020),
                ],
            })))
            .mount(&server)
            .await;

        let client = test_client(&server);
        let args = BodyPaginationArgs::from_args(&json!({"all": true}));
        let result = body_pagination_dispatch(
            &args,
            &client,
            "/v3/workspaces/W1/auditlogs",
            &["data"],
            &["id"],
            || json!({"applicability": "AUTH"}),
            |item| item.get("eventTime").and_then(|v| v.as_i64()),
        )
        .await
        .unwrap();
        let items = result.get("items").and_then(|v| v.as_array()).unwrap();
        assert_eq!(items.len(), 5, "expected 3 + 2 + 0 across 3 pages");
        let p = result.get("pagination").unwrap();
        assert_eq!(p.get("style").and_then(|v| v.as_str()), Some("body"));
        assert_eq!(
            p.get("has_more").and_then(|v| v.as_bool()),
            Some(false),
            "reached natural end (empty page) -> has_more false"
        );
    }

    #[tokio::test]
    async fn body_dispatch_limit_truncates_but_has_more_reflects_server() {
        // Server returns a non-empty response and a valid next-boundary;
        // limit truncates. has_more must still be true.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v3/workspaces/W1/auditlogs"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [
                    audit_event("e1", 1700000010),
                    audit_event("e2", 1700000015),
                    audit_event("e3", 1700000020),
                ],
            })))
            .mount(&server)
            .await;
        let client = test_client(&server);
        let args = BodyPaginationArgs::from_args(&json!({"limit": 2}));
        let result = body_pagination_dispatch(
            &args,
            &client,
            "/v3/workspaces/W1/auditlogs",
            &["data"],
            &["id"],
            || json!({"applicability": "AUTH"}),
            |item| item.get("eventTime").and_then(|v| v.as_i64()),
        )
        .await
        .unwrap();
        let items = result.get("items").and_then(|v| v.as_array()).unwrap();
        assert_eq!(items.len(), 2, "limit cap honoured");
        let p = result.get("pagination").unwrap();
        assert_eq!(
            p.get("has_more").and_then(|v| v.as_bool()),
            Some(true),
            "limit-truncated non-empty page should report has_more=true"
        );
        // next_page_timestamp should be the last item's timestamp from the
        // UNTRUNCATED response (e3 = 1700000020), so the caller can continue
        // from where the server left off.
        assert_eq!(
            p.get("next_page_timestamp").and_then(|v| v.as_i64()),
            Some(1700000020)
        );
    }

    #[test]
    fn body_pagination_args_empty() {
        let a = BodyPaginationArgs::from_args(&json!({}));
        assert!(!a.requested);
        assert!(a.page_rows.is_none());
        assert!(a.page_timestamp.is_none());
        assert!(a.page_direction.is_none());
        assert!(a.limit.is_none());
        assert!(!a.all);
    }

    #[test]
    fn body_pagination_args_full() {
        let a = BodyPaginationArgs::from_args(&json!({
            "page_rows": 100,
            "page_timestamp": 1700000000_i64,
            "page_direction": "PREVIOUS",
            "limit": 50,
            "all": true,
        }));
        assert!(a.requested);
        assert_eq!(a.page_rows, Some(100));
        assert_eq!(a.page_timestamp, Some(1700000000));
        assert_eq!(a.page_direction.as_deref(), Some("PREVIOUS"));
        assert_eq!(a.limit, Some(50));
        assert!(a.all);
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
    fn start_id_args_empty() {
        let s = StartIdArgs::from_args(&json!({}));
        assert!(!s.requested);
        assert!(s.start.is_none());
        assert!(s.start_id.is_none());
        assert_eq!(s.limit, None);
        assert!(!s.all);
    }

    #[test]
    fn start_id_args_full() {
        let s = StartIdArgs::from_args(
            &json!({"start": 1700000000000_i64, "start_id": "c1", "limit": 20, "all": true}),
        );
        assert!(s.requested);
        assert_eq!(s.start, Some(1700000000000));
        assert_eq!(s.start_id.as_deref(), Some("c1"));
        assert_eq!(s.limit, Some(20));
        assert!(s.all);
    }

    #[test]
    fn start_id_args_partial_start_only_still_requested() {
        // Passing only `start` (no `start_id`) is technically invalid as a
        // ClickUp request, but should still register as `requested` so the
        // helper switches to the envelope shape and the caller's URL-builder
        // closure can validate / error out cleanly.
        let s = StartIdArgs::from_args(&json!({"start": 1700000000000_i64}));
        assert!(s.requested);
        assert_eq!(s.start, Some(1700000000000));
        assert!(s.start_id.is_none());
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
