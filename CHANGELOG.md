# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `doc embed-image` CLI command and `clickup_doc_embed_image` MCP tool (144 tools): upload a local image and embed it inline in a ClickUp doc page. The ClickUp API has no doc-level upload endpoint (the v3 attachment endpoint rejects `docs`/`pages` entity types), so the image is stored as an attachment on a host task (`--via-task`, auto-detected from the git branch like other task-scoped commands), and the returned public CDN URL is appended/prepended to the page as `![alt](url)` markdown, which ClickUp converts into a native inline image block.

## [0.13.0] - 2026-05-22

### Changed
- `.clickup.toml` resolution now walks up from the current directory to filesystem root and uses the nearest config file, mirroring how git locates `.git` (#64). Previously the CLI only checked the current directory, so running from any subdirectory of a project silently fell back to the global `~/.config/clickup-cli/config.toml`, which commonly surfaced as a `401 Team not authorized` when the global token belonged to a different workspace. `setup` and `agent-config init` still write to the current directory — only the read path changed.

## [0.12.1] - 2026-05-20

### Fixed
- npm publish pipeline migrated to OIDC-based Trusted Publishing (#63). v0.12.0's npm/Homebrew/AUR/GitHub Packages publish chain failed because npm rotated all granular access tokens with write access that bypassed 2FA following the "Mini Shai-Hulud" supply-chain attacks — the `NPM_TOKEN` repo secret was invalidated mid-release. v0.12.1 has the same code as v0.12.0 but re-publishes via the new TP flow so all distribution surfaces reach 0.12.x. No `NPM_TOKEN` secret is involved any more; the workflow exchanges GitHub's per-run OIDC token for a short-lived publish credential at publish time.

## [0.12.0] - 2026-05-20

### Added
- MCP pagination support across the 15 tools whose underlying ClickUp endpoint is paginated (#14, #48, #49, #51). Page-based (v2): `clickup_task_list`, `clickup_task_search`, `clickup_view_tasks`, `clickup_template_list` accept optional `page` / `limit` / `all`. Cursor-based (v3): `clickup_doc_list`, `clickup_chat_channel_list`, `clickup_chat_message_list`, `clickup_chat_reply_list`, `clickup_chat_channel_followers`, `clickup_chat_channel_members`, `clickup_chat_reaction_list`, `clickup_chat_tagged_users` accept optional `cursor` / `limit` / `all`. Start-id-based (v2 comments): `clickup_comment_list`, `clickup_comment_replies` accept optional `start` / `start_id` / `limit` / `all`. Body-based (v3 audit log): `clickup_audit_log_query` accepts the existing `page_rows` / `page_timestamp` / `page_direction` plus new `limit` / `all`. The contract is **opt-in and non-breaking**: when no pagination arg is passed, the response is unchanged — a bare compact array, same shape existing MCP clients see today. When ANY pagination arg is passed, the response becomes `{"items": [...], "pagination": {style, page|next_cursor|next_start+next_start_id|next_page_timestamp, has_more, returned, last_page, all}}`. With `all=true` the helper walks pages until the natural termination signal (ClickUp's `last_page=true` for page-based, `next_cursor=null` for cursor-based, page shorter than 25 items for start-id-based, empty response for body-based) or `limit` is reached (hard-capped at 100 pages to prevent runaway loops). Introduces `crate::mcp::pagination` with `PageArgs`, `CursorArgs`, `StartIdArgs`, `BodyPaginationArgs`, `page_dispatch`, `cursor_dispatch`, `start_id_dispatch`, and `body_pagination_dispatch` helpers backed by wiremock-driven tests.

### Changed
- `clickup_chat_channel_followers`, `clickup_chat_channel_members`, `clickup_chat_reaction_list`, `clickup_chat_tagged_users` MCP tools now return **compact** response objects (matching every other chat list tool). Previously these four returned the raw API response object verbatim with all per-item fields (initials, profilePicture, color, etc.); now they return only the documented compact field set per item: members/followers/tagged_users → `id, name, username, email`; reactions → `reaction, user, date`. Callers that depended on `initials`, `profilePicture`, or `color` will see those fields absent. The shape narrowing is unconditional — applies whether or not pagination args are passed.

### Fixed
- CLI commands that call paginated ClickUp endpoints now honour the global `--all` / `--page` / `--limit` flags consistently (#50). Previously only `task list` and `task search` actually paginated; every other paginated list command silently returned just the first page or applied `--limit` as a client-side cap on the first page only — misleading users into thinking they had all results. 13 CLI commands fixed: `view tasks`, `template list` (page-based v2); `doc list`, `chat channel-list`, `chat channel-followers`, `chat channel-members`, `chat message-list`, `chat reaction-list`, `chat reply-list`, `chat tagged-users` (cursor-based v3); `comment list`, `comment replies` (start-id-based v2); `audit-log query` (body-based v3). New global flags `--cursor` (cursor pagination continuation), `--start` + `--start-id` (start-id pagination continuation pair) accompany the existing `--page`. The CLI now uses a shared `crate::commands::pagination` module that mirrors the MCP-side helpers from #14/#48/#49/#51; the shared module handles loop termination, MAX_PAGES guard (100), and the cli-flag → walker plumbing. `--limit` is now correctly enforced AFTER walking (so `--all --limit 500` returns up to 500 items across N pages, not "first page truncated to 500"). Breaking change for two subcommands: `clickup-cli view tasks` and `clickup-cli template list` no longer accept their command-local `--page` arg (the global one supersedes); callers passing `--page 0` continue to work unchanged.

## [0.11.0] - 2026-05-19

### Changed
- **BREAKING — binary rename (#39):** the shipped binary is no longer named `clickup`. The previous name collided with the official ClickUp desktop app on Linux, which installs a `clickup` binary on `PATH`. From 0.11.0 onwards two binaries ship from the same code:
  - **`clickup-cli`** — the canonical name. Matches the crate name (`cargo install clickup-cli`) and the AUR package (`clickup-cli-bin`). All documentation and help text reference this name.
  - **`clkup`** — a short alias for daily ergonomics (5 chars, no hyphen, no collision with the desktop app). Identical behaviour to `clickup-cli`.
  - The previous `clickup` binary is **removed**, not aliased. Existing users must migrate.

  Migration:
  - **MCP configs** (Claude Desktop, Cursor, Codex, etc.): replace `"command": "clickup"` with `"command": "clickup-cli"` (or `"clkup"`) in `claude_desktop_config.json` / `.mcp.json` / equivalent.
  - **Shell aliases & scripts**: replace `clickup ` invocations with `clickup-cli ` (or `clkup `). A one-shot rewrite: `sed -i 's/\bclickup /clickup-cli /g' your-script.sh` (review the diff — don't blindly replace if you reference the ClickUp brand name in prose).
  - **Shell completions**: regenerate via `clickup-cli completions <shell> > /path/to/completion`. Old completion files keyed off the `clickup` binary name still source-load but won't fire on the new binaries until regenerated.
  - **Injected agent-config blocks** (`clickup agent-config inject`): re-run `clickup-cli agent-config inject` to refresh the CLI reference baked into your CLAUDE.md / .cursorrules / equivalent. The injection markers (`<!-- clickup-cli:begin -->` / `<!-- clickup-cli:end -->`) are unchanged, so the re-inject is a clean in-place update.

  Why a hard break: keeping `clickup` alongside `clickup-cli` would defeat the rename — the collision with the desktop app would persist. Pre-1.0 semver allows the break.
- **BREAKING:** `audit-log query` (CLI and `clickup_audit_log_query` MCP tool) request body reshaped to match ClickUp's v3 OpenAPI spec. Previous implementation invented `{type, user_id, date_filter:{start_date,end_date}}`, none of which the endpoint recognises. Correct shape is `{applicability, filter?:{...}, pagination?:{...}}` where the inner filter fields are `eventType`, `eventStatus`, `userId` (array), `userEmail` (array), `startTime`, `endTime`. CLI flags renamed: `--type` is gone, replaced by `--applicability` (required) plus optional `--event-type`. `--user-id` is now repeatable. New flags: `--event-status`, `--user-email`, `--start-time`, `--end-time`, `--page-rows`, `--page-timestamp`, `--page-direction`. No working caller exists because the previous body shape produced no useful response.
- **BREAKING:** `acl update` (CLI and `clickup_acl_update` MCP tool) body reshaped to match the v3 spec. Previous implementation invented `{access_type, grant, revoke}` arrays; the endpoint accepts `{private?:bool, entries?:[{kind, id, permission_level?}]}`. CLI flags now: `--private true|false`, `--grant-user USER_ID[:LEVEL]` (repeatable), `--grant-group GROUP_ID[:LEVEL]` (repeatable), `--revoke-user USER_ID` (repeatable), `--revoke-group GROUP_ID` (repeatable). Permission level accepts `read|comment|edit|create` (mapped to spec's `1|3|4|5` integers). `--body` raw JSON escape hatch retained. MCP gains an `entries` array parameter (objects of `{kind, id, permission_level?}` with kind enum `user|group`) so MCP callers can grant/revoke access — previously only `private` was exposed on the MCP side. Same justification as audit-log.

### Fixed
- `chat channel-list`, `chat message-list`, and `chat reply-list` CLI commands now read the v3 `data` envelope first and fall back to the older `channels`/`messages`/`replies` keys for compatibility (#39). Before this fix the commands consistently returned empty results because the v3 endpoints return their list under `data` — the matching MCP tools were already fixed in 0.10.0 but the CLI parity was missed.

## [0.10.0] - 2026-05-18

### Changed
- **BREAKING:** `task replace-estimates` (CLI and `clickup_task_replace_estimates` MCP tool) reworked to match ClickUp's spec and remove a data-loss footgun. The old shape `{time_estimates: [{user_id, time_estimate}]}` was wrong on three fronts: the body should be a bare array (no wrapper), the field names are `assignee` and `time` (not `user_id` / `time_estimate`), and accepting only one assignee meant the "replace" operation silently erased every other user's estimate. CLI: `--assignee` / `--time` removed, replaced by repeatable `--estimate ASSIGNEE:MS` (ASSIGNEE accepts a numeric user id or the literal `unassigned`); new `--body` raw JSON escape hatch. MCP: `user_id` / `time_estimate` scalars replaced by an `estimates` array of `{assignee, time}` objects.
- **BREAKING:** `time rename-tag` (CLI and `clickup_time_rename_tag` MCP tool) now sends the required `tag_bg` and `tag_fg` hex-colour fields per ClickUp's spec. The endpoint marks them required even when the caller only wants to rename; previously omitting them likely 400'd or silently failed. CLI gains required `--tag-bg` and `--tag-fg` flags; MCP gains required `tag_bg` and `tag_fg` schema parameters. Callers who want to keep the existing colours should pass the current hex values.
- CLI help and MCP tool descriptions corrected after an audit against ClickUp's official OpenAPI spec. No behaviour change; documentation only.
  - `comment create`, `comment update`, `comment reply` (and the corresponding `clickup_comment_*` MCP tools) no longer claim markdown support. ClickUp's v2 comment API only accepts plain `comment_text` and renders neither markdown nor rich text; markdown syntax is stored verbatim. @mentions are still rendered.
  - `clickup_doc_create` MCP `parent.type` cheat sheet corrected: the enum is 4=space, 5=folder, 6=list, 7=everything, 12=workspace. The previous text said 7=task, which is wrong.
  - `clickup_webhook_delete` MCP description: the alternative-to-delete suggestion now points at `status='inactive'`. The previous text said `'suspended'`, which is not a value the API accepts.
  - `clickup_audit_log_query` MCP `type` parameter description: corrected example enum values to ClickUp's actual categories (AUTH, HIERARCHY, USER, CUSTOM_FIELDS, AGENT, OTHER). The previous text invented `task_created`, `user_added`, `permission_changed`.
  - `clickup_chat_channel_update` MCP `description` field no longer claims markdown support; ClickUp's chat-channel description field is plain text.

### Fixed
- MCP task-scoped tools now auto-detect custom task IDs (e.g. `PROJ-42`) and inject the required `?custom_task_ids=true&team_id=<ws>` query string. Affects `clickup_task_get`, `clickup_task_update`, `clickup_task_delete`, `clickup_task_add_tag`, `clickup_task_remove_tag`, `clickup_task_add_dep`, `clickup_task_remove_dep`, `clickup_task_link`, `clickup_task_unlink`, `clickup_field_set`, `clickup_field_unset`, `clickup_task_time_in_status`, `clickup_attachment_list`, `clickup_attachment_upload`, `clickup_comment_list`, `clickup_comment_create`, `clickup_checklist_create`. Previously the CLI commands handled custom IDs but the MCP equivalents always treated the value as an internal ClickUp ID and 404'd for custom-format IDs. The `CU-` prefix is also now transparently stripped on the MCP side, matching CLI behaviour. Schema parameter names are unchanged; detection is automatic based on ID format.
- Chat v3 fixes after auditing against ClickUp's OpenAPI spec:
  - `clickup_chat_message_send` MCP tool now sends the required `type` field (default `"message"`, configurable via a new optional `type` schema arg). ClickUp's v3 endpoint rejects message-send requests that omit `type`. The CLI was already sending it.
  - `clickup_chat_dm` MCP tool reworked: previously sent `{user_id, content}`, which is not in the spec at all. The endpoint creates a DM channel (no message body) and takes `user_ids: [...]` per spec. New schema takes a `user_ids` array and returns the channel object; callers should follow with `clickup_chat_message_send` to post a message. CLI was already correct.
  - `clickup_chat_message_list`, `clickup_chat_channel_list`, and `clickup_chat_reply_list` MCP tools now read the v3 `data` envelope first and fall back to the older `messages`/`channels`/`replies` keys for compatibility. Before this fix the MCP tools consistently returned empty arrays because the v3 endpoints return their list under `data`.
  - `chat reaction-remove` CLI and `clickup_chat_reaction_remove` MCP tool now percent-encode the emoji segment in the request path. Sending a raw multi-byte emoji like `👍` previously produced a malformed URL.
- `goal create` (CLI and `clickup_goal_create` MCP tool) now sends the required `multiple_owners` boolean field that ClickUp's spec requires on goal create. CLI sends false (only single `--owner` is supported via the flag). MCP derives the value from the size of the `owner_ids` array. Without this fix the goal-create endpoint rejected requests with `multiple_owners is required`.
- `group create` CLI wire-level: body field is `members` (ClickUp's spec), the CLI was sending `member_ids`. Values must be integers, the CLI was sending strings. Parses `--member` values into integers and now bails clearly when a value is not numeric.
- `group list` CLI: ClickUp's `GET /v2/group` endpoint requires `team_id` as a query parameter. The CLI omitted it and the request 400'd. Now passes the resolved workspace id automatically.
- `group update` CLI: add/remove member arrays in the body are integers per spec, not strings. Now parsed and validated; non-numeric IDs produce a clear error before the request goes out.
- `clickup_doc_edit_page` MCP tool now supports the `mode` parameter (`replace` / `append` / `prepend`) and forwards it to ClickUp as `content_edit_mode`. The CLI's `doc edit-page --mode` flag was already wired; the MCP equivalent silently dropped any mode value and always replaced. Invalid values now error out before the request.
- `view create` (CLI and `clickup_view_create` MCP tool) now sends the seven required complex body fields (`grouping`, `divide`, `sorting`, `filters`, `columns`, `team_sidebar`, `settings`) populated with ClickUp's documented neutral defaults. The previous body sent only `{name, type}` which ClickUp's spec rejects (all seven are required). The resulting view can be customised in the ClickUp UI afterwards.
- `task create --description` and `task update --description` (and the `clickup_task_create` / `clickup_task_update` MCP tools) now render markdown in the ClickUp UI (#22). The CLI was sending the plain-text `description` API field, which doesn't interpret markdown; switched to `markdown_content`, ClickUp's documented markdown-rendering field. Plain-text descriptions still display identically. User-facing flag and MCP schema parameter name (`description`) are unchanged.
- `list create --content` and `list update --content` (and the `clickup_list_create` / `clickup_list_update` MCP tools) now render markdown in the ClickUp UI. Same root cause as the task-description bug above: the CLI was sending the plain-text `content` field, but ClickUp's docs explicitly say to use `markdown_content` to format a list description. CLI flag and MCP arg name (`content`) are unchanged.
- `clickup_chat_reaction_add` MCP tool and `chat reaction-add` CLI: the request body field was `emoji`, ClickUp's OpenAPI spec names it `reaction`. The CLI/MCP input arg name remains `emoji`, only the wire field changed. Without this fix the endpoint returned `Reaction required`.
- `clickup_tag_update` MCP tool sent `tag_fg` / `tag_bg` on the tag-update endpoint. ClickUp's update endpoint uses `fg_color` / `bg_color` (an inconsistency with the create endpoint, which still uses `tag_fg` / `tag_bg`). The CLI was already correct; the MCP tool now matches. Caller-facing arg names unchanged.
- `task time-in-status` bulk mode comma-joined every task ID into a single `task_ids=` query parameter, which ClickUp treats as one unknown ID. Switched to repeated `task_ids=A&task_ids=B&...` query params per the OpenAPI spec.
- `doc create --parent-type` sent the type as a string (`"SPACE"` etc.). ClickUp's spec requires an integer enum (4=space, 5=folder, 6=list, 7=everything, 12=workspace). CLI now accepts the string names case-insensitively (plus the raw integers) and translates to the integer enum. Help text expanded to list the new values (EVERYTHING, WORKSPACE).
- `doc list --creator` sent `creator_id=` as the query param, ClickUp's docs name it `creator`. Filter was silently ignored before; now applies as documented.

## [0.9.1] - 2026-04-23

### Fixed
- `msrv` CI job had been failing on every push since it was introduced (`5f21db6`) because `Cargo.lock` is format v4 (default for cargo 1.78+) but the declared MSRV was `1.75`. Bumped `rust-version` to `1.88` — the actual minimum enforced by transitive dependencies today (`toml_writer` needs edition 2024 → 1.85; `icu_*@2.2.0` → 1.86; `comfy-table 7.2.2` uses let-chains → 1.88). No runtime behaviour change.
- AUR publish workflow had never fired for any release. Root cause: our releases are created by `softprops/action-gh-release@v2` using `GITHUB_TOKEN`, and GitHub deliberately does not fire downstream workflow triggers for release events produced by `GITHUB_TOKEN` (anti-loop safeguard). Switched the AUR workflow's trigger from `release: [released]` to `workflow_run` after "Build and Release" completes successfully. Also added `workflow_dispatch` with a `tag` input so past releases can be rerun manually.
- Bumped `KSXGitHub/github-actions-deploy-aur` from `v4.1.1` → `v4.1.3`. v4.1.1 is broken upstream (`runuser` rewrites `-c` as `--command`, which bash rejects); fixed in v4.1.2, hardened in v4.1.3.

## [0.9.0] - 2026-04-23

### Added
- Task ID auto-detection from git branch names (#8). Task-scoped CLI commands (`task get`, `task update`, `task add-dep`, `comment create`, `attachment upload`, `checklist create`, `field set`, `member list`, `time list/create/start`, and more) now resolve the task ID from the current git branch when no explicit ID is given. Matches ClickUp default IDs like `CU-abc123` and custom `PREFIX-NUMBER` IDs; standard workflow prefixes (`feat/`, `fix/`, `hotfix/`, `bugfix/`, `release/`, `chore/`, `docs/`, `refactor/`, `test/`, `ci/`, `perf/`, `build/`, `style/`) are stripped before matching. Explicit CLI args always win; destructive or ambiguous commands (`task delete`, `task link`, `task unlink`, `guest share-task`, `guest unshare-task`) never auto-detect. Resolution chain: explicit arg → `CLICKUP_TASK_ID` env → git branch. A one-line breadcrumb `"resolved task X from branch Y"` is printed to stderr on table output; suppressed by `-q` or `--output json`.
- Explicit `CU-` prefix on task IDs is now transparently stripped (`clickup task get CU-abc123` → `GET /v2/task/abc123`).
- Custom-format explicit IDs (`PROJ-42`) auto-inject `custom_task_ids=true&team_id=<ws>` on all task-scoped commands, not just `task get --custom-task-id`.
- New config section `[git] enabled = true / verbose = true` in `~/.config/clickup-cli/config.toml` and `.clickup.toml`. Turn detection off entirely with `[git] enabled = false` or per-invocation with `CLICKUP_GIT_DETECT=0`.
- `CLICKUP_TASK_ID` environment variable as an alternative source (overrides branch, overridden by explicit CLI arg).
- `CLICKUP_API_URL` environment variable to point the CLI at a mock server (integration-test infrastructure; not for end users).
- MCP server is explicitly **out of scope** for branch-detect because the host editor pins the MCP server's `cwd` at spawn time — branch-detect from MCP would reliably resolve the wrong branch.

### Fixed
- `clickup attachment list` and the `clickup_attachment_list` MCP tool returned HTTP 400 for every task (#9). The CLI was calling `GET /v3/workspaces/{ws}/task/{id}/attachments`, which does not exist on ClickUp's side. Fixed to call `GET /v2/task/{id}` and extract the inline `attachments` array — per ClickUp's API docs, there is no dedicated list-attachments endpoint; attachments come back with the task itself.

### Changed
- `task add-tag` and `task remove-tag` now accept either `<task_id> <tag_name>` (two positionals, explicit) or `<tag_name>` alone (one positional, task auto-detected from branch). Fully backward compatible with the existing two-arg form.
- The `[ID]` positional on `task get`, `task update`, `task add-dep`, `task remove-dep`, `task move`, `task set-estimate`, `task replace-estimates`, and `task delete` is now optional in `--help` output. Non-branch-detect behaviour of `task delete` is unchanged — it still refuses without an explicit ID.

## [0.8.2] - 2026-04-23

### Added
- Statically-linked Linux musl release binary (`clickup-linux-x86_64-musl.tar.gz`) for Alpine, distroless, and minimal-container use cases (#3). Runs on any Linux distro; no libc or TLS runtime dependencies.
- Alpine install section in `docs/install.md` with a Dockerfile snippet, and a Linux Homebrew entry pointing at the existing tap.
- Changelog now mirrored at https://clickup-cli.com/changelog/ via an auto-sync workflow that rebuilds `docs/changelog.md` on every push that touches `CHANGELOG.md`.

## [0.8.1] - 2026-04-23

### Fixed
- Release workflow: run `cargo publish` on a clean tree before downloading build artifacts, and drop `--allow-dirty`. The v0.8.0 release hit crates.io's 10 MB upload cap because the prior order packaged the downloaded platform binaries into the crate tarball. As a result, v0.8.0 made it to the GitHub Release page but never reached crates.io, npm, GitHub Packages, or Homebrew — v0.8.1 is the first fully-published build of the MCP tool filtering feature.

## [0.8.0] - 2026-04-23

### Added
- `clickup mcp serve` now accepts filtering flags so the MCP server can expose a subset of its 143 tools at startup:
  - `--profile <all|read|safe>` (default `all`): `read` exposes only read-class tools; `safe` excludes destructive tools.
  - `--read-only` shortcut for `--profile read`.
  - `--groups` / `--exclude-groups` to include or drop resource groups (e.g. `task,comment,time`).
  - `--tools` / `--exclude-tools` to include or drop individual tools by exact name.
  - Matching environment variables: `CLICKUP_MCP_PROFILE`, `CLICKUP_MCP_READ_ONLY`, `CLICKUP_MCP_GROUPS`, `CLICKUP_MCP_EXCLUDE_GROUPS`, `CLICKUP_MCP_TOOLS`, `CLICKUP_MCP_EXCLUDE_TOOLS`.
- Filters apply to both `tools/list` (shrinks the LLM's context) and `tools/call` (rejects filtered tools with JSON-RPC `-32601`), so filtering is an access-control guarantee, not just a context optimization.
- Startup log line on stderr summarizing the active filter, e.g. `MCP: profile=read, exposing 52/143 tools`.
- Internal tool classifier mapping every MCP tool to a `(class, group)` pair with a CI self-check that fails if a tool can't be classified.

### Fixed
- Release workflow (`.github/workflows/build.yml`): `cargo publish` now runs with `--allow-dirty` (build artifacts in the workspace were making the tree "dirty") and all three publish steps (crates.io, npm, GitHub Packages) now check whether the version already exists before publishing and fail hard on any other error. The previous `|| echo "skipped"` pattern silently swallowed the crates.io failure during the v0.7.0 release.

## [0.7.0] - 2026-04-17

### Changed
- Rewrote MCP tool definitions for 134 of 143 tools to raise Glama Tool Definition Quality Score (TDQS). Pass 1 covered the 23 tools scoring ≤2.5; pass 2 covered the ~111 tools scoring 2.6–3.4. Only the 9 A-tier tools (≥3.5) were left untouched. Every rewritten tool now includes purpose context, usage guidance (with irreversibility warnings and pointers to safer alternatives for destructive ops), behavioural transparency (return value, cascading effects), and richer parameter semantics (how to obtain each ID, valid enum values, omission behaviour, constraints). Average `description` length went from ~30 chars to ~241 chars; target server grade uplift from C (2.8) to A under Glama's 60%-mean + 40%-min formula. Tool count, names, parameter names/types, and required/optional splits were preserved.
- Bumped `reqwest` from 0.12 to 0.13 (feature flag `rustls-tls` renamed to `rustls`; no code changes required).
- Bumped `toml` from 0.8 to 1.0.
- Relaxed `comfy-table` pin from `=7.1.1` to `"7"` (picks up 7.2.2).
- Relaxed `wiremock` pin from `=0.6.0` to `"0.6"` (picks up 0.6.5).
- `cargo update` across all transitive deps (tokio 1.50→1.52, clap 4.6.0→4.6.1, rustls 0.23.37→0.23.38, plus ~40 others).
- Synced `npm/package.json` version to 0.6.7.

### Added
- Jekyll SEO plumbing for clickup-cli.com: `jekyll-seo-tag` + `jekyll-sitemap` plugins, `Gemfile`, `robots.txt`, `docs/assets/og-image.{svg,png}` (1200×630 social card).
- Per-page `description` and explicit `permalink` on `index.html`, `install.md`, `commands.md`, `mcp.md`.
- `theme-color` meta tag in the layout.
- "Made by D3 Vitamin" footer attribution.

### Fixed
- Home page `<title>` no longer renders as "Home — clickup-cli"; now uses a keyword-rich title generated by `jekyll-seo-tag`.
- Nav links now target trailing-slash permalinks (avoids GitHub Pages redirect hops).

## Prior versions

Release notes for 0.6.7 and earlier are auto-generated from commit history on the
[GitHub Releases page](https://github.com/nicholasbester/clickup-cli/releases).

[Unreleased]: https://github.com/nicholasbester/clickup-cli/compare/v0.12.1...HEAD
[0.12.1]: https://github.com/nicholasbester/clickup-cli/compare/v0.12.0...v0.12.1
[0.12.0]: https://github.com/nicholasbester/clickup-cli/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/nicholasbester/clickup-cli/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/nicholasbester/clickup-cli/compare/v0.9.1...v0.10.0
[0.9.1]: https://github.com/nicholasbester/clickup-cli/compare/v0.9.0...v0.9.1
[0.9.0]: https://github.com/nicholasbester/clickup-cli/compare/v0.8.2...v0.9.0
[0.8.2]: https://github.com/nicholasbester/clickup-cli/compare/v0.8.1...v0.8.2
[0.8.1]: https://github.com/nicholasbester/clickup-cli/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/nicholasbester/clickup-cli/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/nicholasbester/clickup-cli/compare/v0.6.7...v0.7.0
