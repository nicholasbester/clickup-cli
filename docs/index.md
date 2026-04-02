---
layout: default
title: Home
---

<div class="hero">
  <p class="hero-num">∞ · Element Cu</p>
  <img src="{{ '/assets/clickup-cli-logo.svg' | relative_url }}" alt="clickup-cli" width="100" height="114" style="margin-bottom:16px">
  <h1>clickup <em>cli</em></h1>
  <p class="lead">A token-efficient CLI and MCP server for the ClickUp API. ~130 endpoints. 143 MCP tools. ~98% smaller than raw JSON. Works with any AI agent.</p>
  <div class="hero-badges">
    <a href="https://crates.io/crates/clickup-cli"><img src="https://img.shields.io/crates/v/clickup-cli" alt="crates.io"></a>
    <a href="https://www.npmjs.com/package/@nick.bester/clickup-cli"><img src="https://img.shields.io/npm/v/@nick.bester/clickup-cli" alt="npm"></a>
    <a href="https://github.com/nicholasbester/clickup-cli/releases"><img src="https://img.shields.io/github/v/release/nicholasbester/clickup-cli" alt="release"></a>
    <a href="https://glama.ai/mcp/servers/nicholasbester/clickup-cli"><img src="https://glama.ai/mcp/servers/nicholasbester/clickup-cli/badges/score.svg" alt="Glama"></a>
  </div>
  <p class="hero-ver">v0.6.7 · Rust · Apache-2.0</p>
</div>

<div class="content">

<p class="sec-num">The Problem</p>
## Why This <em>Exists</em>

ClickUp's API returns deeply nested JSON. A single task list query produces **~12,000 tokens** of data — statuses wrapped in objects, assignees as arrays of user objects, timestamps as Unix milliseconds, custom fields, checklists, dependencies.

For AI agents operating within context windows, this is a serious problem. A few API calls can consume most of the available context, leaving little room for reasoning.

> You didn't build an AI agent to waste 12,000 tokens on a task list. There's an element for that.

<p class="sec-num">The Solution</p>
## Token-Efficient <em>Output</em>

clickup-cli flattens nested objects, selects only essential fields, and renders compact output — whether you're using the CLI or the MCP server.

<div class="cards">
  <div class="card">
    <div class="card-icon">📊</div>
    <div class="card-title">Raw API JSON</div>
    <div class="card-desc">~12,000 tokens for 5 tasks. Nested status objects, full user profiles, timestamp integers, custom field metadata.</div>
  </div>
  <div class="card">
    <div class="card-icon">⚡</div>
    <div class="card-title">clickup-cli Output</div>
    <div class="card-desc">~150 tokens for 5 tasks. Flattened status strings, comma-joined assignees, formatted dates. ~98% reduction.</div>
  </div>
  <div class="card">
    <div class="card-icon">🔧</div>
    <div class="card-title">Same Data, Less Noise</div>
    <div class="card-desc"><code>status: "in progress"</code> instead of <code>status: {status: "in progress", color: "#4466ff", ...}</code></div>
  </div>
</div>

<p class="sec-num">Coverage</p>
## 28 Resource Groups, 4 <em>Utilities</em>

<div class="cards">
  <div class="card">
    <div class="card-title" style="color:var(--amber)">Core</div>
    <div class="card-desc">workspace · space · folder · list · task</div>
  </div>
  <div class="card">
    <div class="card-title" style="color:var(--terra)">Collaboration</div>
    <div class="card-desc">comment · checklist · tag · field · task-type · attachment</div>
  </div>
  <div class="card">
    <div class="card-title" style="color:var(--teal)">Tracking</div>
    <div class="card-desc">time · goal · view · member · user</div>
  </div>
  <div class="card">
    <div class="card-title" style="color:var(--glass)">Communication</div>
    <div class="card-desc">chat (v3) · doc (v3) · webhook · template</div>
  </div>
  <div class="card">
    <div class="card-title" style="color:var(--deep)">Admin</div>
    <div class="card-desc">guest · group · role · shared · audit-log · acl</div>
  </div>
  <div class="card">
    <div class="card-title" style="color:var(--coral)">Utilities</div>
    <div class="card-desc">setup · auth · status · completions · agent-config · mcp</div>
  </div>
</div>

<p class="sec-num">Quick Start</p>
## Get <em>Running</em>

<div class="install-grid">
  <div class="install-card">
    <div class="install-card-title">npm</div>
    <code>npm install -g @nick.bester/clickup-cli</code>
  </div>
  <div class="install-card">
    <div class="install-card-title">Homebrew</div>
    <code>brew tap nicholasbester/clickup-cli</code>
    <code>brew install clickup-cli</code>
  </div>
  <div class="install-card">
    <div class="install-card-title">Cargo</div>
    <code>cargo install clickup-cli</code>
  </div>
  <div class="install-card">
    <div class="install-card-title">Docker</div>
    <code>docker build -t clickup-cli .</code>
  </div>
</div>

```bash
# Configure
clickup setup --token pk_your_token_here

# Verify
clickup auth whoami

# Use
clickup task list --list 12345
clickup task create --list 12345 --name "My Task" --priority 3
```

<p class="sec-num">AI Integration</p>
## Two Ways to <em>Connect</em>

<div class="cards">
  <div class="card" style="border-left:3px solid var(--amber)">
    <div class="card-title">CLI Mode (recommended)</div>
    <div class="card-desc">Most token-efficient. Inject a ~1,000 token command reference into your agent instructions. The agent runs CLI commands directly. Works with any LLM.</div>
    <pre style="margin-top:12px;font-size:11px"><code>clickup agent-config inject   # Auto-detects CLAUDE.md, agent.md, .cursorrules</code></pre>
  </div>
  <div class="card" style="border-left:3px solid var(--teal)">
    <div class="card-title">MCP Mode (143 tools)</div>
    <div class="card-desc">For Claude Desktop, Cursor, and tools that prefer native tool integration. 143 tools with compact responses. More tokens consumed by tool schemas.</div>
    <pre style="margin-top:12px;font-size:11px"><code>clickup agent-config init --mcp</code></pre>
  </div>
</div>

<div style="text-align:center;margin:48px 0 24px">
  <a href="{{ '/install' | relative_url }}" style="display:inline-block;padding:10px 28px;background:var(--deep);color:var(--golden-lt);border-radius:6px;font-size:12px;font-weight:500;letter-spacing:1px;text-transform:uppercase;text-decoration:none;transition:all .2s">Installation →</a>
  <a href="{{ '/commands' | relative_url }}" style="display:inline-block;padding:10px 28px;background:transparent;color:var(--deep);border:1.5px solid var(--deep);border-radius:6px;font-size:12px;font-weight:500;letter-spacing:1px;text-transform:uppercase;text-decoration:none;margin-left:8px;transition:all .2s">Commands →</a>
  <a href="{{ '/mcp' | relative_url }}" style="display:inline-block;padding:10px 28px;background:transparent;color:var(--teal);border:1.5px solid var(--teal);border-radius:6px;font-size:12px;font-weight:500;letter-spacing:1px;text-transform:uppercase;text-decoration:none;margin-left:8px;transition:all .2s">MCP Server →</a>
</div>

</div>
