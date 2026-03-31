# clickup-cli

A CLI for the [ClickUp API](https://clickup.com/api/), optimized for AI agents and human users. Covers all ~130 endpoints across 28 resource groups.

**[Documentation](https://nicholasbester.github.io/clickup-cli/)**

## Why?

ClickUp's API responses are massive. A single task list query returns deeply nested JSON — statuses, assignees, priorities, custom fields, checklists, dependencies — easily **12,000+ tokens** for just 5 tasks. For AI agents (Claude Code, Cursor, Copilot, etc.) operating within context windows, this is a serious problem: a few API calls can consume most of an agent's available context.

clickup-cli solves this with **token-efficient output by default**:

```
Full API JSON for 5 tasks:  ~12,000 tokens (450 lines)
clickup-cli table output:      ~150 tokens (7 lines)
Reduction:                          ~98%
```

The CLI flattens nested objects, selects only essential fields, and renders compact tables. Agents get the information they need without drowning in JSON. When you need the full response, `--output json` is always available.

Beyond token efficiency, clickup-cli gives AI agents a simple, predictable interface to ClickUp: `clickup <resource> <action> [ID] [flags]`. No SDK, no auth boilerplate, no JSON parsing — just shell commands with structured output.

## Install

### macOS (Homebrew)

```bash
brew tap nicholasbester/clickup-cli
brew install clickup-cli
```

To upgrade to the latest version:
```bash
brew upgrade clickup-cli
```

### macOS / Linux (pre-built binary)

Download the latest release for your platform:

```bash
# macOS Apple Silicon (M1/M2/M3/M4)
curl -L https://github.com/nicholasbester/clickup-cli/releases/latest/download/clickup-macos-arm64.tar.gz | tar xz
sudo mv clickup /usr/local/bin/

# macOS Intel
curl -L https://github.com/nicholasbester/clickup-cli/releases/latest/download/clickup-macos-x86_64.tar.gz | tar xz
sudo mv clickup /usr/local/bin/

# Linux x86_64
curl -L https://github.com/nicholasbester/clickup-cli/releases/latest/download/clickup-linux-x86_64.tar.gz | tar xz
sudo mv clickup /usr/local/bin/

# Linux ARM64
curl -L https://github.com/nicholasbester/clickup-cli/releases/latest/download/clickup-linux-arm64.tar.gz | tar xz
sudo mv clickup /usr/local/bin/
```

### Windows

Download `clickup-windows-x86_64.zip` from the [latest release](https://github.com/nicholasbester/clickup-cli/releases/latest), extract it, and add `clickup.exe` to your PATH.

### From source (any platform)

Requires [Rust](https://rustup.rs/) 1.70+:

```bash
git clone https://github.com/nicholasbester/clickup-cli.git
cd clickup-cli
cargo install --path .
```

### Verify installation

```bash
clickup --version
# clickup 0.5.2
```

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

Two ways to connect AI agents to ClickUp:

### Option 1: CLI Mode (shell commands)

Inject a compressed command reference into your project's CLAUDE.md:

```bash
clickup agent-config inject            # Into CLAUDE.md
clickup agent-config inject AGENT.md   # Into specific file
clickup agent-config show              # Preview the block
```

This adds a ~1,000 token block giving the agent full knowledge of every command. The agent then runs CLI commands directly.

### Option 2: MCP Server (native tool calls)

For Claude Desktop, Cursor, and other MCP-capable tools, run clickup-cli as an MCP server:

```json
{
  "mcpServers": {
    "clickup": {
      "command": "clickup",
      "args": ["mcp", "serve"]
    }
  }
}
```

This exposes 18 tools (task CRUD, search, comments, time tracking, and more) as native tool calls — no shell commands needed. See the [MCP documentation](https://nicholasbester.github.io/clickup-cli/mcp) for full setup.

## Configuration

### Token Resolution (highest priority wins)

1. `--token` CLI flag
2. `CLICKUP_TOKEN` environment variable
3. Config file (`~/.config/clickup-cli/config.toml`)

### Workspace Resolution

1. `--workspace` CLI flag
2. `CLICKUP_WORKSPACE` environment variable
3. Config file default

### Check Current Config

```bash
clickup status
```

```
clickup-cli v0.5.2

Config:    ~/.config/clickup-cli/config.toml
Token:     pk_441...RB4Y
Workspace: 2648001
```

## Shell Completions

```bash
# Bash
clickup completions bash > ~/.bash_completion.d/clickup

# Zsh
clickup completions zsh > ~/.zfunc/_clickup

# Fish
clickup completions fish > ~/.config/fish/completions/clickup.fish

# PowerShell
clickup completions powershell > clickup.ps1
```

## Output Modes

| Flag | Description |
|------|-------------|
| _(default)_ | Aligned table with essential fields |
| `--output json` | Full API response |
| `--output json-compact` | Default fields as JSON |
| `--output csv` | CSV format |
| `-q` / `--quiet` | IDs only, one per line |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Client error (bad input) |
| 2 | Auth/permission error (401, 403) |
| 3 | Not found (404) |
| 4 | Rate limited (429) |
| 5 | Server error (5xx) |

## License

Apache-2.0
