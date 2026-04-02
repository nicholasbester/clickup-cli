---
layout: default
title: MCP Server
nav_order: 4
---

# MCP Server

clickup-cli includes a built-in [Model Context Protocol](https://modelcontextprotocol.io/) server, allowing LLMs to interact with ClickUp through structured tool calls instead of shell commands.

**143 tools** covering 100% of the ClickUp API — every endpoint available via CLI is also available as an MCP tool.

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

### Claude Code

Add `.mcp.json` to your project root:

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

Or generate it automatically:

```bash
clickup agent-config init --mcp
```

**Note:** Use the full path to `clickup` (run `which clickup` to find it). Use `clickup-cli` as the server name to avoid conflicts with other ClickUp MCP integrations.

### Docker

Run the MCP server as a Docker container:

```bash
# Build
docker build -t clickup-cli .

# Run MCP server (stdio)
docker run -i --rm \
  -v ~/.config/clickup-cli:/root/.config/clickup-cli \
  clickup-cli mcp serve
```

Or configure with environment variable:

```bash
docker run -i --rm \
  -e CLICKUP_TOKEN=pk_your_token \
  -e CLICKUP_WORKSPACE=your_workspace_id \
  clickup-cli mcp serve
```

Use in `.mcp.json`:

```json
{
  "mcpServers": {
    "clickup-cli": {
      "command": "docker",
      "args": ["run", "-i", "--rm", "-e", "CLICKUP_TOKEN=pk_your_token", "-e", "CLICKUP_WORKSPACE=your_workspace_id", "clickup-cli", "mcp", "serve"]
    }
  }
}
```

### Prerequisites

Run `clickup setup --token pk_your_token` first, or create a project-level `.clickup.toml`:

```bash
clickup agent-config init --token pk_your_token --workspace 12345 --mcp
```

This creates both `.clickup.toml` (auth config) and `.mcp.json` (MCP server config) in one command.

## Available Tools (143)

| Category | Tools | Count |
|----------|-------|-------|
| **Auth** | whoami, auth_check | 2 |
| **Workspaces** | workspace_list, workspace_seats, workspace_plan | 3 |
| **Spaces** | space_list, space_get, space_create, space_update, space_delete | 5 |
| **Folders** | folder_list, folder_get, folder_create, folder_update, folder_delete | 5 |
| **Lists** | list_list, list_get, list_create, list_update, list_delete, list_add_task, list_remove_task | 7 |
| **Tasks** | task_list, task_get, task_create, task_update, task_delete, task_search, task_time_in_status, task_move, task_set_estimate, task_replace_estimates, task_add_dep, task_remove_dep, task_link, task_unlink, task_add_tag, task_remove_tag | 16 |
| **Checklists** | checklist_create, checklist_update, checklist_delete, checklist_add_item, checklist_update_item, checklist_delete_item | 6 |
| **Comments** | comment_list, comment_create, comment_update, comment_delete, comment_replies, comment_reply | 6 |
| **Tags** | tag_list, tag_create, tag_update, tag_delete | 4 |
| **Custom Fields** | field_list, field_set, field_unset | 3 |
| **Task Types** | task_type_list | 1 |
| **Attachments** | attachment_list, attachment_upload | 2 |
| **Time Tracking** | time_list, time_get, time_create, time_update, time_delete, time_start, time_stop, time_current, time_tags, time_add_tags, time_remove_tags, time_rename_tag, time_history | 13 |
| **Goals** | goal_list, goal_get, goal_create, goal_update, goal_delete, goal_add_kr, goal_update_kr, goal_delete_kr | 8 |
| **Views** | view_list, view_get, view_create, view_update, view_delete, view_tasks | 6 |
| **Members** | member_list | 1 |
| **Users** | user_get, user_invite, user_update, user_remove | 4 |
| **Chat (v3)** | chat_channel_list, chat_channel_create, chat_channel_get, chat_channel_update, chat_channel_delete, chat_channel_followers, chat_channel_members, chat_dm, chat_message_list, chat_message_send, chat_message_update, chat_message_delete, chat_reaction_list, chat_reaction_add, chat_reaction_remove, chat_reply_list, chat_reply_send, chat_tagged_users | 18 |
| **Docs (v3)** | doc_list, doc_get, doc_create, doc_pages, doc_get_page, doc_add_page, doc_edit_page | 7 |
| **Webhooks** | webhook_list, webhook_create, webhook_update, webhook_delete | 4 |
| **Templates** | template_list, template_apply_task, template_apply_list, template_apply_folder | 4 |
| **Guests** | guest_get, guest_invite, guest_update, guest_remove, guest_share_task, guest_unshare_task, guest_share_list, guest_unshare_list, guest_share_folder, guest_unshare_folder | 10 |
| **Groups** | group_list, group_create, group_update, group_delete | 4 |
| **Roles** | role_list | 1 |
| **Shared** | shared_list | 1 |
| **Audit Logs** | audit_log_query | 1 |
| **ACLs** | acl_update | 1 |

All tool names are prefixed with `clickup_` (e.g., `clickup_task_list`).

## How It Works

The MCP server uses JSON-RPC 2.0 over stdio. It reads requests from stdin and writes responses to stdout. The server uses the same HTTP client and authentication as the CLI commands, and returns **token-efficient compact responses** — the same field flattening as the CLI's table output, but as JSON. Status objects, priority objects, assignee arrays, and timestamps are all flattened to simple values.

```
LLM ↔ JSON-RPC (stdio) ↔ clickup mcp serve ↔ ClickUp API
                                ↓
                        Compact JSON response
                   (flattened, essential fields only)
```

## CLI vs MCP

| | CLI Mode (recommended) | MCP Mode |
|---|---|---|
| **Setup cost** | ~1,000 tokens (once) | 143 tool schemas loaded into context |
| **Setup** | `clickup agent-config inject` | Add to MCP server config |
| **Output** | Token-efficient tables (default) | Token-efficient compact JSON |
| **Integration** | Shell commands via agent | Native tool calls |
| **Coverage** | All ~130 endpoints | All ~130 endpoints (143 tools) |
| **Best for** | Claude Code, shell-based agents | Claude Desktop, Cursor, VS Code |

**The CLI approach is preferred for token efficiency** — it costs ~1,000 tokens once for the full command reference, while MCP tool schemas consume significantly more context per session. Both have 100% API coverage with token-efficient output.

Use MCP when your tool requires native tool integration (e.g., Claude Desktop doesn't run shell commands).

Both modes deliver ~98% token reduction compared to raw API JSON. Both use the same authentication and config file.

[← Command Reference](commands)  ·  [Home →](.)
