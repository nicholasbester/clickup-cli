# clickup-cli

Rust CLI for the ClickUp API, optimized for AI agent consumption. Covers all ~130 endpoints across 28 resource groups and 4 utility commands.

## Build & Test

```bash
cargo build                    # Build
cargo test                     # Run all tests
cargo test --test test_cli     # CLI smoke tests only
cargo test --test test_output  # Output formatting tests only
cargo run -- --help            # Show help
```

## Architecture

- Single binary: `clickup`
- Entry: `src/main.rs` → `src/lib.rs`
- Commands: `src/commands/{resource}.rs` — one file per API resource group
- Models: `src/models/{resource}.rs` — serde structs for API responses
- Core: `src/client.rs` (HTTP + retry), `src/config.rs` (TOML), `src/output.rs` (formatting), `src/error.rs` (errors)

## CLI Pattern

```
clickup <resource> <action> [ID] [flags]
```

## Command Groups

### Core (v0.1)
- `setup` — configure API token and default workspace
- `auth` — whoami, check
- `workspace` — list, seats, plan
- `space` — list, get, create, update, delete
- `folder` — list, get, create, update, delete
- `list` — list, get, create, update, delete, add-task, remove-task
- `task` — list, search, get, create, update, delete, time-in-status, add-tag, remove-tag, add-dep, remove-dep, link, unlink, move, set-estimate, replace-estimates

### Collaboration (v0.2)
- `checklist` — create, update, delete, add-item, update-item, delete-item
- `comment` — list, create, update, delete, replies, reply
- `tag` — list, create, update, delete
- `field` — list, set, unset
- `task-type` — list
- `attachment` — list, upload

### Tracking (v0.3)
- `time` — list, get, current, create, update, delete, start, stop, tags, add-tags, remove-tags, rename-tag, history
- `goal` — list, get, create, update, delete, add-kr, update-kr, delete-kr
- `view` — list, get, create, update, delete, tasks
- `member` — list
- `user` — invite, get, update, remove

### Communication (v0.4)
- `chat` — channel-list, channel-create, channel-get, channel-update, channel-delete, channel-followers, channel-members, dm, message-list, message-send, message-update, message-delete, reaction-list, reaction-add, reaction-remove, reply-list, reply-send, tagged-users
- `doc` — list, create, get, pages, add-page, page, edit-page
- `webhook` — list, create, update, delete
- `template` — list, apply-task, apply-list, apply-folder

### Admin (v0.5)
- `guest` — invite, get, update, remove, share-task, unshare-task, share-list, unshare-list, share-folder, unshare-folder
- `group` — list, create, update, delete
- `role` — list
- `shared` — list
- `audit-log` — query (Enterprise)
- `acl` — update (Enterprise)

### Utilities
- `status` — show current config, token (masked), workspace
- `completions` — generate shell completions (bash, zsh, fish, powershell)
- `agent-config` — show or inject compressed CLI reference into CLAUDE.md
- `mcp serve` — start MCP server (JSON-RPC over stdio, 18 tools)

## Global Flags

- `--token TOKEN` — override config file token
- `--workspace ID` — override default workspace
- `--output MODE` — table (default), json, json-compact, csv
- `--fields LIST` — comma-separated field names
- `--no-header` — omit table header
- `--all` — fetch all pages
- `--limit N` — cap results
- `--page N` — manual page
- `-q` / `--quiet` — IDs only
- `--timeout SECS` — HTTP timeout (default 30)

## Config

Location: `~/.config/clickup-cli/config.toml`

```toml
[auth]
token = "pk_..."

[defaults]
workspace_id = "12345"
```

Token resolution: `--token` flag > `CLICKUP_TOKEN` env var > config file
Workspace resolution: `--workspace` flag > `CLICKUP_WORKSPACE` env var > config file

## MCP Server

Start with `clickup mcp serve`. Returns token-efficient compact JSON (same flattening as CLI tables). Exposes 18 tools: clickup_whoami, clickup_workspace_list, clickup_space_list, clickup_folder_list, clickup_list_list, clickup_task_list, clickup_task_get, clickup_task_create, clickup_task_update, clickup_task_delete, clickup_task_search, clickup_comment_list, clickup_comment_create, clickup_field_list, clickup_field_set, clickup_time_start, clickup_time_stop, clickup_time_list.

## Exit Codes

- 0: success
- 1: client error (400, bad input)
- 2: auth/permission error (401, 403)
- 3: not found (404)
- 4: rate limited (429)
- 5: server error (5xx)

## Key API Notes

- "team_id" in v2 = workspace_id
- All timestamps are Unix milliseconds
- Priority: 1=Urgent, 2=High, 3=Normal, 4=Low
- task_count on folders is a string, not integer
- v3 endpoints (chat, docs, audit logs, ACLs, attachments) use cursor pagination
- Tag create uses tag_fg/tag_bg, tag update uses fg_color/bg_color (API inconsistency)
- Webhook update/delete use /v2/webhook/{id} path
- Guest, audit-log, and ACL endpoints require Enterprise plan
