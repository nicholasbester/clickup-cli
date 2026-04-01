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
- `agent-config` — show or inject CLI reference (auto-detects CLAUDE.md, agent.md, .cursorrules, etc.)
- `mcp serve` — start MCP server (JSON-RPC over stdio, 143 tools, 100% API coverage)

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

| Level | File |
|-------|------|
| Project | `.clickup.toml` (current directory) |
| Global | `~/.config/clickup-cli/config.toml` |

```toml
[auth]
token = "pk_..."

[defaults]
workspace_id = "12345"
```

Resolution: `--flag` > `CLICKUP_TOKEN`/`CLICKUP_WORKSPACE` env > `.clickup.toml` > global config

## MCP Server

Start with `clickup mcp serve`. Returns token-efficient compact JSON (same flattening as CLI tables). Exposes 143 tools with 100% ClickUp API coverage — every endpoint available via CLI is also available as an MCP tool.

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

<!-- clickup-cli:begin -->To interface with ClickUp, use the `clickup` CLI (LLM-agnostic, works with any AI agent). Pattern: `clickup <resource> <action> [ID] [flags]`. Global flags: --output table|json|json-compact|csv, --fields LIST, -q (IDs only), --no-header, --all (paginate), --limit N, --page N, --token TOKEN, --workspace ID, --timeout SECS. Commands: setup [--token T]; auth whoami|check; workspace list|seats|plan; space list [--archived]|get ID|create --name N [--private]|update ID [--name N]|delete ID; folder list --space ID|get ID|create --space ID --name N|update ID --name N|delete ID; list list --folder ID|--space ID|get ID|create --folder ID|--space ID --name N [--content T] [--due-date DATE]|update ID|delete ID|add-task LIST TASK|remove-task LIST TASK; task list --list ID [--status S] [--assignee ID] [--tag T] [--include-closed]|search [--space ID] [--status S]|get ID [--subtasks] [--custom-task-id]|create --list ID --name N [--description T] [--status S] [--priority 1-4] [--assignee ID] [--tag T] [--due-date DATE] [--parent ID]|update ID [--name N] [--status S] [--priority N] [--add-assignee ID] [--rem-assignee ID]|delete ID|time-in-status ID...|add-tag ID TAG|remove-tag ID TAG|add-dep ID --depends-on ID|remove-dep ID --depends-on ID|link ID TARGET|unlink ID TARGET|move ID --list ID|set-estimate ID --assignee ID --time MS|replace-estimates ID --assignee ID --time MS; checklist create --task ID --name N|update ID [--name N]|delete ID|add-item ID --name N|update-item ID ITEM [--name N] [--resolved]|delete-item ID ITEM; comment list --task ID|--list ID|--view ID|create --task ID|--list ID|--view ID --text T [--notify-all]|update ID --text T [--resolved]|delete ID|replies ID|reply ID --text T; tag list --space ID|create --space ID --name N [--fg-color H] [--bg-color H]|update --space ID --tag N [--name NEW]|delete --space ID --tag N; field list --list ID|--folder ID|--space ID|--workspace-level|set TASK FIELD --value V|unset TASK FIELD; task-type list; attachment list --task ID|upload --task ID FILE; time list [--start-date D] [--end-date D] [--task ID]|get ID|current|create --start D --duration MS [--task ID]|update ID|delete ID|start [--task ID]|stop|tags|add-tags --entry-id ID --tag N|remove-tags --entry-id ID --tag N|rename-tag --name OLD --new-name NEW|history ID; goal list|get ID|create --name N --due-date D|update ID|delete ID|add-kr ID --name N --type T --steps-start N --steps-end N|update-kr ID --steps-current N|delete-kr ID; view list --workspace-level|--space ID|--folder ID|--list ID|get ID|create --name N --type T --space ID|--folder ID|--list ID|update ID|delete ID|tasks ID; member list --task ID|--list ID; user invite --email E|get ID|update ID|remove ID; chat channel-list|channel-create --name N|channel-get ID|channel-update ID|channel-delete ID|channel-followers ID|channel-members ID|dm USER...|message-list --channel ID|message-send --channel ID --text T|message-update ID --text T|message-delete ID|reaction-list MSG|reaction-add MSG --emoji E|reaction-remove MSG EMOJI|reply-list MSG|reply-send MSG --text T|tagged-users MSG; doc list|create --name N|get ID|pages ID [--content]|add-page DOC --name N [--content T]|page DOC PAGE|edit-page DOC PAGE --content T [--mode replace|append|prepend]; webhook list|create --endpoint URL --event E|update ID --endpoint URL --event E|delete ID; template list|apply-task TPL --list ID --name N|apply-list TPL --folder ID|--space ID --name N|apply-folder TPL --space ID --name N; guest invite --email E|get ID|update ID|remove ID|share-task TASK GUEST --permission P|unshare-task TASK GUEST|share-list LIST GUEST --permission P|unshare-list LIST GUEST|share-folder FOLDER GUEST --permission P|unshare-folder FOLDER GUEST; group list|create --name N --member ID|update ID [--add-member ID] [--rem-member ID]|delete ID; role list; shared list; audit-log query --type T [--user-id ID] [--start-date D] [--end-date D]; acl update TYPE ID [--private] [--body JSON]. Priority: 1=Urgent 2=High 3=Normal 4=Low. Dates: YYYY-MM-DD. All timestamps Unix ms. team_id=workspace_id in API. Exit codes: 0=ok 1=client-error 2=auth 3=not-found 4=rate-limited 5=server-error. Config: ~/.config/clickup-cli/config.toml or .clickup.toml (project-level). Setup: `clickup setup --token pk_XXX`.<!-- clickup-cli:end -->
