---
layout: default
title: Installation
---

# Installation

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

## From source

Requires [Rust](https://rustup.rs/) 1.70+:

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
clickup --version
clickup auth whoami
```

[← Home](.)  ·  [Command Reference →](commands)
