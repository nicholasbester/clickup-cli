# clickup-cli: Rust Rewrite Design Spec

## Overview

A Rust CLI wrapping the full ClickUp REST API (v2 + v3), optimized for both human and AI agent consumption. Replaces the dormant Go CLI (fantasticrabbit/ClickupCLI) which covered only 2 of 28 API resource groups.

**Primary goals:**
1. Cover all ~120 ClickUp API endpoints across 30 resource groups
2. Token-efficient default output for AI agent context windows
3. Simple auth via personal API tokens — no OAuth browser flow
4. Phased releases, usable from v0.1

**Repository:** https://github.com/nicholasbester/clickup-cli

## Architecture

### Approach: Monolithic Crate

Single crate, single binary. Each resource group gets its own module under `src/commands/` and `src/models/`.

```
src/
  main.rs                    # clap CLI definition, command routing
  client.rs                  # HTTP client, auth, rate limiting, pagination
  config.rs                  # Config file read/write, setup flow
  output.rs                  # Table/compact/json formatting, --fields
  commands/
    setup.rs                 # clickup setup
    auth.rs                  # clickup auth whoami
    workspace.rs             # clickup workspace {list,seats,plan}
    space.rs                 # clickup space {list,get,create,update,delete}
    folder.rs                # clickup folder {list,get,create,update,delete}
    list.rs                  # clickup list {list,get,create,update,delete,add-task,remove-task}
    task.rs                  # clickup task {list,search,get,create,update,delete,...}
    checklist.rs             # clickup checklist {create,update,delete,add-item,...}
    comment.rs               # clickup comment {list,create,update,delete,replies,reply}
    tag.rs                   # clickup tag {list,create,update,delete}
    field.rs                 # clickup field {list,set,unset}
    task_type.rs             # clickup task-type list
    goal.rs                  # clickup goal {list,get,create,update,delete,add-kr,...}
    view.rs                  # clickup view {list,get,create,update,delete,tasks}
    member.rs                # clickup member list
    user.rs                  # clickup user {invite,get,update,remove}
    guest.rs                 # clickup guest {invite,get,update,remove,share-*,unshare-*}
    group.rs                 # clickup group {list,create,update,delete}
    role.rs                  # clickup role list
    shared.rs                # clickup shared list
    time.rs                  # clickup time {list,get,current,create,update,delete,start,stop,...}
    webhook.rs               # clickup webhook {list,create,update,delete}
    template.rs              # clickup template {list,apply-task,apply-list,apply-folder}
    chat.rs                  # clickup chat {channel,message,reaction,reply,...}
    doc.rs                   # clickup doc {list,get,create,pages,add-page,page,edit-page}
    audit_log.rs             # clickup audit-log query
    acl.rs                   # clickup acl update
    attachment.rs            # clickup attachment {list,upload}
  models/
    mod.rs                   # Common types (Pagination, ApiError, etc.)
    task.rs                  # Task, TaskStatus, Priority, etc.
    list.rs                  # List, ListStatus
    space.rs                 # Space, SpaceFeatures
    folder.rs                # Folder
    workspace.rs             # Workspace, WorkspacePlan, Seats
    comment.rs               # Comment, ThreadedComment
    goal.rs                  # Goal, KeyResult
    view.rs                  # View, ViewType
    user.rs                  # User, Member, Guest
    time_entry.rs            # TimeEntry, TimeTag
    webhook.rs               # Webhook, WebhookEvent
    chat.rs                  # ChatChannel, ChatMessage, ChatReaction
    doc.rs                   # Doc, DocPage
    custom_field.rs          # CustomField, FieldType, FieldValue
    tag.rs                   # Tag
    template.rs              # Template, TemplateOptions
    attachment.rs            # Attachment
```

### Why This Approach

- Simple: one crate, fast compilation at this scale (~120 endpoints across ~28 files)
- Easy navigation: one file per resource group in both commands/ and models/
- `clap` derive macros work naturally with this layout
- Can split into a workspace later if a reusable client library becomes valuable

## CLI Command Structure

### Pattern

```
clickup <resource> <action> [ID] [flags]
```

Resource-first (like `gh`, `kubectl`, `docker`). Groups help text by resource for discoverability.

### Global Flags

| Flag | Description |
|------|-------------|
| `--token <TOKEN>` | Override config file token |
| `--workspace <ID>` | Override default workspace |
| `--output <MODE>` | table (default), json, json-compact, csv |
| `--fields <LIST>` | Comma-separated field names |
| `--no-header` | Omit table header row |
| `--all` | Fetch all pages (auto-paginate) |
| `--limit <N>` | Cap total results |
| `--page <N>` | Manual page selection |
| `--quiet` / `-q` | Only print IDs, one per line |
| `--timeout <SECS>` | HTTP timeout (default 30) |

### v0.1 Commands (Core Workflow)

```
clickup setup [--token TOKEN]
clickup auth whoami
clickup auth check

clickup workspace list
clickup workspace seats
clickup workspace plan

clickup space list [--archived]
clickup space get <ID>
clickup space create --name NAME [--private] [--multiple-assignees]
clickup space update <ID> [--name NAME] [--color HEX]
clickup space delete <ID>

clickup folder list --space <ID> [--archived]
clickup folder get <ID>
clickup folder create --space <ID> --name NAME
clickup folder update <ID> --name NAME
clickup folder delete <ID>

clickup list list --folder <ID> [--archived]
clickup list list --space <ID> [--archived]
clickup list get <ID>
clickup list create --folder <ID> --name NAME [--content TEXT] [--priority N] [--due-date DATE]
clickup list create --space <ID> --name NAME
clickup list update <ID> [--name NAME] [--content TEXT]
clickup list delete <ID>
clickup list add-task <LIST_ID> <TASK_ID>
clickup list remove-task <LIST_ID> <TASK_ID>

clickup task list --list <ID> [--status X] [--assignee ID] [--tag X] [--include-closed] [--order-by field] [--reverse]
clickup task search [--space ID] [--folder ID] [--list ID] [--status X] [--assignee ID] [--tag X]
clickup task get <ID> [--subtasks] [--custom-task-id]
clickup task create --list <ID> --name NAME [--description TEXT] [--status S] [--priority 1-4] [--assignee ID] [--tag NAME] [--due-date DATE] [--parent TASK_ID]
clickup task update <ID> [--name X] [--status X] [--priority N] [--add-assignee ID] [--rem-assignee ID] [--description TEXT]
clickup task delete <ID>
clickup task time-in-status <ID>
clickup task time-in-status --bulk <ID1> <ID2> ...
```

### Post-v0.1 Commands

See GitHub issues #16-#39 for complete CLI command checklists per resource group, organized by release version (v0.2 through v0.5).

## Authentication

### Method

Personal API tokens only (`pk_*`). No OAuth, no client ID/secret, no `.env` files.

### Resolution Order (highest wins)

1. `--token` CLI flag
2. Config file `~/.config/clickup-cli/config.toml`

### Setup Flow

**Interactive (humans):**
```
$ clickup setup
Welcome to clickup-cli!

API Token (get one at Settings > Apps): pk_****
Validating... ✓ Authenticated as Nick Bester

Fetching workspaces...
  [1] Sparkland Trading (ID: 2648001)

Only one workspace found — setting as default.
Config saved to ~/.config/clickup-cli/config.toml
```

**Non-interactive (AI agents):**
```
$ clickup setup --token pk_12345
Config saved to ~/.config/clickup-cli/config.toml
```

### Workspace Resolution

- If one workspace: auto-set as default
- If multiple: show list, let user pick
- Always overridable with `--workspace` flag

### When Not Configured

```
Error: Not configured
  Hint: Run 'clickup setup' to configure your API token
```

## Configuration

### File Location

`~/.config/clickup-cli/config.toml`

### Schema

```toml
[auth]
token = "pk_12345..."

[defaults]
workspace_id = "2648001"
output = "table"          # optional default output mode
```

Minimal by design. Token + workspace only. Optional output preference.

## Output Formatting

### Modes

| Flag | Mode | Use Case |
|------|------|----------|
| _(default)_ | `table` | Aligned columns, header row, essential fields |
| `--output json` | Full JSON | Complete API response for `jq` |
| `--output json-compact` | Filtered JSON | Only default fields, as JSON |
| `--output csv` | CSV | Spreadsheets, data pipelines |
| `--quiet` / `-q` | IDs only | One ID per line, for scripting |
| `--no-header` | Headerless table | Piping, concatenation |

### Default Fields Per Resource

| Resource | Default Fields |
|----------|---------------|
| Task | id, name, status, priority, assignees, due_date |
| List | id, name, task_count, status, due_date |
| Space | id, name, private, archived |
| Folder | id, name, task_count, list_count |
| Comment | id, user, date, text (truncated 60 chars) |
| Goal | id, name, percent_completed, due_date |
| Time Entry | id, task_name, duration, start, billable |

### Field Flattening

Nested API objects are flattened to display values:

| API Response | Displayed As |
|-------------|-------------|
| `status: {status: "in progress", ...}` | `"in progress"` |
| `priority: {priority: "high", ...}` | `"high"` |
| `assignees: [{username: "Nick"}]` | `"Nick"` |
| `assignees: [{username: "Nick"}, {username: "Bob"}]` | `"Nick, Bob"` |
| `due_date: "1773652547089"` | `"2026-03-17"` |
| `null` | `"-"` |

### Custom Fields

`--fields id,name,status` selects top-level flattened fields. Works with all output modes.

### Token Efficiency Example

`clickup task list --list 901522179701` for 5 tasks:

- **Full JSON:** ~450 lines (~12,000 tokens)
- **Table (default):** ~7 lines (~150 tokens)
- **Reduction:** ~98%

## HTTP Client

### Stack

- `reqwest` with `rustls-tls`
- Base URL: `https://api.clickup.com/api`
- Default timeout: 30s
- Headers: `Authorization: pk_...`, `Content-Type: application/json`

### Rate Limiting

Reads `X-RateLimit-Remaining` and `X-RateLimit-Reset` response headers.

| Plan | Requests/min |
|------|-------------|
| Free/Unlimited/Business | 100 |
| Business Plus | 1,000 |
| Enterprise | 10,000 |

### Retry Policy

| Response | Action |
|----------|--------|
| 429 | Wait until `X-RateLimit-Reset`, retry once |
| 5xx | Exponential backoff: 1s, 2s, 4s (max 3 retries) |
| All others | No retry |

### Pagination

**v2 (page-based):**
- `page=0`, 100 items/page, `last_page: boolean` in response
- Used by: Get Tasks, Get View Tasks, Get Task Templates

**v3 (cursor-based):**
- `cursor` + `limit` (1-100, default 50), `next_cursor` in response
- Used by: All Chat, Docs, Attachments v3 endpoints

**CLI flags:**
- `--all`: fetch all pages, stream results
- `--limit N`: cap total results
- `--page N`: manual page selection

## Error Handling

### Format

```
Error: Task not found
  Status:  404
  Task ID: abc123
  Hint:    Check the task ID, or use --custom-task-id if using a custom ID
```

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Client error (bad input, 400) |
| 2 | Auth error (401, no token configured) |
| 3 | Not found (404) |
| 4 | Rate limited (429, after retries exhausted) |
| 5 | Server error (5xx, after retries exhausted) |

## Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` v4 (derive) | CLI argument parsing |
| `reqwest` (rustls-tls) | HTTP client |
| `serde` / `serde_json` | JSON serialization |
| `tokio` | Async runtime |
| `toml` | Config file parsing |
| `dirs` | Platform config paths |
| `thiserror` | Error type derive |
| `chrono` | Date formatting/parsing |
| `comfy-table` | Table output formatting |

## Release Plan

| Release | Scope | GitHub Issues |
|---------|-------|---------------|
| **v0.1** | Setup, auth, workspaces, spaces, folders, lists, tasks, output engine | #1-#6, #8, #10-#15 (tracker: #40) |
| **v0.2** | Comments, tags, custom fields, custom task types, checklists, relationships, attachments | #16-#22 (tracker: #41) |
| **v0.3** | Time tracking, goals, views, members, users | #24-#27, #32 (tracker: #42) |
| **v0.4** | Chat v3, docs v3, webhooks, templates, tasks v3 | #23, #33-#35, #38 (tracker: #43) |
| **v0.5** | Guests, groups, roles, shared hierarchy, audit logs, ACLs, attachments v3, CI/CD, integration tests | #7, #9, #28-#31, #36-#37, #39 (tracker: #44) |

## API Coverage

30 resource groups (24 v2 + 6 v3), ~120 endpoints total. See GitHub issues #10-#39 for complete per-endpoint documentation including HTTP methods, paths, parameters, response schemas, and CLI command mappings.

### v2 (24 groups)
Authorization, Workspaces, Spaces, Folders, Lists, Tasks, Task Checklists, Task Relationships, Comments, Custom Fields, Custom Task Types, Attachments, Tags, Goals, Views, Members, Users, Guests, User Groups, Roles, Time Tracking, Webhooks, Shared Hierarchy, Templates

### v3 (6 groups)
Chat, Docs, Audit Logs, Privacy & Access, Tasks (move + time estimates), Attachments

## API Notes

- **Base URL v2:** `https://api.clickup.com/api/v2/`
- **Base URL v3:** `https://api.clickup.com/api/v3/`
- **Timestamps:** All Unix milliseconds
- **"team_id" means workspace_id** throughout v2
- **Tag color inconsistency:** create uses `tag_fg`/`tag_bg`, edit uses `fg_color`/`bg_color`
- **No official SDK** for any language — this wraps the raw REST API
- **Custom task IDs:** many task endpoints support `--custom-task-id` flag
- **Priority values:** 1=Urgent, 2=High, 3=Normal, 4=Low
