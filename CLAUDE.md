# clickup-cli

Rust CLI for the ClickUp API, optimized for AI agent consumption.

## Build & Test

```bash
cargo build                    # Build
cargo test                     # Run all tests
cargo test --test test_cli     # CLI smoke tests only
cargo test --test test_output  # Output formatting tests only
cargo run -- --help            # Show help
```

## Architecture

- Single binary: `clickup`
- Entry: `src/main.rs` → `src/lib.rs`
- Commands: `src/commands/{resource}.rs` — one file per API resource group
- Models: `src/models/{resource}.rs` — serde structs for API responses
- Core: `src/client.rs` (HTTP), `src/config.rs` (TOML), `src/output.rs` (formatting), `src/error.rs` (errors)

## CLI Pattern

```
clickup <resource> <action> [ID] [flags]
```

Resources: setup, auth, workspace, space, folder, list, task

## Global Flags

- `--token TOKEN` — override config file token
- `--workspace ID` — override default workspace
- `--output MODE` — table (default), json, json-compact, csv
- `--fields LIST` — comma-separated field names
- `--no-header` — omit table header
- `--all` — fetch all pages
- `--limit N` — cap results
- `--page N` — manual page
- `-q` / `--quiet` — IDs only
- `--timeout SECS` — HTTP timeout (default 30)

## Config

Location: `~/.config/clickup-cli/config.toml`

```toml
[auth]
token = "pk_..."

[defaults]
workspace_id = "12345"
```

## Exit Codes

- 0: success
- 1: client error (400, bad input)
- 2: auth error (401, no token)
- 3: not found (404)
- 4: rate limited (429)
- 5: server error (5xx)

## Key API Notes

- "team_id" in v2 = workspace_id
- All timestamps are Unix milliseconds
- Priority: 1=Urgent, 2=High, 3=Normal, 4=Low
- task_count on folders is a string, not integer
