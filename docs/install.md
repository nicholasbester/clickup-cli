---
layout: default
title: Installation
description: Install clickup-cli via npm, Homebrew, Cargo, Docker, or prebuilt binaries. Works on macOS, Linux, and Windows.
permalink: /install/
---

# Installation

## Migration from 0.10.x

The `clickup` binary was **removed in 0.11.0**. Installs now ship two binaries instead:

- `clickup-cli` — canonical name (use this in MCP configs, scripts, and CI)
- `clkup` — short alias with identical behaviour, useful for daily typing

If you're upgrading, run `s/clickup /clickup-cli /` in your MCP configs (`.mcp.json`, `claude_desktop_config.json`) and any shell aliases. Or swap to the shorter `clkup` if you prefer.

## npm (any platform with Node.js)

```bash
npm install -g @nick.bester/clickup-cli
```

## Homebrew (macOS or Linux)

```bash
brew tap nicholasbester/clickup-cli
brew install clickup-cli
```

Upgrade: `brew upgrade clickup-cli`

Works on Linux too — the tap ships native x86_64 and arm64 Linux binaries.

## macOS / Linux (pre-built binary)

```bash
# macOS Apple Silicon (M1/M2/M3/M4)
curl -L https://github.com/nicholasbester/clickup-cli/releases/latest/download/clickup-macos-arm64.tar.gz | tar xz
sudo mv clickup-cli clkup /usr/local/bin/

# macOS Intel
curl -L https://github.com/nicholasbester/clickup-cli/releases/latest/download/clickup-macos-x86_64.tar.gz | tar xz
sudo mv clickup-cli clkup /usr/local/bin/

# Linux x86_64
curl -L https://github.com/nicholasbester/clickup-cli/releases/latest/download/clickup-linux-x86_64.tar.gz | tar xz
sudo mv clickup-cli clkup /usr/local/bin/

# Linux ARM64
curl -L https://github.com/nicholasbester/clickup-cli/releases/latest/download/clickup-linux-arm64.tar.gz | tar xz
sudo mv clickup-cli clkup /usr/local/bin/
```

### Alpine / musl Linux

For Alpine-based containers (LibreChat, distroless-ish images, minimal Docker layers) use the statically-linked musl build. It has no libc or TLS runtime dependencies and runs on any Linux distribution, not just Alpine.

```sh
curl -L https://github.com/nicholasbester/clickup-cli/releases/latest/download/clickup-linux-x86_64-musl.tar.gz | tar xz
mv clickup-cli clkup /usr/local/bin/
clickup-cli --version       # Should show: clickup-cli {{ site.version }}
```

Or inside a Dockerfile:

```dockerfile
FROM alpine:3.20
RUN apk add --no-cache curl tar \
    && curl -L https://github.com/nicholasbester/clickup-cli/releases/latest/download/clickup-linux-x86_64-musl.tar.gz \
       | tar xz -C /usr/local/bin/
```

## Arch Linux (AUR)

```bash
yay -S clickup-cli-bin
# or
paru -S clickup-cli-bin
```

`clickup-cli-bin` wraps the prebuilt x86_64 / aarch64 Linux binaries — no Rust toolchain required. Maintained in-sync with every release via GitHub Actions.

## Windows

Download `clickup-windows-x86_64.zip` from the [latest release](https://github.com/nicholasbester/clickup-cli/releases/latest), extract, and add `clickup-cli.exe` (and optionally `clkup.exe`) to your PATH.

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

## Two binaries

Every install method (npm, Homebrew, Cargo, AUR, prebuilt tarballs, Docker) ships **two binaries** with identical behaviour:

- `clickup-cli` — canonical name (use this in scripts, CI, and MCP configs)
- `clkup` — short alias, handy for daily interactive typing

Pick whichever you prefer; both accept the same flags and subcommands. The rest of these docs use `clickup-cli` for clarity, but `clkup` is fully interchangeable.

## Setup

Get a personal API token from ClickUp: **Settings > Apps > API Token**

```bash
# Interactive
clickup-cli setup

# Non-interactive (for CI/scripts/agents)
clickup-cli setup --token pk_your_token_here
```

Config is saved to `~/.config/clickup-cli/config.toml`.

## Verify

```bash
clickup-cli --version       # Should show: clickup-cli {{ site.version }}
clickup-cli auth whoami
clickup-cli status          # Show config, token (masked), workspace
```

## Shell Completions

```bash
# Bash
clickup-cli completions bash > ~/.bash_completion.d/clickup-cli

# Zsh (add ~/.zfunc to fpath in .zshrc first)
clickup-cli completions zsh > ~/.zfunc/_clickup-cli

# Fish
clickup-cli completions fish > ~/.config/fish/completions/clickup-cli.fish

# PowerShell
clickup-cli completions powershell > clickup-cli.ps1
```

## Project-Level Config

For per-project settings (different workspace, different token), create a `.clickup.toml` in the project root:

```bash
clickup-cli agent-config init --token pk_xxx --workspace 12345
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
clickup-cli agent-config inject              # Auto-detects existing file
clickup-cli agent-config inject CLAUDE.md    # Claude Code
clickup-cli agent-config inject agent.md     # Generic
clickup-cli agent-config inject .cursorrules # Cursor
```

Auto-detection checks: `CLAUDE.md`, `agent.md`, `AGENT.md`, `.cursorrules`, `.github/copilot-instructions.md`

[← Home](.)  ·  [Command Reference →](commands)  ·  [MCP Server →](mcp)
