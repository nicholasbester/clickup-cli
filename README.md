# clickup-cli

A CLI for the [ClickUp API](https://clickup.com/api/), optimized for AI agents and human users.

## Install

```bash
cargo install --path .
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

## Usage

```bash
# List workspaces
clickup workspace list

# List spaces
clickup space list

# List tasks in a list
clickup task list --list 12345

# Create a task
clickup task create --list 12345 --name "My Task" --priority 3

# Get task details
clickup task get abc123

# Search tasks
clickup task search --status "in progress"

# JSON output for scripting
clickup task list --list 12345 --output json

# IDs only for piping
clickup task list --list 12345 -q
```

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
