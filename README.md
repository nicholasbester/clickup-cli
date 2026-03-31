# clickup-cli

A CLI for the [ClickUp API](https://clickup.com/api/), optimized for AI agents and human users. Covers all ~130 endpoints across 28 resource groups.

## Install

**macOS (Homebrew):**
```bash
brew tap nicholasbester/clickup-cli
brew install clickup-cli
```

**From source (any platform with Rust):**
```bash
cargo install --path .
```

**Pre-built binaries:**

Download from [GitHub Releases](https://github.com/nicholasbester/clickup-cli/releases) for macOS (Apple Silicon, Intel), Linux (x86_64, ARM), and Windows.

## Quick Start

```bash
# Configure your API token
clickup setup

# Or non-interactive
clickup setup --token pk_your_token_here

# Verify
clickup auth whoami
```

## Usage Examples

```bash
# Hierarchy navigation
clickup workspace list
clickup space list
clickup folder list --space 12345
clickup list list --folder 67890

# Task management
clickup task list --list 12345
clickup task create --list 12345 --name "My Task" --priority 3
clickup task get abc123
clickup task update abc123 --status "in progress"
clickup task search --status "in progress" --assignee 44106202

# Comments and collaboration
clickup comment list --task abc123
clickup comment create --task abc123 --text "Looking good!"
clickup comment reply COMMENT_ID --text "Thanks!"

# Time tracking
clickup time start --task abc123 --description "Working on feature"
clickup time stop
clickup time list --start-date 2026-03-01 --end-date 2026-03-31

# Goals and views
clickup goal list
clickup view list --space 12345
clickup view tasks VIEW_ID

# Tags and custom fields
clickup tag list --space 12345
clickup field list --list 12345
clickup field set TASK_ID FIELD_ID --value "some value"

# Chat (v3)
clickup chat channel-list
clickup chat message-send --channel CHAN_ID --text "Hello team"

# Docs (v3)
clickup doc list
clickup doc get DOC_ID

# Output modes
clickup task list --list 12345 --output json        # Full JSON
clickup task list --list 12345 --output json-compact # Default fields as JSON
clickup task list --list 12345 --output csv          # CSV
clickup task list --list 12345 -q                    # IDs only
clickup task list --list 12345 --fields id,name,status  # Custom fields
```

## Command Groups

| Group | Commands |
|-------|----------|
| `setup` | Configure token and workspace |
| `auth` | whoami, check |
| `workspace` | list, seats, plan |
| `space` | list, get, create, update, delete |
| `folder` | list, get, create, update, delete |
| `list` | list, get, create, update, delete, add-task, remove-task |
| `task` | list, search, get, create, update, delete, time-in-status, add-tag, remove-tag, add-dep, remove-dep, link, unlink, move, set-estimate, replace-estimates |
| `checklist` | create, update, delete, add-item, update-item, delete-item |
| `comment` | list, create, update, delete, replies, reply |
| `tag` | list, create, update, delete |
| `field` | list, set, unset |
| `task-type` | list |
| `attachment` | list, upload |
| `time` | list, get, current, create, update, delete, start, stop, tags, add-tags, remove-tags, rename-tag, history |
| `goal` | list, get, create, update, delete, add-kr, update-kr, delete-kr |
| `view` | list, get, create, update, delete, tasks |
| `member` | list |
| `user` | invite, get, update, remove |
| `chat` | channel-list, channel-create, channel-get, channel-update, channel-delete, dm, message-list, message-send, message-update, message-delete, reaction-list, reaction-add, reaction-remove, reply-list, reply-send, and more |
| `doc` | list, create, get, pages, add-page, page, edit-page |
| `webhook` | list, create, update, delete |
| `template` | list, apply-task, apply-list, apply-folder |
| `guest` | invite, get, update, remove, share-task, unshare-task, share-list, unshare-list, share-folder, unshare-folder |
| `group` | list, create, update, delete |
| `role` | list |
| `shared` | list |
| `audit-log` | query |
| `acl` | update |

## AI Agent Integration

The CLI is designed for AI agents (Claude Code, Cursor, etc.). The default table output is ~98% smaller than raw JSON, saving thousands of tokens per call.

To make the CLI discoverable by AI agents in any project, inject a compressed command reference into the project's CLAUDE.md:

```bash
# Inject into current project's CLAUDE.md
clickup agent-config inject

# Inject into a specific file
clickup agent-config inject path/to/AGENT.md

# Preview the reference block
clickup agent-config show
```

This adds a single-line `<!-- clickup-cli:begin -->...<!-- clickup-cli:end -->` block containing all commands. Re-running the command updates the block in place. The block is ~1,000 tokens and gives the agent full knowledge of every available command and flag.

## Output Modes

| Flag | Description |
|------|-------------|
| _(default)_ | Aligned table with essential fields |
| `--output json` | Full API response |
| `--output json-compact` | Default fields as JSON |
| `--output csv` | CSV format |
| `-q` / `--quiet` | IDs only, one per line |

## License

Apache-2.0
