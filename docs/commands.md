---
layout: default
title: Command Reference
description: Complete reference for every clickup-cli command — workspace, task, list, folder, time tracking, docs, chat, webhooks, and more. Covers ~130 ClickUp API endpoints.
permalink: /commands/
---

# Command Reference

Pattern: `clickup-cli <resource> <action> [ID] [flags]` (or `clkup` for short)

## Global Flags

| Flag | Description |
|------|-------------|
| `--output MODE` | `table` (default), `json`, `json-compact`, `csv` |
| `--fields LIST` | Comma-separated field names to display |
| `--no-header` | Omit table header row |
| `-q` / `--quiet` | Print IDs only, one per line |
| `--all` | Auto-walk every page; applies to every paginated list command (hard-capped at 100 pages) |
| `--limit N` | Cap total items returned (enforced after walking, so `--all --limit 500` returns ≤500 across N pages) |
| `--page N` | Manual page selection (v2 page-style: `task list`, `task search`, `view tasks`, `template list`) |
| `--cursor X` | Manual cursor (v3 cursor-style: `doc list`, all `chat *` list commands) |
| `--start MS` + `--start-id ID` | Manual boundary pair (v2 start-id-style: `comment list`, `comment replies`) |
| `--token TOKEN` | Override config file token |
| `--workspace ID` | Override default workspace |
| `--timeout SECS` | HTTP timeout (default 30) |

---

## setup

Configure API token and default workspace.

```bash
# Interactive
clickup-cli setup

# Non-interactive
clickup-cli setup --token pk_your_token_here
```

```
Validating... ✓ Authenticated as Jane Smith

Only one workspace found — setting as default.
  Acme Corp (ID: 1234567)
Config saved to ~/.config/clickup-cli/config.toml
```

---

## auth

```bash
clickup-cli auth whoami    # Show current user
clickup-cli auth check     # Validate token (exit code only)
```

```
+----------+------------+---------------------+
| id       | username   | email               |
+====================================================+
| 12345678 | Jane Smith | jane@acme-corp.com  |
+----------+------------+---------------------+
```

---

## workspace

```bash
clickup-cli workspace list     # List workspaces
clickup-cli workspace seats    # Show seat usage
clickup-cli workspace plan     # Show current plan
```

```
+---------+-----------+---------+
| id      | name      | members |
+===============================+
| 1234567 | Acme Corp | 24      |
+---------+-----------+---------+
```

---

## space

```bash
clickup-cli space list [--archived]
clickup-cli space get <ID>
clickup-cli space create --name NAME [--private] [--multiple-assignees]
clickup-cli space update <ID> [--name NAME] [--color HEX]
clickup-cli space delete <ID>
```

```
+----------+-------------+---------+----------+
| id       | name        | private | archived |
+================================================+
| 1001     | Engineering | true    | false    |
| 1002     | Marketing   | false   | false    |
| 1003     | Operations  | true    | false    |
+----------+-------------+---------+----------+
```

---

## folder

```bash
clickup-cli folder list --space <ID> [--archived]
clickup-cli folder get <ID>
clickup-cli folder create --space <ID> --name NAME
clickup-cli folder update <ID> --name NAME
clickup-cli folder delete <ID>
```

```
+-------+-----------------+------------+------------+
| id    | name            | task_count | list_count |
+====================================================+
| 2001  | Q1 Initiatives  | 45         | 6          |
| 2002  | Sprint Backlog  | 128        | 4          |
| 2003  | Product Roadmap | 32         | 3          |
+-------+-----------------+------------+------------+
```

---

## list

```bash
clickup-cli list list --folder <ID> [--archived]
clickup-cli list list --space <ID> [--archived]    # folderless lists
clickup-cli list get <ID>
clickup-cli list create --folder <ID> --name NAME [--content TEXT] [--priority N] [--due-date DATE]
clickup-cli list create --space <ID> --name NAME   # folderless
clickup-cli list update <ID> [--name NAME] [--content TEXT]
clickup-cli list delete <ID>
clickup-cli list add-task <LIST_ID> <TASK_ID>
clickup-cli list remove-task <LIST_ID> <TASK_ID>
```

```
+-------+-------------------------------+------------+--------+------------+
| id    | name                          | task_count | status | due_date   |
+=========================================================================+
| 3001  | Backlog                       | 87         | -      | -          |
| 3002  | Sprint 12 (4/1/26 - 4/14/26)  | 18         | -      | 2026-04-14 |
+-------+-------------------------------+------------+--------+------------+
```

---

## task

```bash
# List and search
clickup-cli task list --list <ID> [--status S] [--assignee ID] [--tag T] [--include-closed] [--order-by field] [--reverse]
clickup-cli task search [--space ID] [--folder ID] [--list ID] [--status S] [--assignee ID] [--tag T]

# CRUD
clickup-cli task get <ID> [--subtasks] [--custom-task-id]
clickup-cli task create --list <ID> --name NAME [--description TEXT] [--status S] [--priority 1-4] [--assignee ID] [--tag NAME] [--due-date DATE] [--parent TASK_ID]
clickup-cli task update <ID> [--name X] [--status X] [--priority N] [--add-assignee ID] [--rem-assignee ID] [--description TEXT]
clickup-cli task delete <ID>

# Relationships and tags
clickup-cli task add-dep <ID> --depends-on <OTHER_ID>
clickup-cli task remove-dep <ID> --depends-on <OTHER_ID>
clickup-cli task link <ID> <TARGET_ID>
clickup-cli task unlink <ID> <TARGET_ID>
clickup-cli task add-tag <ID> <TAG_NAME>
clickup-cli task remove-tag <ID> <TAG_NAME>

# Time and estimates
clickup-cli task time-in-status <ID>...
clickup-cli task move <ID> --list <LIST_ID>
clickup-cli task set-estimate <ID> --assignee USER_ID --time MS
clickup-cli task replace-estimates <ID> --assignee USER_ID --time MS
```

```
+-----------+-----------------------------------------+-------------+----------+-------------+------------+
| id        | name                                    | status      | priority | assignees   | due_date   |
+==========================================================================================================+
| abc123    | Migrate database to new cluster         | Open        | urgent   | Alex Chen   | -          |
| def456    | API rate limiter implementation          | in progress | high     | Sara Jones  | 2026-04-15 |
| ghi789    | Update onboarding docs                  | Open        | normal   | -           | -          |
+-----------+-----------------------------------------+-------------+----------+-------------+------------+
```

Priority values: 1=Urgent, 2=High, 3=Normal, 4=Low. Dates: YYYY-MM-DD format.

---

## checklist

```bash
clickup-cli checklist create --task <ID> --name NAME
clickup-cli checklist update <ID> [--name NAME] [--position N]
clickup-cli checklist delete <ID>
clickup-cli checklist add-item <ID> --name NAME [--assignee USER_ID]
clickup-cli checklist update-item <ID> <ITEM_ID> [--name NAME] [--resolved] [--assignee USER_ID]
clickup-cli checklist delete-item <ID> <ITEM_ID>
```

---

## comment

```bash
clickup-cli comment list --task <ID>          # also --list, --view
clickup-cli comment create --task <ID> --text TEXT [--assignee ID] [--notify-all]
clickup-cli comment create --list <ID> --text TEXT
clickup-cli comment create --view <ID> --text TEXT
clickup-cli comment update <ID> --text TEXT [--resolved] [--assignee ID]
clickup-cli comment delete <ID>
clickup-cli comment replies <ID>              # list threaded replies
clickup-cli comment reply <ID> --text TEXT [--assignee ID]
```

---

## tag

```bash
clickup-cli tag list --space <ID>
clickup-cli tag create --space <ID> --name NAME [--fg-color HEX] [--bg-color HEX]
clickup-cli tag update --space <ID> --tag NAME [--name NEW_NAME] [--fg-color HEX] [--bg-color HEX]
clickup-cli tag delete --space <ID> --tag NAME
```

```
+---------------+----------+----------+
| name          | fg_color | bg_color |
+======================================+
| frontend      | #c51162  | #c51162  |
| bug           | #ff0000  | #ff0000  |
| documentation | #FF7800  | #f9d900  |
+---------------+----------+----------+
```

---

## field

Custom fields on tasks.

```bash
clickup-cli field list --list <ID>            # also --folder, --space, --workspace-level
clickup-cli field set <FIELD_ID> --value VALUE [TASK_ID]
clickup-cli field unset <FIELD_ID> [TASK_ID]
```

Value can be a string, number, or JSON for complex field types. For `drop_down`, use the option ID from the field's `type_config.options`; for `labels`, pass a JSON array of option IDs.

---

## task-type

```bash
clickup-cli task-type list
```

```
+------+-----------+-------------+
| id   | name      | name_plural |
+===================================+
| 1    | milestone | -           |
| 1003 | Bug       | Bugs        |
| 1007 | Epic      | Epics       |
+------+-----------+-------------+
```

---

## attachment

```bash
clickup-cli attachment list --task <ID>
clickup-cli attachment upload --task <ID> <FILE_PATH>
```

---

## time

Time tracking with start/stop timer support.

```bash
# Timer
clickup-cli time start [--task ID] [--description TEXT] [--billable]
clickup-cli time stop

# CRUD
clickup-cli time list [--start-date DATE] [--end-date DATE] [--assignee ID] [--task ID]
clickup-cli time get <ID>
clickup-cli time current
clickup-cli time create --start DATE --duration MS [--task ID] [--description TEXT] [--billable]
clickup-cli time update <ID> [--start DATE] [--end DATE] [--description TEXT] [--billable]
clickup-cli time delete <ID>

# Tags
clickup-cli time tags
clickup-cli time add-tags --entry-id ID --tag NAME [--tag NAME...]
clickup-cli time remove-tags --entry-id ID --tag NAME
clickup-cli time rename-tag --name OLD --new-name NEW

# History
clickup-cli time history <ID>
```

---

## goal

```bash
clickup-cli goal list [--include-completed]
clickup-cli goal get <ID>
clickup-cli goal create --name NAME --due-date DATE --description TEXT [--color HEX] [--owner ID]
clickup-cli goal update <ID> [--name NAME] [--due-date DATE] [--add-owner ID] [--rem-owner ID]
clickup-cli goal delete <ID>

# Key Results
clickup-cli goal add-kr <GOAL_ID> --name NAME --type TYPE --steps-start N --steps-end N [--unit UNIT] [--owner ID]
clickup-cli goal update-kr <KR_ID> --steps-current N [--note TEXT]
clickup-cli goal delete-kr <KR_ID>
```

Key result types: `number`, `currency`, `boolean`, `percentage`, `automatic`

---

## view

```bash
clickup-cli view list --workspace-level       # also --space, --folder, --list
clickup-cli view get <ID>
clickup-cli view create --name NAME --type TYPE --space <ID>   # also --folder, --list, --workspace-level
clickup-cli view update <ID> [--name NAME]
clickup-cli view delete <ID>
clickup-cli view tasks <ID>
```

View types: `list`, `board`, `calendar`, `gantt`, `activity`, `map`, `workload`, `table`

---

## member

```bash
clickup-cli member list --task <ID>
clickup-cli member list --list <ID>
```

---

## user

```bash
clickup-cli user invite --email EMAIL [--admin] [--custom-role-id ID]
clickup-cli user get <ID>
clickup-cli user update <ID> [--username NAME] [--admin] [--custom-role-id ID]
clickup-cli user remove <ID>
```

---

## chat (v3)

```bash
# Channels
clickup-cli chat channel-list [--include-closed]
clickup-cli chat channel-create --name NAME [--visibility PUBLIC|PRIVATE]
clickup-cli chat channel-get <ID>
clickup-cli chat channel-update <ID> [--name NAME] [--topic TEXT]
clickup-cli chat channel-delete <ID>
clickup-cli chat channel-followers <ID>
clickup-cli chat channel-members <ID>
clickup-cli chat dm <USER_ID> [USER_ID...]

# Messages
clickup-cli chat message-list --channel <ID>
clickup-cli chat message-send --channel <ID> --text TEXT [--type message|post]
clickup-cli chat message-update <ID> --text TEXT
clickup-cli chat message-delete <ID>

# Reactions and replies
clickup-cli chat reaction-list <MSG_ID>
clickup-cli chat reaction-add <MSG_ID> --emoji NAME
clickup-cli chat reaction-remove <MSG_ID> <EMOJI>
clickup-cli chat reply-list <MSG_ID>
clickup-cli chat reply-send <MSG_ID> --text TEXT
clickup-cli chat tagged-users <MSG_ID>
```

---

## doc (v3)

```bash
clickup-cli doc list [--creator ID] [--archived]
clickup-cli doc create --name NAME [--visibility PUBLIC|PRIVATE|PERSONAL] [--parent-type TYPE --parent-id ID]
clickup-cli doc get <ID>
clickup-cli doc pages <ID> [--content] [--max-depth N]
clickup-cli doc add-page <DOC_ID> --name NAME [--parent-page ID] [--content TEXT]
clickup-cli doc page <DOC_ID> <PAGE_ID>
clickup-cli doc edit-page <DOC_ID> <PAGE_ID> --content TEXT [--mode replace|append|prepend]
clickup-cli doc embed-image <DOC_ID> <PAGE_ID> <FILE> [--via-task TASK_ID] [--alt TEXT] [--mode append|prepend]
```

`embed-image` uploads a local image and embeds it inline in a doc page. The ClickUp API has no doc-level upload, so the image is stored as an attachment on a host task (`--via-task`, auto-detected from the current git branch like other task-scoped commands) and referenced from the page as `![alt](url)` markdown, which ClickUp converts into a native inline image block. `--alt` defaults to the file name. `--mode` is `append` (default) or `prepend`; `replace` is intentionally unsupported — use `edit-page` to rewrite a page. Output shows the CDN url, page_id, and mode; `-q` prints just the URL.

---

## webhook

```bash
clickup-cli webhook list
clickup-cli webhook create --endpoint URL --event EVENT [--event EVENT...] [--space ID | --folder ID | --list ID | --task ID]
clickup-cli webhook update <ID> --endpoint URL --event EVENT --status active
clickup-cli webhook delete <ID>
```

Events: `taskCreated`, `taskUpdated`, `taskDeleted`, `taskStatusUpdated`, `taskCommentPosted`, `taskCommentUpdated`, and more.

---

## template

```bash
clickup-cli template list
clickup-cli template apply-task <TEMPLATE_ID> --list <ID> --name NAME
clickup-cli template apply-list <TEMPLATE_ID> --folder <ID> --name NAME
clickup-cli template apply-list <TEMPLATE_ID> --space <ID> --name NAME
clickup-cli template apply-folder <TEMPLATE_ID> --space <ID> --name NAME
```

---

## guest (Enterprise)

```bash
clickup-cli guest invite --email EMAIL [--can-edit-tags] [--can-see-time-spent] [--can-create-views] [--custom-role-id ID]
clickup-cli guest get <ID>
clickup-cli guest update <ID> [--can-edit-tags] [--can-see-time-spent] [--can-create-views]
clickup-cli guest remove <ID>

# Share/unshare resources with guests
clickup-cli guest share-task <TASK_ID> <GUEST_ID> --permission read|comment|edit|create
clickup-cli guest unshare-task <TASK_ID> <GUEST_ID>
clickup-cli guest share-list <LIST_ID> <GUEST_ID> --permission LEVEL
clickup-cli guest unshare-list <LIST_ID> <GUEST_ID>
clickup-cli guest share-folder <FOLDER_ID> <GUEST_ID> --permission LEVEL
clickup-cli guest unshare-folder <FOLDER_ID> <GUEST_ID>
```

---

## group

```bash
clickup-cli group list
clickup-cli group create --name NAME --member ID [--member ID...]
clickup-cli group update <ID> [--name NAME] [--add-member ID] [--rem-member ID]
clickup-cli group delete <ID>
```

---

## role (Enterprise)

```bash
clickup-cli role list
```

---

## shared

```bash
clickup-cli shared list    # Tasks, lists, and folders shared with you
```

---

## audit-log (Enterprise, v3)

```bash
clickup-cli audit-log query --type TYPE [--user-id ID] [--start-date DATE] [--end-date DATE]
```

Types: `AUTH`, `CUSTOM_FIELDS`, `HIERARCHY`, `USER`, `AGENT`, `OTHER`

---

## acl (Enterprise, v3)

```bash
clickup-cli acl update <OBJECT_TYPE> <OBJECT_ID> [--private] [--grant-user ID --permission LEVEL] [--revoke-user ID] [--body JSON]
```

---

## agent-config

Generate compressed CLI reference for AI agent configs.

```bash
clickup-cli agent-config show              # Print to stdout
clickup-cli agent-config inject            # Inject into CLAUDE.md
clickup-cli agent-config inject path/to/AGENT.md   # Inject into specific file
```

The injected block is delimited with `<!-- clickup-cli:begin -->...<!-- clickup-cli:end -->` and can be updated in place by re-running the command.

---

## Output Examples

### Table (default)
```
+---------+------------------------------+-------------+----------+------------+
| id      | name                         | status      | priority | assignees  |
+==============================================================================+
| abc123  | Migrate database             | Open        | urgent   | Alex Chen  |
| def456  | API rate limiter             | in progress | high     | Sara Jones |
+---------+------------------------------+-------------+----------+------------+
```

### JSON (`--output json`)
Full API response — use for scripting with `jq`.

### JSON Compact (`--output json-compact`)
Only default fields, as JSON:
```json
[{"id":"abc123","name":"Migrate database","status":"Open","priority":"urgent"}]
```

### CSV (`--output csv`)
```
id,name,status,priority,assignees,due_date
abc123,Migrate database,Open,urgent,Alex Chen,-
```

### Quiet (`-q`)
```
abc123
def456
```

---

## status

Show current configuration.

```bash
clickup-cli status
```

```
clickup-cli v{{ site.version }}

Config:    ~/.config/clickup-cli/config.toml
Token:     pk_abc...wxyz
Workspace: 1234567
```

---

## completions

Generate shell completions.

```bash
clickup-cli completions bash       # Bash
clickup-cli completions zsh        # Zsh
clickup-cli completions fish       # Fish
clickup-cli completions powershell # PowerShell
```

See [Installation](install) for setup instructions.

---

## mcp

Start the MCP server for native LLM tool integration. See [MCP Server](mcp) for full documentation.

```bash
clickup-cli mcp serve
```

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Client error (bad input, 400) |
| 2 | Auth/permission error (401, 403) |
| 3 | Not found (404) |
| 4 | Rate limited (429) |
| 5 | Server error (5xx) |

[← Home](.)  ·  [Installation →](install)  ·  [MCP Server →](mcp)
