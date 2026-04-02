<p align="center">
  <img src="docs/assets/clickup-cli-logo.svg" alt="clickup-cli" width="80" />
</p>

<h1 align="center">clickup-cli</h1>

<p align="center">
  A CLI for the <a href="https://clickup.com/api/">ClickUp API</a>, optimized for AI agents and human users.<br/>
  Covers all ~130 endpoints across 28 resource groups and 4 utility commands.
</p>

<p align="center">
  <a href="https://crates.io/crates/clickup-cli"><img src="https://img.shields.io/crates/v/clickup-cli" alt="crates.io" /></a>
  <a href="https://www.npmjs.com/package/@nick.bester/clickup-cli"><img src="https://img.shields.io/npm/v/@nick.bester/clickup-cli" alt="npm" /></a>
  <a href="https://github.com/nicholasbester/clickup-cli/releases"><img src="https://img.shields.io/github/v/release/nicholasbester/clickup-cli" alt="GitHub release" /></a>
  <a href="https://github.com/nicholasbester/clickup-cli/actions"><img src="https://img.shields.io/github/actions/workflow/status/nicholasbester/clickup-cli/ci.yml" alt="CI" /></a>
  <a href="https://glama.ai/mcp/servers/nicholasbester/clickup-cli"><img src="https://glama.ai/mcp/servers/nicholasbester/clickup-cli/badges/score.svg" alt="Glama MCP" /></a>
  <a href="https://nicholasbester.github.io/clickup-cli/">Documentation</a>
</p>

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

### npm (any platform with Node.js)

```bash
npm install -g @nick.bester/clickup-cli
```

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

### From crates.io (any platform)

Requires [Rust](https://rustup.rs/) 1.70+:

```bash
cargo install clickup-cli
```

### Docker

```bash
docker build -t clickup-cli .
docker run -i --rm -e CLICKUP_TOKEN=pk_xxx -e CLICKUP_WORKSPACE=12345 clickup-cli mcp serve
```

### From source

```bash
git clone https://github.com/nicholasbester/clickup-cli.git
cd clickup-cli
cargo install --path .
```

### Verify installation

```bash
clickup --version
# clickup 0.6.1
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
| **Utilities** | |
| `status` | Show current config, token (masked), workspace |
| `completions` | Generate shell completions (bash, zsh, fish, powershell) |
| `agent-config` | show, inject — CLI reference for AI agent configs |
| `mcp` | serve — MCP server for native LLM tool integration |

## AI Agent Integration

Two ways to connect AI agents to ClickUp:

### Recommended: CLI Mode (shell commands)

The CLI approach is **the most token-efficient way** to give an agent ClickUp access. Injecting the command reference costs ~1,000 tokens once, and every command returns compact table output (~150 tokens for 5 tasks). There are no tool schemas consuming context. Works with any LLM/agent framework.

```bash
clickup agent-config inject            # Auto-detects: CLAUDE.md, agent.md, .cursorrules, etc.
clickup agent-config inject AGENT.md   # Or specify any file explicitly
clickup agent-config show              # Preview the block
```

Auto-detection checks for existing files in order: `CLAUDE.md`, `agent.md`, `AGENT.md`, `.cursorrules`, `.github/copilot-instructions.md`. Falls back to creating `CLAUDE.md` if none exist.

The agent then runs CLI commands directly — the full ClickUp API in ~1,000 tokens of instructions.

### Alternative: MCP Server (native tool calls)

For Claude Desktop, Cursor, and other MCP-capable tools that prefer native tool integration. Note: MCP tool schemas consume more tokens in the agent's context than the CLI reference approach.

Generate the MCP config automatically:

```bash
clickup agent-config init --mcp
```

Or add `.mcp.json` to your project root manually:

```json
{
  "mcpServers": {
    "clickup-cli": {
      "command": "/opt/homebrew/bin/clickup",
      "args": ["mcp", "serve"]
    }
  }
}
```

This exposes 143 tools covering 100% of the ClickUp API as native tool calls with token-efficient compact responses. See the [MCP documentation](https://nicholasbester.github.io/clickup-cli/mcp) for full setup.

## Configuration

### Config Files

| Level | File | Use case |
|-------|------|----------|
| **Project** | `.clickup.toml` | Per-project token/workspace (team repos, CI) |
| **Global** | `~/.config/clickup-cli/config.toml` | Personal default |

Create a project-level config:
```bash
clickup agent-config init --token pk_xxx --workspace 12345
```

This creates `.clickup.toml` in the current directory. Add it to `.gitignore` if it contains a token. Project config takes priority over global config.

### Token Resolution (highest priority wins)

1. `--token` CLI flag
2. `CLICKUP_TOKEN` environment variable
3. `.clickup.toml` (project-level)
4. `~/.config/clickup-cli/config.toml` (global)

### Workspace Resolution

1. `--workspace` CLI flag
2. `CLICKUP_WORKSPACE` environment variable
3. `.clickup.toml` (project-level)
4. `~/.config/clickup-cli/config.toml` (global)

### Check Current Config

```bash
clickup status
```

```
clickup-cli v0.6.1

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

[Apache-2.0](LICENSE)
