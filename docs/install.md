---
layout: default
title: Installation
nav_order: 2
---

# Installation

## npm (any platform with Node.js)

```bash
npm install -g @nick.bester/clickup-cli
```

## macOS (Homebrew)

```bash
brew tap nicholasbester/clickup-cli
brew install clickup-cli
```

Upgrade: `brew upgrade clickup-cli`

## macOS / Linux (pre-built binary)

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

## Windows

Download `clickup-windows-x86_64.zip` from the [latest release](https://github.com/nicholasbester/clickup-cli/releases/latest), extract, and add `clickup.exe` to your PATH.

## From crates.io

Requires [Rust](https://rustup.rs/) 1.70+:

```bash
cargo install clickup-cli
```

## From source

```bash
git clone https://github.com/nicholasbester/clickup-cli.git
cd clickup-cli
cargo install --path .
```

## Setup

Get a personal API token from ClickUp: **Settings > Apps > API Token**

```bash
# Interactive
clickup setup

# Non-interactive (for CI/scripts/agents)
clickup setup --token pk_your_token_here
```

Config is saved to `~/.config/clickup-cli/config.toml`.

## Verify

```bash
clickup --version       # Should show: clickup 0.6.1
clickup auth whoami
clickup status          # Show config, token (masked), workspace
```

## Shell Completions

```bash
# Bash
clickup completions bash > ~/.bash_completion.d/clickup

# Zsh (add ~/.zfunc to fpath in .zshrc first)
clickup completions zsh > ~/.zfunc/_clickup

# Fish
clickup completions fish > ~/.config/fish/completions/clickup.fish

# PowerShell
clickup completions powershell > clickup.ps1
```

## Project-Level Config

For per-project settings (different workspace, different token), create a `.clickup.toml` in the project root:

```bash
clickup agent-config init --token pk_xxx --workspace 12345
```

This creates:
```toml
[auth]
token = "pk_xxx"

[defaults]
workspace_id = "12345"
```

Project config (`.clickup.toml`) takes priority over global config. Add it to `.gitignore` if it contains a token.

## Environment Variables

For CI/CD and scripting:

| Variable | Description |
|----------|-------------|
| `CLICKUP_TOKEN` | API token |
| `CLICKUP_WORKSPACE` | Default workspace ID |

## Resolution Order (highest priority wins)

1. `--flag` CLI argument
2. Environment variable (`CLICKUP_TOKEN`, `CLICKUP_WORKSPACE`)
3. Project config (`.clickup.toml`)
4. Global config (`~/.config/clickup-cli/config.toml`)

## AI Agent Setup

The CLI is LLM-agnostic. Inject the command reference into whichever agent instruction file your project uses:

```bash
clickup agent-config inject              # Auto-detects existing file
clickup agent-config inject CLAUDE.md    # Claude Code
clickup agent-config inject agent.md     # Generic
clickup agent-config inject .cursorrules # Cursor
```

Auto-detection checks: `CLAUDE.md`, `agent.md`, `AGENT.md`, `.cursorrules`, `.github/copilot-instructions.md`

[← Home](.)  ·  [Command Reference →](commands)  ·  [MCP Server →](mcp)
