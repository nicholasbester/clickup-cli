# clickup-cli

CLI for the [ClickUp API](https://clickup.com/api/), optimized for AI agents. Covers all ~130 endpoints with token-efficient output (~98% smaller than raw JSON).

## Install

```bash
npm install -g clickup-cli
```

## Quick Start

```bash
clickup setup --token pk_your_token_here
clickup auth whoami
clickup task list --list 12345
```

## Features

- **130 API endpoints** across 28 resource groups
- **Token-efficient output** — tables ~98% smaller than raw JSON
- **MCP server** — 143 tools for Claude Desktop, Cursor, etc.
- **LLM-agnostic** — works with any AI agent framework

## Documentation

[nicholasbester.github.io/clickup-cli](https://nicholasbester.github.io/clickup-cli/)

## Other Install Methods

- **Homebrew:** `brew tap nicholasbester/clickup-cli && brew install clickup-cli`
- **Cargo:** `cargo install clickup-cli`
- **Binary:** [GitHub Releases](https://github.com/nicholasbester/clickup-cli/releases)

## License

BSL 1.1 (Business Source License) — free to use, contributions welcome. See [LICENSE](https://github.com/nicholasbester/clickup-cli/blob/main/LICENSE).
