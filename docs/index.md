---
layout: default
title: Home
nav_order: 1
---

# clickup-cli

A command-line interface for the [ClickUp API](https://clickup.com/api/), built in Rust and optimized for AI agents.

## The Problem

ClickUp's API returns deeply nested JSON. A single query for 5 tasks produces **~12,000 tokens** of data — statuses wrapped in objects, assignees as arrays of user objects, timestamps as Unix milliseconds, custom fields, checklists, dependencies, and more.

For AI agents operating within context windows (Claude Code, Cursor, Copilot, etc.), this is a serious problem. A few API calls can consume most of the available context, leaving little room for reasoning.

## The Solution

clickup-cli delivers **token-efficient output by default**:

```
┌─────────────────────────────┬──────────────┐
│ Full API JSON (5 tasks)     │ ~12,000 tokens │
│ clickup-cli table output    │    ~150 tokens │
│ Reduction                   │         ~98%   │
└─────────────────────────────┴──────────────┘
```

The CLI flattens nested objects (`status.status` → `"in progress"`), joins arrays (`assignees[].username` → `"Nick, Bob"`), converts timestamps (`1773652547089` → `"2026-03-17"`), and shows only essential fields.

When you need the full response, `--output json` is always available.

## Coverage

**130 endpoints** across **28 resource groups**, covering the entire ClickUp v2 and v3 API:

| Category | Resources |
|----------|-----------|
| **Core** | workspace, space, folder, list, task |
| **Collaboration** | comment, checklist, tag, field, task-type, attachment |
| **Tracking** | time, goal, view, member, user |
| **Communication** | chat (v3), doc (v3), webhook, template |
| **Admin** | guest, group, role, shared, audit-log, acl |

## Quick Start

```bash
# Install (macOS)
brew tap nicholasbester/clickup-cli
brew install clickup-cli

# Configure
clickup setup --token pk_your_token_here

# Use
clickup task list --list 12345
clickup task create --list 12345 --name "My Task" --priority 3
clickup task get abc123
```

## AI Agent Integration

Two ways to connect AI agents to ClickUp:

**CLI Mode** — inject a command reference into your project's CLAUDE.md:
```bash
clickup agent-config inject
```

**MCP Mode** — run as a native tool server for Claude Desktop, Cursor, etc.:
```json
{
  "mcpServers": {
    "clickup": { "command": "clickup", "args": ["mcp", "serve"] }
  }
}
```

## Features

- **Shell completions** for bash, zsh, fish, powershell
- **Environment variables** — `CLICKUP_TOKEN`, `CLICKUP_WORKSPACE` for CI/CD
- **Smart error handling** — exit codes, hints, plan upgrade suggestions for 403s
- **`clickup status`** — check current config at a glance

[Command Reference →](commands)  ·  [Installation →](install)  ·  [MCP Server →](mcp)  ·  [GitHub →](https://github.com/nicholasbester/clickup-cli)
