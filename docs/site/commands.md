---
layout: default
title: Command Reference
---

# Command Reference

Pattern: `clickup <resource> <action> [ID] [flags]`

## Global Flags

| Flag | Description |
|------|-------------|
| `--output MODE` | `table` (default), `json`, `json-compact`, `csv` |
| `--fields LIST` | Comma-separated field names to display |
| `--no-header` | Omit table header row |
| `-q` / `--quiet` | Print IDs only, one per line |
| `--all` | Fetch all pages (auto-paginate) |
| `--limit N` | Cap total results |
| `--page N` | Manual page selection |
| `--token TOKEN` | Override config file token |
| `--workspace ID` | Override default workspace |
| `--timeout SECS` | HTTP timeout (default 30) |

---

## setup

Configure API token and default workspace.

```bash
# Interactive
clickup setup

# Non-interactive
clickup setup --token pk_your_token_here
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
clickup auth whoami    # Show current user
clickup auth check     # Validate token (exit code only)
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
clickup workspace list     # List workspaces
clickup workspace seats    # Show seat usage
clickup workspace plan     # Show current plan
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
clickup space list [--archived]
clickup space get <ID>
clickup space create --name NAME [--private] [--multiple-assignees]
clickup space update <ID> [--name NAME] [--color HEX]
clickup space delete <ID>
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
clickup folder list --space <ID> [--archived]
clickup folder get <ID>
clickup folder create --space <ID> --name NAME
clickup folder update <ID> --name NAME
clickup folder delete <ID>
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
clickup list list --folder <ID> [--archived]
clickup list list --space <ID> [--archived]    # folderless lists
clickup list get <ID>
clickup list create --folder <ID> --name NAME [--content TEXT] [--priority N] [--due-date DATE]
clickup list create --space <ID> --name NAME   # folderless
clickup list update <ID> [--name NAME] [--content TEXT]
clickup list delete <ID>
clickup list add-task <LIST_ID> <TASK_ID>
clickup list remove-task <LIST_ID> <TASK_ID>
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
clickup task list --list <ID> [--status S] [--assignee ID] [--tag T] [--include-closed] [--order-by field] [--reverse]
clickup task search [--space ID] [--folder ID] [--list ID] [--status S] [--assignee ID] [--tag T]

# CRUD
clickup task get <ID> [--subtasks] [--custom-task-id]
clickup task create --list <ID> --name NAME [--description TEXT] [--status S] [--priority 1-4] [--assignee ID] [--tag NAME] [--due-date DATE] [--parent TASK_ID]
clickup task update <ID> [--name X] [--status X] [--priority N] [--add-assignee ID] [--rem-assignee ID] [--description TEXT]
clickup task delete <ID>

# Relationships and tags
clickup task add-dep <ID> --depends-on <OTHER_ID>
clickup task remove-dep <ID> --depends-on <OTHER_ID>
clickup task link <ID> <TARGET_ID>
clickup task unlink <ID> <TARGET_ID>
clickup task add-tag <ID> <TAG_NAME>
clickup task remove-tag <ID> <TAG_NAME>

# Time and estimates
clickup task time-in-status <ID>...
clickup task move <ID> --list <LIST_ID>
clickup task set-estimate <ID> --assignee USER_ID --time MS
clickup task replace-estimates <ID> --assignee USER_ID --time MS
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
clickup checklist create --task <ID> --name NAME
clickup checklist update <ID> [--name NAME] [--position N]
clickup checklist delete <ID>
clickup checklist add-item <ID> --name NAME [--assignee USER_ID]
clickup checklist update-item <ID> <ITEM_ID> [--name NAME] [--resolved] [--assignee USER_ID]
clickup checklist delete-item <ID> <ITEM_ID>
```

---

## comment

```bash
clickup comment list --task <ID>          # also --list, --view
clickup comment create --task <ID> --text TEXT [--assignee ID] [--notify-all]
clickup comment create --list <ID> --text TEXT
clickup comment create --view <ID> --text TEXT
clickup comment update <ID> --text TEXT [--resolved] [--assignee ID]
clickup comment delete <ID>
clickup comment replies <ID>              # list threaded replies
clickup comment reply <ID> --text TEXT [--assignee ID]
```

---

## tag

```bash
clickup tag list --space <ID>
clickup tag create --space <ID> --name NAME [--fg-color HEX] [--bg-color HEX]
clickup tag update --space <ID> --tag NAME [--name NEW_NAME] [--fg-color HEX] [--bg-color HEX]
clickup tag delete --space <ID> --tag NAME
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
clickup field list --list <ID>            # also --folder, --space, --workspace-level
clickup field set <TASK_ID> <FIELD_ID> --value VALUE
clickup field unset <TASK_ID> <FIELD_ID>
```

Value can be a string, number, or JSON for complex field types.

---

## task-type

```bash
clickup task-type list
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
clickup attachment list --task <ID>
clickup attachment upload --task <ID> <FILE_PATH>
```

---

## time

Time tracking with start/stop timer support.

```bash
# Timer
clickup time start [--task ID] [--description TEXT] [--billable]
clickup time stop

# CRUD
clickup time list [--start-date DATE] [--end-date DATE] [--assignee ID] [--task ID]
clickup time get <ID>
clickup time current
clickup time create --start DATE --duration MS [--task ID] [--description TEXT] [--billable]
clickup time update <ID> [--start DATE] [--end DATE] [--description TEXT] [--billable]
clickup time delete <ID>

# Tags
clickup time tags
clickup time add-tags --entry-id ID --tag NAME [--tag NAME...]
clickup time remove-tags --entry-id ID --tag NAME
clickup time rename-tag --name OLD --new-name NEW

# History
clickup time history <ID>
```

---

## goal

```bash
clickup goal list [--include-completed]
clickup goal get <ID>
clickup goal create --name NAME --due-date DATE --description TEXT [--color HEX] [--owner ID]
clickup goal update <ID> [--name NAME] [--due-date DATE] [--add-owner ID] [--rem-owner ID]
clickup goal delete <ID>

# Key Results
clickup goal add-kr <GOAL_ID> --name NAME --type TYPE --steps-start N --steps-end N [--unit UNIT] [--owner ID]
clickup goal update-kr <KR_ID> --steps-current N [--note TEXT]
clickup goal delete-kr <KR_ID>
```

Key result types: `number`, `currency`, `boolean`, `percentage`, `automatic`

---

## view

```bash
clickup view list --workspace-level       # also --space, --folder, --list
clickup view get <ID>
clickup view create --name NAME --type TYPE --space <ID>   # also --folder, --list, --workspace-level
clickup view update <ID> [--name NAME]
clickup view delete <ID>
clickup view tasks <ID> [--page N]
```

View types: `list`, `board`, `calendar`, `gantt`, `activity`, `map`, `workload`, `table`

---

## member

```bash
clickup member list --task <ID>
clickup member list --list <ID>
```

---

## user

```bash
clickup user invite --email EMAIL [--admin] [--custom-role-id ID]
clickup user get <ID>
clickup user update <ID> [--username NAME] [--admin] [--custom-role-id ID]
clickup user remove <ID>
```

---

## chat (v3)

```bash
# Channels
clickup chat channel-list [--include-closed]
clickup chat channel-create --name NAME [--visibility PUBLIC|PRIVATE]
clickup chat channel-get <ID>
clickup chat channel-update <ID> [--name NAME] [--topic TEXT]
clickup chat channel-delete <ID>
clickup chat channel-followers <ID>
clickup chat channel-members <ID>
clickup chat dm <USER_ID> [USER_ID...]

# Messages
clickup chat message-list --channel <ID>
clickup chat message-send --channel <ID> --text TEXT [--type message|post]
clickup chat message-update <ID> --text TEXT
clickup chat message-delete <ID>

# Reactions and replies
clickup chat reaction-list <MSG_ID>
clickup chat reaction-add <MSG_ID> --emoji NAME
clickup chat reaction-remove <MSG_ID> <EMOJI>
clickup chat reply-list <MSG_ID>
clickup chat reply-send <MSG_ID> --text TEXT
clickup chat tagged-users <MSG_ID>
```

---

## doc (v3)

```bash
clickup doc list [--creator ID] [--archived]
clickup doc create --name NAME [--visibility PUBLIC|PRIVATE|PERSONAL] [--parent-type TYPE --parent-id ID]
clickup doc get <ID>
clickup doc pages <ID> [--content] [--max-depth N]
clickup doc add-page <DOC_ID> --name NAME [--parent-page ID] [--content TEXT]
clickup doc page <DOC_ID> <PAGE_ID>
clickup doc edit-page <DOC_ID> <PAGE_ID> --content TEXT [--mode replace|append|prepend]
```

---

## webhook

```bash
clickup webhook list
clickup webhook create --endpoint URL --event EVENT [--event EVENT...] [--space ID | --folder ID | --list ID | --task ID]
clickup webhook update <ID> --endpoint URL --event EVENT --status active
clickup webhook delete <ID>
```

Events: `taskCreated`, `taskUpdated`, `taskDeleted`, `taskStatusUpdated`, `taskCommentPosted`, `taskCommentUpdated`, and more.

---

## template

```bash
clickup template list [--page N]
clickup template apply-task <TEMPLATE_ID> --list <ID> --name NAME
clickup template apply-list <TEMPLATE_ID> --folder <ID> --name NAME
clickup template apply-list <TEMPLATE_ID> --space <ID> --name NAME
clickup template apply-folder <TEMPLATE_ID> --space <ID> --name NAME
```

---

## guest (Enterprise)

```bash
clickup guest invite --email EMAIL [--can-edit-tags] [--can-see-time-spent] [--can-create-views] [--custom-role-id ID]
clickup guest get <ID>
clickup guest update <ID> [--can-edit-tags] [--can-see-time-spent] [--can-create-views]
clickup guest remove <ID>

# Share/unshare resources with guests
clickup guest share-task <TASK_ID> <GUEST_ID> --permission read|comment|edit|create
clickup guest unshare-task <TASK_ID> <GUEST_ID>
clickup guest share-list <LIST_ID> <GUEST_ID> --permission LEVEL
clickup guest unshare-list <LIST_ID> <GUEST_ID>
clickup guest share-folder <FOLDER_ID> <GUEST_ID> --permission LEVEL
clickup guest unshare-folder <FOLDER_ID> <GUEST_ID>
```

---

## group

```bash
clickup group list
clickup group create --name NAME --member ID [--member ID...]
clickup group update <ID> [--name NAME] [--add-member ID] [--rem-member ID]
clickup group delete <ID>
```

---

## role (Enterprise)

```bash
clickup role list
```

---

## shared

```bash
clickup shared list    # Tasks, lists, and folders shared with you
```

---

## audit-log (Enterprise, v3)

```bash
clickup audit-log query --type TYPE [--user-id ID] [--start-date DATE] [--end-date DATE]
```

Types: `AUTH`, `CUSTOM_FIELDS`, `HIERARCHY`, `USER`, `AGENT`, `OTHER`

---

## acl (Enterprise, v3)

```bash
clickup acl update <OBJECT_TYPE> <OBJECT_ID> [--private] [--grant-user ID --permission LEVEL] [--revoke-user ID] [--body JSON]
```

---

## agent-config

Generate compressed CLI reference for AI agent configs.

```bash
clickup agent-config show              # Print to stdout
clickup agent-config inject            # Inject into CLAUDE.md
clickup agent-config inject path/to/AGENT.md   # Inject into specific file
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

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Client error (bad input, 400) |
| 2 | Auth error (401, no token) |
| 3 | Not found (404) |
| 4 | Rate limited (429) |
| 5 | Server error (5xx) |

[← Home](.)  ·  [Installation →](install)
