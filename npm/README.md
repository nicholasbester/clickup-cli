<p align="center">
  <img src="https://raw.githubusercontent.com/nicholasbester/clickup-cli/main/docs/assets/clickup-icon.svg" alt="ClickUp" width="60" />
</p>

<h1 align="center">clickup-cli</h1>

<p align="center">
  CLI for the <a href="https://clickup.com/api/">ClickUp API</a>, optimized for AI agents and human users.<br/>
  ~130 endpoints · 143 MCP tools · ~98% token reduction
</p>

<p align="center">
  <a href="https://nicholasbester.github.io/clickup-cli/">Documentation</a> · <a href="https://github.com/nicholasbester/clickup-cli">GitHub</a> · <a href="https://nicholasbester.github.io/clickup-cli/commands">Command Reference</a> · <a href="https://nicholasbester.github.io/clickup-cli/mcp">MCP Server</a>
</p>

## Why?

ClickUp's API responses are massive — a single task list query returns deeply nested JSON that can consume **12,000+ tokens** for just 5 tasks. For AI agents operating in context windows, this is a serious problem.

clickup-cli solves this with **token-efficient output by default**:

```
Full API JSON (5 tasks):  ~12,000 tokens
clickup-cli output:          ~150 tokens
Reduction:                       ~98%
```

## Install

```bash
npm install -g @nick.bester/clickup-cli
```

## Quick Start

```bash
# Configure (get token from ClickUp Settings > Apps > API Token)
clickup setup --token pk_your_token_here

# Verify
clickup auth whoami

# Navigate workspace
clickup workspace list
clickup space list
clickup folder list --space 12345
clickup list list --folder 67890

# Task management
clickup task list --list 12345
clickup task create --list 12345 --name "My Task" --priority 3
clickup task get abc123
clickup task update abc123 --status "in progress"
clickup task search --status "in progress"
```

## Output Modes

```bash
clickup task list --list 12345                      # Table (default, compact)
clickup task list --list 12345 --output json         # Full API JSON
clickup task list --list 12345 --output json-compact # Flattened JSON
clickup task list --list 12345 --output csv          # CSV
clickup task list --list 12345 -q                    # IDs only
```

## Command Groups (28 resource groups + 4 utilities)

| Category | Commands |
|----------|----------|
| **Core** | setup, auth, workspace, space, folder, list, task |
| **Collaboration** | checklist, comment, tag, field, task-type, attachment |
| **Tracking** | time, goal, view, member, user |
| **Communication** | chat (v3), doc (v3), webhook, template |
| **Admin** | guest, group, role, shared, audit-log, acl |
| **Utilities** | status, completions, agent-config, mcp |

## AI Agent Integration

### CLI Mode (recommended — most token-efficient)

Inject a compressed command reference (~1,000 tokens) into your project's agent instructions:

```bash
clickup agent-config inject   # Auto-detects: CLAUDE.md, agent.md, .cursorrules, etc.
```

### MCP Mode (143 tools with compact responses)

For Claude Desktop, Cursor, and other MCP-capable tools:

```bash
clickup agent-config init --mcp   # Creates .mcp.json at project root
```

Or configure manually — add `.mcp.json` to your project root:

```json
{
  "mcpServers": {
    "clickup-cli": {
      "command": "clickup",
      "args": ["mcp", "serve"]
    }
  }
}
```

## Project-Level Config

```bash
clickup agent-config init --token pk_xxx --workspace 12345 --mcp
```

Creates `.clickup.toml` (auth) + `.mcp.json` (MCP server) in one command. Project config takes priority over global config.

## Shell Completions

```bash
clickup completions bash > ~/.bash_completion.d/clickup
clickup completions zsh > ~/.zfunc/_clickup
clickup completions fish > ~/.config/fish/completions/clickup.fish
```

## Other Install Methods

| Method | Command |
|--------|---------|
| **Homebrew** | `brew tap nicholasbester/clickup-cli && brew install clickup-cli` |
| **Cargo** | `cargo install clickup-cli` |
| **Binary** | Download from [GitHub Releases](https://github.com/nicholasbester/clickup-cli/releases) |

## Links

- **Documentation:** [nicholasbester.github.io/clickup-cli](https://nicholasbester.github.io/clickup-cli/)
- **Command Reference:** [nicholasbester.github.io/clickup-cli/commands](https://nicholasbester.github.io/clickup-cli/commands)
- **MCP Server:** [nicholasbester.github.io/clickup-cli/mcp](https://nicholasbester.github.io/clickup-cli/mcp)
- **GitHub:** [github.com/nicholasbester/clickup-cli](https://github.com/nicholasbester/clickup-cli)

## License

[BSL 1.1](https://github.com/nicholasbester/clickup-cli/blob/main/LICENSE) (Business Source License) — free to use, contributions welcome. Creating competing commercial products requires permission. Converts to Apache 2.0 on 2030-04-01.
