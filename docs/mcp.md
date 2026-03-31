---
layout: default
title: MCP Server
nav_order: 4
---

# MCP Server

clickup-cli includes a built-in [Model Context Protocol](https://modelcontextprotocol.io/) server, allowing LLMs to interact with ClickUp through structured tool calls instead of shell commands.

## Setup

### Claude Desktop

Add to your Claude Desktop config (`~/Library/Application Support/Claude/claude_desktop_config.json`):

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

### Cursor

Add to your Cursor MCP settings:

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

### Prerequisites

Run `clickup setup --token pk_your_token` first. The MCP server reads the token and default workspace from your config file.

## Available Tools

The MCP server exposes 18 tools:

### Auth
- **clickup_whoami** — Get the authenticated user

### Hierarchy
- **clickup_workspace_list** — List workspaces
- **clickup_space_list** — List spaces in a workspace
- **clickup_folder_list** — List folders in a space
- **clickup_list_list** — List lists in a folder or space

### Tasks
- **clickup_task_list** — List tasks in a list (with optional status/assignee filters)
- **clickup_task_get** — Get task details (with optional subtasks)
- **clickup_task_create** — Create a task (name, description, status, priority, assignees, tags, due date)
- **clickup_task_update** — Update a task (name, status, priority, description, assignees)
- **clickup_task_delete** — Delete a task
- **clickup_task_search** — Search tasks across a workspace

### Comments
- **clickup_comment_list** — List comments on a task
- **clickup_comment_create** — Add a comment to a task

### Custom Fields
- **clickup_field_list** — List custom fields for a list
- **clickup_field_set** — Set a custom field value on a task

### Time Tracking
- **clickup_time_start** — Start a time tracking timer
- **clickup_time_stop** — Stop the running timer
- **clickup_time_list** — List time entries

## How It Works

The MCP server uses JSON-RPC 2.0 over stdio. It reads requests from stdin and writes responses to stdout. The server uses the same HTTP client and authentication as the CLI commands, but returns full JSON responses for maximum flexibility.

```
LLM ↔ JSON-RPC (stdio) ↔ clickup mcp serve ↔ ClickUp API
```

## CLI vs MCP

| | CLI Mode | MCP Mode |
|---|---|---|
| **Setup** | `clickup agent-config inject` into CLAUDE.md | Add to MCP server config |
| **Output** | Token-efficient tables (default) or JSON | Full JSON (structured) |
| **Integration** | Shell commands via agent | Native tool calls |
| **Best for** | Claude Code, shell-based agents | Claude Desktop, Cursor, VS Code |

Both modes use the same authentication and config file. You can use both simultaneously.

[← Command Reference](commands)  ·  [Home →](.)
