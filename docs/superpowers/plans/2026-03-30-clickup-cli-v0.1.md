# clickup-cli v0.1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a working Rust CLI that authenticates with ClickUp, manages workspaces/spaces/folders/lists/tasks, and outputs results in token-efficient table format.

**Architecture:** Single-crate Rust binary using clap derive for CLI parsing, reqwest for HTTP, serde for JSON. Each resource group gets a command module and a model module. A shared output engine handles all formatting. The HTTP client handles auth, rate limiting, and retries.

**Tech Stack:** Rust, clap v4 (derive), reqwest (rustls-tls), serde/serde_json, tokio, toml, dirs, thiserror, chrono, comfy-table, wiremock (dev)

**Spec:** `docs/superpowers/specs/2026-03-17-clickup-cli-rust-rewrite-design.md`

---

## File Structure

```
Cargo.toml
src/
  main.rs              # Tokio entrypoint, clap App with global flags, command dispatch
  error.rs             # CliError enum, exit codes, Display impl, JSON error output
  config.rs            # Config struct, TOML read/write, config path resolution
  client.rs            # ClickUpClient: HTTP methods, auth header, rate limit, retry
  output.rs            # OutputConfig, Displayable trait, table/json/csv/quiet formatting
  commands/
    mod.rs             # Re-exports all command modules
    setup.rs           # `clickup setup` handler
    auth.rs            # `clickup auth whoami` and `clickup auth check`
    workspace.rs       # `clickup workspace {list,seats,plan}`
    space.rs           # `clickup space {list,get,create,update,delete}`
    folder.rs          # `clickup folder {list,get,create,update,delete}`
    list.rs            # `clickup list {list,get,create,update,delete,add-task,remove-task}`
    task.rs            # `clickup task {list,search,get,create,update,delete,time-in-status}`
  models/
    mod.rs             # Re-exports, common pagination types
    user.rs            # User struct (from /v2/user)
    workspace.rs       # Workspace, WorkspaceSeat, WorkspacePlan
    space.rs           # Space
    folder.rs          # Folder
    list.rs            # List
    task.rs            # Task, TaskStatus, Priority, TimeInStatus
tests/
  common/mod.rs        # Shared test helpers (mock server setup, fixture loading)
  test_config.rs       # Config read/write tests
  test_output.rs       # Output formatting tests
  test_error.rs        # Error display tests
  test_client.rs       # HTTP client tests with wiremock
  test_commands.rs     # Command handler tests with wiremock
```

---

### Task 1: Project Scaffolding

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/error.rs`
- Create: `src/config.rs`
- Create: `src/client.rs`
- Create: `src/output.rs`
- Create: `src/commands/mod.rs`
- Create: `src/models/mod.rs`

**Why:** Establish the Rust project skeleton so `cargo build` works and `clickup --help` shows the top-level command groups. All modules start as stubs.

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "clickup-cli"
version = "0.1.0"
edition = "2021"
description = "CLI for the ClickUp API, optimized for AI agents"
license = "Apache-2.0"

[[bin]]
name = "clickup"
path = "src/main.rs"

[dependencies]
clap = { version = "4", features = ["derive"] }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
toml = "0.8"
dirs = "6"
thiserror = "2"
chrono = { version = "0.4", features = ["serde"] }
comfy-table = "7"

[dev-dependencies]
wiremock = "0.6"
tempfile = "3"
assert_cmd = "2"
predicates = "3"
```

- [ ] **Step 2: Create src/main.rs with clap CLI skeleton**

```rust
mod client;
mod commands;
mod config;
mod error;
mod models;
mod output;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "clickup", version, about = "CLI for the ClickUp API")]
pub struct Cli {
    /// API token (overrides config file)
    #[arg(long, global = true)]
    pub token: Option<String>,

    /// Workspace ID (overrides config default)
    #[arg(long, global = true)]
    pub workspace: Option<String>,

    /// Output format: table, json, json-compact, csv
    #[arg(long, global = true, default_value = "table")]
    pub output: String,

    /// Comma-separated list of fields to display
    #[arg(long, global = true)]
    pub fields: Option<String>,

    /// Omit table header row
    #[arg(long, global = true)]
    pub no_header: bool,

    /// Fetch all pages
    #[arg(long, global = true)]
    pub all: bool,

    /// Cap total results
    #[arg(long, global = true)]
    pub limit: Option<usize>,

    /// Manual page selection
    #[arg(long, global = true)]
    pub page: Option<u32>,

    /// Only print IDs, one per line
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// HTTP timeout in seconds
    #[arg(long, global = true, default_value = "30")]
    pub timeout: u64,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Configure API token and default workspace
    Setup(commands::setup::SetupArgs),
    /// Authentication commands
    Auth {
        #[command(subcommand)]
        command: commands::auth::AuthCommands,
    },
    /// Workspace commands
    Workspace {
        #[command(subcommand)]
        command: commands::workspace::WorkspaceCommands,
    },
    /// Space commands
    Space {
        #[command(subcommand)]
        command: commands::space::SpaceCommands,
    },
    /// Folder commands
    Folder {
        #[command(subcommand)]
        command: commands::folder::FolderCommands,
    },
    /// List commands
    List {
        #[command(subcommand)]
        command: commands::list::ListCommands,
    },
    /// Task commands
    Task {
        #[command(subcommand)]
        command: commands::task::TaskCommands,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let exit_code = run(cli).await;
    std::process::exit(exit_code);
}

async fn run(cli: Cli) -> i32 {
    let result = match cli.command {
        Commands::Setup(args) => commands::setup::execute(args, &cli).await,
        Commands::Auth { command } => commands::auth::execute(command, &cli).await,
        Commands::Workspace { command } => commands::workspace::execute(command, &cli).await,
        Commands::Space { command } => commands::space::execute(command, &cli).await,
        Commands::Folder { command } => commands::folder::execute(command, &cli).await,
        Commands::List { command } => commands::list::execute(command, &cli).await,
        Commands::Task { command } => commands::task::execute(command, &cli).await,
    };
    match result {
        Ok(()) => 0,
        Err(e) => {
            e.print(&cli.output);
            e.exit_code()
        }
    }
}
```

- [ ] **Step 3: Create module stubs**

`src/error.rs`:
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("{message}")]
    ClientError { message: String, status: u16 },
    #[error("{message}")]
    AuthError { message: String },
    #[error("{message}")]
    NotFound { message: String, resource_id: String },
    #[error("{message}")]
    RateLimited { message: String, retry_after: Option<u64> },
    #[error("{message}")]
    ServerError { message: String },
    #[error("{0}")]
    ConfigError(String),
    #[error("{0}")]
    IoError(#[from] std::io::Error),
}

impl CliError {
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::ClientError { .. } => 1,
            CliError::AuthError { .. } => 2,
            CliError::NotFound { .. } => 3,
            CliError::RateLimited { .. } => 4,
            CliError::ServerError { .. } => 5,
            CliError::ConfigError(_) => 1,
            CliError::IoError(_) => 1,
        }
    }

    pub fn print(&self, output_mode: &str) {
        if output_mode == "json" {
            let json = serde_json::json!({
                "error": true,
                "message": self.to_string(),
                "exit_code": self.exit_code(),
                "hint": self.hint(),
            });
            eprintln!("{}", serde_json::to_string_pretty(&json).unwrap());
        } else {
            eprintln!("Error: {}", self);
            if let Some(status) = self.status() {
                eprintln!("  Status:  {}", status);
            }
            if let Some(hint) = self.hint() {
                eprintln!("  Hint:    {}", hint);
            }
        }
    }

    fn status(&self) -> Option<u16> {
        match self {
            CliError::ClientError { status, .. } => Some(*status),
            CliError::AuthError { .. } => Some(401),
            CliError::NotFound { .. } => Some(404),
            CliError::RateLimited { .. } => Some(429),
            CliError::ServerError { .. } => Some(500),
            _ => None,
        }
    }

    fn hint(&self) -> Option<String> {
        match self {
            CliError::AuthError { .. } => {
                Some("Check your API token, or run 'clickup setup' to reconfigure".into())
            }
            CliError::NotFound { resource_id, .. } => {
                Some(format!("Check the ID '{}', or use --custom-task-id if using a custom task ID", resource_id))
            }
            CliError::RateLimited { retry_after, .. } => {
                retry_after.map(|s| format!("Rate limited. Retry after {} seconds", s))
            }
            CliError::ServerError { .. } => {
                Some("ClickUp server error. Try again in a few seconds.".into())
            }
            CliError::ConfigError(_) => {
                Some("Run 'clickup setup' to configure your API token".into())
            }
            _ => None,
        }
    }
}
```

`src/config.rs`:
```rust
use crate::error::CliError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub auth: AuthConfig,
    #[serde(default)]
    pub defaults: DefaultsConfig,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AuthConfig {
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct DefaultsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

impl Config {
    pub fn config_path() -> Result<PathBuf, CliError> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| CliError::ConfigError("Could not determine config directory".into()))?;
        Ok(config_dir.join("clickup-cli").join("config.toml"))
    }

    pub fn load() -> Result<Self, CliError> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Err(CliError::ConfigError("Not configured".into()));
        }
        let contents = std::fs::read_to_string(&path)?;
        toml::from_str(&contents)
            .map_err(|e| CliError::ConfigError(format!("Invalid config file: {}", e)))
    }

    pub fn save(&self) -> Result<(), CliError> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)
            .map_err(|e| CliError::ConfigError(format!("Failed to serialize config: {}", e)))?;
        std::fs::write(&path, contents)?;
        Ok(())
    }
}
```

`src/client.rs`:
```rust
use crate::error::CliError;

pub struct ClickUpClient {
    http: reqwest::Client,
    base_url: String,
    token: String,
}

impl ClickUpClient {
    pub fn new(token: &str, timeout_secs: u64) -> Result<Self, CliError> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()
            .map_err(|e| CliError::ClientError {
                message: format!("Failed to create HTTP client: {}", e),
                status: 0,
            })?;
        Ok(Self {
            http,
            base_url: "https://api.clickup.com/api".to_string(),
            token: token.to_string(),
        })
    }

    pub async fn get(&self, path: &str) -> Result<serde_json::Value, CliError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .get(&url)
            .header("Authorization", &self.token)
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| CliError::ClientError {
                message: format!("Request failed: {}", e),
                status: 0,
            })?;
        self.handle_response(resp).await
    }

    pub async fn post(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value, CliError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .post(&url)
            .header("Authorization", &self.token)
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| CliError::ClientError {
                message: format!("Request failed: {}", e),
                status: 0,
            })?;
        self.handle_response(resp).await
    }

    pub async fn put(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value, CliError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .put(&url)
            .header("Authorization", &self.token)
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| CliError::ClientError {
                message: format!("Request failed: {}", e),
                status: 0,
            })?;
        self.handle_response(resp).await
    }

    pub async fn delete(&self, path: &str) -> Result<serde_json::Value, CliError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .delete(&url)
            .header("Authorization", &self.token)
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| CliError::ClientError {
                message: format!("Request failed: {}", e),
                status: 0,
            })?;
        self.handle_response(resp).await
    }

    async fn handle_response(&self, resp: reqwest::Response) -> Result<serde_json::Value, CliError> {
        let status = resp.status().as_u16();
        if status == 200 {
            let body: serde_json::Value = resp.json().await.map_err(|e| CliError::ClientError {
                message: format!("Failed to parse response: {}", e),
                status,
            })?;
            return Ok(body);
        }
        let body_text = resp.text().await.unwrap_or_default();
        let message = serde_json::from_str::<serde_json::Value>(&body_text)
            .ok()
            .and_then(|v| v.get("err").and_then(|e| e.as_str()).map(String::from))
            .unwrap_or_else(|| format!("HTTP {}", status));

        match status {
            401 => Err(CliError::AuthError { message }),
            404 => Err(CliError::NotFound {
                message,
                resource_id: String::new(),
            }),
            429 => Err(CliError::RateLimited {
                message,
                retry_after: None,
            }),
            500..=599 => Err(CliError::ServerError { message }),
            _ => Err(CliError::ClientError { message, status }),
        }
    }

    #[cfg(test)]
    pub fn with_base_url(mut self, base_url: &str) -> Self {
        self.base_url = base_url.to_string();
        self
    }
}
```

`src/output.rs`:
```rust
use comfy_table::{Table, ContentArrangement};

pub struct OutputConfig {
    pub mode: String,
    pub fields: Option<Vec<String>>,
    pub no_header: bool,
    pub quiet: bool,
}

impl OutputConfig {
    pub fn from_cli(mode: &str, fields: &Option<String>, no_header: bool, quiet: bool) -> Self {
        Self {
            mode: mode.to_string(),
            fields: fields.as_ref().map(|f| f.split(',').map(|s| s.trim().to_string()).collect()),
            no_header,
            quiet,
        }
    }

    pub fn print_items(&self, items: &[serde_json::Value], default_fields: &[&str], id_field: &str) {
        if self.quiet {
            for item in items {
                if let Some(id) = item.get(id_field).and_then(|v| v.as_str()) {
                    println!("{}", id);
                }
            }
            return;
        }

        let fields: Vec<&str> = match &self.fields {
            Some(f) => f.iter().map(|s| s.as_str()).collect(),
            None => default_fields.to_vec(),
        };

        match self.mode.as_str() {
            "json" => {
                println!("{}", serde_json::to_string_pretty(items).unwrap());
            }
            "json-compact" => {
                let filtered: Vec<serde_json::Value> = items
                    .iter()
                    .map(|item| {
                        let mut obj = serde_json::Map::new();
                        for &field in &fields {
                            if let Some(val) = item.get(field) {
                                obj.insert(field.to_string(), val.clone());
                            }
                        }
                        serde_json::Value::Object(obj)
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&filtered).unwrap());
            }
            "csv" => {
                if !self.no_header {
                    println!("{}", fields.join(","));
                }
                for item in items {
                    let row: Vec<String> = fields
                        .iter()
                        .map(|&f| flatten_value(item.get(f)))
                        .collect();
                    println!("{}", row.join(","));
                }
            }
            _ => {
                // table (default)
                let mut table = Table::new();
                table.set_content_arrangement(ContentArrangement::Dynamic);
                if !self.no_header {
                    table.set_header(fields.iter().map(|f| f.to_string()).collect::<Vec<_>>());
                }
                for item in items {
                    let row: Vec<String> = fields
                        .iter()
                        .map(|&f| flatten_value(item.get(f)))
                        .collect();
                    table.add_row(row);
                }
                println!("{}", table);
            }
        }
    }

    pub fn print_single(&self, item: &serde_json::Value, default_fields: &[&str], id_field: &str) {
        self.print_items(&[item.clone()], default_fields, id_field);
    }

    pub fn print_message(&self, message: &str) {
        if self.mode == "json" {
            println!("{}", serde_json::json!({ "message": message }));
        } else {
            println!("{}", message);
        }
    }
}

pub fn flatten_value(value: Option<&serde_json::Value>) -> String {
    match value {
        None | Some(serde_json::Value::Null) => "-".to_string(),
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        Some(serde_json::Value::Bool(b)) => b.to_string(),
        Some(serde_json::Value::Array(arr)) => {
            // For arrays of objects with "username" field (assignees)
            let items: Vec<String> = arr
                .iter()
                .map(|v| {
                    if let Some(username) = v.get("username").and_then(|u| u.as_str()) {
                        username.to_string()
                    } else if let Some(s) = v.as_str() {
                        s.to_string()
                    } else {
                        v.to_string()
                    }
                })
                .collect();
            if items.is_empty() { "-".to_string() } else { items.join(", ") }
        }
        Some(serde_json::Value::Object(obj)) => {
            // Flatten nested objects: status.status, priority.priority
            if let Some(inner) = obj.get("status").and_then(|v| v.as_str()) {
                inner.to_string()
            } else if let Some(inner) = obj.get("priority").and_then(|v| v.as_str()) {
                inner.to_string()
            } else if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
                name.to_string()
            } else if let Some(username) = obj.get("username").and_then(|v| v.as_str()) {
                username.to_string()
            } else {
                serde_json::to_string(&serde_json::Value::Object(obj.clone())).unwrap()
            }
        }
    }
}
```

`src/commands/mod.rs`:
```rust
pub mod auth;
pub mod folder;
pub mod list;
pub mod setup;
pub mod space;
pub mod task;
pub mod workspace;
```

`src/models/mod.rs`:
```rust
pub mod folder;
pub mod list;
pub mod space;
pub mod task;
pub mod user;
pub mod workspace;
```

Create empty model stubs — each file will be filled in by later tasks:

`src/models/user.rs`:
```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct User {
    pub id: u64,
    pub username: String,
    pub email: String,
    pub color: Option<String>,
    pub profilePicture: Option<String>,
}
```

`src/models/workspace.rs`:
```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub members: Option<Vec<serde_json::Value>>,
}
```

`src/models/space.rs`:
```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Space {
    pub id: String,
    pub name: String,
    pub private: Option<bool>,
    pub archived: Option<bool>,
}
```

`src/models/folder.rs`:
```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub task_count: Option<String>,
    pub lists: Option<Vec<serde_json::Value>>,
}
```

`src/models/list.rs`:
```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct List {
    pub id: String,
    pub name: String,
    pub task_count: Option<serde_json::Value>,
    pub status: Option<serde_json::Value>,
    pub due_date: Option<String>,
    pub content: Option<String>,
}
```

`src/models/task.rs`:
```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub status: Option<serde_json::Value>,
    pub priority: Option<serde_json::Value>,
    pub assignees: Option<Vec<serde_json::Value>>,
    pub due_date: Option<String>,
    pub description: Option<String>,
    pub custom_id: Option<String>,
}
```

Now create command stubs. Each returns `todo!()` — they'll be implemented in later tasks:

`src/commands/setup.rs`:
```rust
use clap::Args;
use crate::Cli;
use crate::error::CliError;

#[derive(Args)]
pub struct SetupArgs {
    /// API token (skip interactive prompt)
    #[arg(long)]
    pub token: Option<String>,
}

pub async fn execute(_args: SetupArgs, _cli: &Cli) -> Result<(), CliError> {
    todo!("Implemented in Task 4")
}
```

`src/commands/auth.rs`:
```rust
use clap::Subcommand;
use crate::Cli;
use crate::error::CliError;

#[derive(Subcommand)]
pub enum AuthCommands {
    /// Show current user info
    Whoami,
    /// Quick token validation (exit code only)
    Check,
}

pub async fn execute(_command: AuthCommands, _cli: &Cli) -> Result<(), CliError> {
    todo!("Implemented in Task 5")
}
```

`src/commands/workspace.rs`:
```rust
use clap::Subcommand;
use crate::Cli;
use crate::error::CliError;

#[derive(Subcommand)]
pub enum WorkspaceCommands {
    /// List workspaces
    List,
    /// Show seat usage
    Seats,
    /// Show current plan
    Plan,
}

pub async fn execute(_command: WorkspaceCommands, _cli: &Cli) -> Result<(), CliError> {
    todo!("Implemented in Task 6")
}
```

`src/commands/space.rs`:
```rust
use clap::Subcommand;
use crate::Cli;
use crate::error::CliError;

#[derive(Subcommand)]
pub enum SpaceCommands {
    /// List spaces
    List {
        /// Include archived spaces
        #[arg(long)]
        archived: bool,
    },
    /// Get space details
    Get {
        /// Space ID
        id: String,
    },
    /// Create a new space
    Create {
        /// Space name
        #[arg(long)]
        name: String,
        /// Make space private
        #[arg(long)]
        private: bool,
        /// Allow multiple assignees
        #[arg(long)]
        multiple_assignees: bool,
    },
    /// Update a space
    Update {
        /// Space ID
        id: String,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// Color hex
        #[arg(long)]
        color: Option<String>,
    },
    /// Delete a space
    Delete {
        /// Space ID
        id: String,
    },
}

pub async fn execute(_command: SpaceCommands, _cli: &Cli) -> Result<(), CliError> {
    todo!("Implemented in Task 7")
}
```

`src/commands/folder.rs`:
```rust
use clap::Subcommand;
use crate::Cli;
use crate::error::CliError;

#[derive(Subcommand)]
pub enum FolderCommands {
    /// List folders in a space
    List {
        /// Space ID
        #[arg(long)]
        space: String,
        /// Include archived
        #[arg(long)]
        archived: bool,
    },
    /// Get folder details
    Get {
        /// Folder ID
        id: String,
    },
    /// Create a folder
    Create {
        /// Space ID
        #[arg(long)]
        space: String,
        /// Folder name
        #[arg(long)]
        name: String,
    },
    /// Update a folder
    Update {
        /// Folder ID
        id: String,
        /// New name
        #[arg(long)]
        name: String,
    },
    /// Delete a folder
    Delete {
        /// Folder ID
        id: String,
    },
}

pub async fn execute(_command: FolderCommands, _cli: &Cli) -> Result<(), CliError> {
    todo!("Implemented in Task 8")
}
```

`src/commands/list.rs`:
```rust
use clap::Subcommand;
use crate::Cli;
use crate::error::CliError;

#[derive(Subcommand)]
pub enum ListCommands {
    /// List lists in a folder or space
    List {
        /// Folder ID
        #[arg(long)]
        folder: Option<String>,
        /// Space ID (folderless lists)
        #[arg(long)]
        space: Option<String>,
        /// Include archived
        #[arg(long)]
        archived: bool,
    },
    /// Get list details
    Get {
        /// List ID
        id: String,
    },
    /// Create a list
    Create {
        /// Folder ID
        #[arg(long)]
        folder: Option<String>,
        /// Space ID (folderless)
        #[arg(long)]
        space: Option<String>,
        /// List name
        #[arg(long)]
        name: String,
        /// List content/description
        #[arg(long)]
        content: Option<String>,
        /// Priority (1-4)
        #[arg(long)]
        priority: Option<u8>,
        /// Due date (YYYY-MM-DD)
        #[arg(long)]
        due_date: Option<String>,
    },
    /// Update a list
    Update {
        /// List ID
        id: String,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// New content
        #[arg(long)]
        content: Option<String>,
    },
    /// Delete a list
    Delete {
        /// List ID
        id: String,
    },
    /// Add a task to this list
    AddTask {
        /// List ID
        list_id: String,
        /// Task ID
        task_id: String,
    },
    /// Remove a task from this list
    RemoveTask {
        /// List ID
        list_id: String,
        /// Task ID
        task_id: String,
    },
}

pub async fn execute(_command: ListCommands, _cli: &Cli) -> Result<(), CliError> {
    todo!("Implemented in Task 9")
}
```

`src/commands/task.rs`:
```rust
use clap::Subcommand;
use crate::Cli;
use crate::error::CliError;

#[derive(Subcommand)]
pub enum TaskCommands {
    /// List tasks in a list
    List {
        /// List ID
        #[arg(long)]
        list: String,
        /// Filter by status
        #[arg(long)]
        status: Option<Vec<String>>,
        /// Filter by assignee
        #[arg(long)]
        assignee: Option<Vec<String>>,
        /// Filter by tag
        #[arg(long)]
        tag: Option<Vec<String>>,
        /// Include closed tasks
        #[arg(long)]
        include_closed: bool,
        /// Order by field
        #[arg(long)]
        order_by: Option<String>,
        /// Reverse sort order
        #[arg(long)]
        reverse: bool,
    },
    /// Search tasks across workspace
    Search {
        /// Filter by space
        #[arg(long)]
        space: Option<String>,
        /// Filter by folder
        #[arg(long)]
        folder: Option<String>,
        /// Filter by list
        #[arg(long)]
        list: Option<String>,
        /// Filter by status
        #[arg(long)]
        status: Option<Vec<String>>,
        /// Filter by assignee
        #[arg(long)]
        assignee: Option<Vec<String>>,
        /// Filter by tag
        #[arg(long)]
        tag: Option<Vec<String>>,
    },
    /// Get task details
    Get {
        /// Task ID
        id: String,
        /// Include subtasks
        #[arg(long)]
        subtasks: bool,
        /// Treat ID as custom task ID
        #[arg(long)]
        custom_task_id: bool,
    },
    /// Create a task
    Create {
        /// List ID
        #[arg(long)]
        list: String,
        /// Task name
        #[arg(long)]
        name: String,
        /// Description
        #[arg(long)]
        description: Option<String>,
        /// Status
        #[arg(long)]
        status: Option<String>,
        /// Priority (1=urgent, 2=high, 3=normal, 4=low)
        #[arg(long)]
        priority: Option<u8>,
        /// Assignee user ID
        #[arg(long)]
        assignee: Option<Vec<String>>,
        /// Tag name
        #[arg(long)]
        tag: Option<Vec<String>>,
        /// Due date (YYYY-MM-DD)
        #[arg(long)]
        due_date: Option<String>,
        /// Parent task ID (creates subtask)
        #[arg(long)]
        parent: Option<String>,
    },
    /// Update a task
    Update {
        /// Task ID
        id: String,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// New status
        #[arg(long)]
        status: Option<String>,
        /// New priority (1-4)
        #[arg(long)]
        priority: Option<u8>,
        /// Add assignee
        #[arg(long)]
        add_assignee: Option<Vec<String>>,
        /// Remove assignee
        #[arg(long)]
        rem_assignee: Option<Vec<String>>,
        /// New description
        #[arg(long)]
        description: Option<String>,
    },
    /// Delete a task
    Delete {
        /// Task ID
        id: String,
    },
    /// Get time in status for a task
    TimeInStatus {
        /// Task ID(s) — multiple IDs triggers bulk mode
        ids: Vec<String>,
    },
}

pub async fn execute(_command: TaskCommands, _cli: &Cli) -> Result<(), CliError> {
    todo!("Implemented in Task 10")
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build`
Expected: Compiles with warnings about unused imports and todo macros.

- [ ] **Step 5: Verify --help and --version work**

Run: `cargo run -- --help`
Expected: Shows "CLI for the ClickUp API" with subcommands: setup, auth, workspace, space, folder, list, task

Run: `cargo run -- --version`
Expected: `clickup 0.1.0`

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/
git commit -m "feat: scaffold Rust project with clap CLI skeleton

All v0.1 command groups stubbed (setup, auth, workspace, space,
folder, list, task). Core modules: error, config, client, output.
Models for all v0.1 resources. clickup --help works."
```

---

### Task 2: Error Handling Tests

**Files:**
- Create: `tests/test_error.rs`
- Modify: `src/error.rs`

**Why:** Verify error formatting, exit codes, and JSON error output before any command uses them.

- [ ] **Step 1: Write failing tests for error display and exit codes**

`tests/test_error.rs`:
```rust
use clickup_cli::error::CliError;

#[test]
fn test_auth_error_exit_code() {
    let err = CliError::AuthError {
        message: "Token expired".into(),
    };
    assert_eq!(err.exit_code(), 2);
}

#[test]
fn test_not_found_exit_code() {
    let err = CliError::NotFound {
        message: "Task not found".into(),
        resource_id: "abc123".into(),
    };
    assert_eq!(err.exit_code(), 3);
}

#[test]
fn test_rate_limited_exit_code() {
    let err = CliError::RateLimited {
        message: "Rate limited".into(),
        retry_after: Some(30),
    };
    assert_eq!(err.exit_code(), 4);
}

#[test]
fn test_server_error_exit_code() {
    let err = CliError::ServerError {
        message: "Internal error".into(),
    };
    assert_eq!(err.exit_code(), 5);
}

#[test]
fn test_auth_error_hint() {
    let err = CliError::AuthError {
        message: "Unauthorized".into(),
    };
    assert!(err.hint().unwrap().contains("clickup setup"));
}

#[test]
fn test_not_found_hint_includes_id() {
    let err = CliError::NotFound {
        message: "Not found".into(),
        resource_id: "abc123".into(),
    };
    assert!(err.hint().unwrap().contains("abc123"));
}

#[test]
fn test_config_error_hint() {
    let err = CliError::ConfigError("Not configured".into());
    assert!(err.hint().unwrap().contains("clickup setup"));
}

#[test]
fn test_error_display() {
    let err = CliError::AuthError {
        message: "Token expired".into(),
    };
    assert_eq!(format!("{}", err), "Token expired");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test test_error`
Expected: FAIL — `CliError` not accessible as public. The `hint()` method is private.

- [ ] **Step 3: Make CliError and hint() public**

In `src/error.rs`, change `fn hint(` to `pub fn hint(` and `fn status(` to `pub fn status(`.

Create `src/lib.rs` to re-export modules:
```rust
pub mod client;
pub mod commands;
pub mod config;
pub mod error;
pub mod models;
pub mod output;
```

Update `src/main.rs` to use `clickup_cli::` prefix for imports:
```rust
use clickup_cli::commands;
use clickup_cli::error::CliError;
```

At the top of `src/main.rs`, remove the `mod` declarations and instead add:
```rust
use clickup_cli::commands;
```

The `Cli` struct and `Commands` enum stay in `main.rs` but the `run` function references `clickup_cli::` types.

Actually, simpler approach — move `Cli` into `lib.rs` so tests can access it too. In `src/lib.rs`:
```rust
pub mod client;
pub mod commands;
pub mod config;
pub mod error;
pub mod models;
pub mod output;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "clickup", version, about = "CLI for the ClickUp API")]
pub struct Cli {
    #[arg(long, global = true)]
    pub token: Option<String>,
    #[arg(long, global = true)]
    pub workspace: Option<String>,
    #[arg(long, global = true, default_value = "table")]
    pub output: String,
    #[arg(long, global = true)]
    pub fields: Option<String>,
    #[arg(long, global = true)]
    pub no_header: bool,
    #[arg(long, global = true)]
    pub all: bool,
    #[arg(long, global = true)]
    pub limit: Option<usize>,
    #[arg(long, global = true)]
    pub page: Option<u32>,
    #[arg(short, long, global = true)]
    pub quiet: bool,
    #[arg(long, global = true, default_value = "30")]
    pub timeout: u64,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Setup(commands::setup::SetupArgs),
    Auth {
        #[command(subcommand)]
        command: commands::auth::AuthCommands,
    },
    Workspace {
        #[command(subcommand)]
        command: commands::workspace::WorkspaceCommands,
    },
    Space {
        #[command(subcommand)]
        command: commands::space::SpaceCommands,
    },
    Folder {
        #[command(subcommand)]
        command: commands::folder::FolderCommands,
    },
    List {
        #[command(subcommand)]
        command: commands::list::ListCommands,
    },
    Task {
        #[command(subcommand)]
        command: commands::task::TaskCommands,
    },
}
```

And `src/main.rs` becomes:
```rust
use clap::Parser;
use clickup_cli::{commands, Cli, Commands};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let exit_code = run(cli).await;
    std::process::exit(exit_code);
}

async fn run(cli: Cli) -> i32 {
    let result = match cli.command {
        Commands::Setup(args) => commands::setup::execute(args, &cli).await,
        Commands::Auth { command } => commands::auth::execute(command, &cli).await,
        Commands::Workspace { command } => commands::workspace::execute(command, &cli).await,
        Commands::Space { command } => commands::space::execute(command, &cli).await,
        Commands::Folder { command } => commands::folder::execute(command, &cli).await,
        Commands::List { command } => commands::list::execute(command, &cli).await,
        Commands::Task { command } => commands::task::execute(command, &cli).await,
    };
    match result {
        Ok(()) => 0,
        Err(e) => {
            e.print(&cli.output);
            e.exit_code()
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test test_error`
Expected: All 8 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/main.rs src/error.rs tests/test_error.rs
git commit -m "test: add error handling tests, expose lib.rs for testing"
```

---

### Task 3: Config Module Tests

**Files:**
- Create: `tests/test_config.rs`
- Modify: `src/config.rs` (if needed)

**Why:** Verify config TOML read/write round-trips before setup command depends on it.

- [ ] **Step 1: Write failing tests for config**

`tests/test_config.rs`:
```rust
use clickup_cli::config::Config;
use tempfile::TempDir;

fn with_test_config_dir<F: FnOnce(&std::path::Path)>(f: F) {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("config.toml");
    f(&config_path);
}

#[test]
fn test_config_save_and_load() {
    with_test_config_dir(|path| {
        let config = Config {
            auth: clickup_cli::config::AuthConfig {
                token: "pk_test_123".into(),
            },
            defaults: clickup_cli::config::DefaultsConfig {
                workspace_id: Some("12345".into()),
                output: None,
            },
        };
        config.save_to(path).unwrap();
        let loaded = Config::load_from(path).unwrap();
        assert_eq!(loaded.auth.token, "pk_test_123");
        assert_eq!(loaded.defaults.workspace_id, Some("12345".into()));
    });
}

#[test]
fn test_config_load_missing_file() {
    with_test_config_dir(|path| {
        let result = Config::load_from(path);
        assert!(result.is_err());
    });
}

#[test]
fn test_config_minimal_toml() {
    with_test_config_dir(|path| {
        std::fs::write(path, "[auth]\ntoken = \"pk_abc\"\n").unwrap();
        let config = Config::load_from(path).unwrap();
        assert_eq!(config.auth.token, "pk_abc");
        assert_eq!(config.defaults.workspace_id, None);
    });
}

#[test]
fn test_config_save_creates_parent_dirs() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("deep").join("nested").join("config.toml");
    let config = Config {
        auth: clickup_cli::config::AuthConfig {
            token: "pk_test".into(),
        },
        defaults: clickup_cli::config::DefaultsConfig::default(),
    };
    config.save_to(&path).unwrap();
    assert!(path.exists());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test test_config`
Expected: FAIL — `save_to` and `load_from` methods don't exist yet.

- [ ] **Step 3: Add path-parameterized methods to Config**

In `src/config.rs`, add:
```rust
pub fn load_from(path: &std::path::Path) -> Result<Self, CliError> {
    if !path.exists() {
        return Err(CliError::ConfigError("Not configured".into()));
    }
    let contents = std::fs::read_to_string(path)?;
    toml::from_str(&contents)
        .map_err(|e| CliError::ConfigError(format!("Invalid config file: {}", e)))
}

pub fn save_to(&self, path: &std::path::Path) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let contents = toml::to_string_pretty(self)
        .map_err(|e| CliError::ConfigError(format!("Failed to serialize config: {}", e)))?;
    std::fs::write(path, contents)?;
    Ok(())
}
```

Update `load()` and `save()` to delegate:
```rust
pub fn load() -> Result<Self, CliError> {
    Self::load_from(&Self::config_path()?)
}

pub fn save(&self) -> Result<(), CliError> {
    self.save_to(&Self::config_path()?)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test test_config`
Expected: All 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/config.rs tests/test_config.rs
git commit -m "test: add config module tests with path-parameterized load/save"
```

---

### Task 4: Output Formatting Tests

**Files:**
- Create: `tests/test_output.rs`
- Modify: `src/output.rs` (if needed)

**Why:** The output engine is used by every command. Verify table, JSON, CSV, quiet, field flattening, and --fields filtering all work correctly.

- [ ] **Step 1: Write failing tests for output formatting**

`tests/test_output.rs`:
```rust
use clickup_cli::output::{flatten_value, OutputConfig};
use serde_json::json;

#[test]
fn test_flatten_null() {
    assert_eq!(flatten_value(None), "-");
    assert_eq!(flatten_value(Some(&json!(null))), "-");
}

#[test]
fn test_flatten_string() {
    assert_eq!(flatten_value(Some(&json!("hello"))), "hello");
}

#[test]
fn test_flatten_number() {
    assert_eq!(flatten_value(Some(&json!(42))), "42");
}

#[test]
fn test_flatten_bool() {
    assert_eq!(flatten_value(Some(&json!(true))), "true");
}

#[test]
fn test_flatten_status_object() {
    let val = json!({"status": "in progress", "color": "#abc"});
    assert_eq!(flatten_value(Some(&val)), "in progress");
}

#[test]
fn test_flatten_priority_object() {
    let val = json!({"priority": "high", "color": "#red"});
    assert_eq!(flatten_value(Some(&val)), "high");
}

#[test]
fn test_flatten_assignees_array() {
    let val = json!([{"username": "Nick"}, {"username": "Bob"}]);
    assert_eq!(flatten_value(Some(&val)), "Nick, Bob");
}

#[test]
fn test_flatten_empty_array() {
    let val = json!([]);
    assert_eq!(flatten_value(Some(&val)), "-");
}

#[test]
fn test_flatten_object_with_name() {
    let val = json!({"name": "My Space", "id": "123"});
    assert_eq!(flatten_value(Some(&val)), "My Space");
}

#[test]
fn test_output_config_parses_fields() {
    let config = OutputConfig::from_cli("table", &Some("id, name, status".into()), false, false);
    assert_eq!(config.fields, Some(vec!["id".to_string(), "name".to_string(), "status".to_string()]));
}

#[test]
fn test_output_config_no_fields() {
    let config = OutputConfig::from_cli("json", &None, false, false);
    assert_eq!(config.fields, None);
}
```

- [ ] **Step 2: Run tests to verify they fail or pass**

Run: `cargo test --test test_output`
Expected: Should pass if output.rs is already correct. If any fail, fix.

- [ ] **Step 3: Add due_date millisecond flattening**

The spec requires `due_date: "1773652547089"` to display as `"2026-03-17"`. Update `flatten_value` to detect ms timestamps in string values.

Add to `src/output.rs`:
```rust
use chrono::DateTime;
```

In `flatten_value`, update the `String` arm:
```rust
Some(serde_json::Value::String(s)) => {
    // Try to parse as Unix millisecond timestamp
    if let Ok(ms) = s.parse::<i64>() {
        if ms > 1_000_000_000_000 && ms < 10_000_000_000_000 {
            if let Some(dt) = DateTime::from_timestamp_millis(ms) {
                return dt.format("%Y-%m-%d").to_string();
            }
        }
    }
    s.clone()
}
```

Add a test for this:
```rust
#[test]
fn test_flatten_due_date_ms_timestamp() {
    // 2026-03-17 as Unix ms
    let val = json!("1773980400000");
    let result = flatten_value(Some(&val));
    assert!(result.starts_with("2026-03-1"), "Expected date starting with 2026-03-1, got: {}", result);
}

#[test]
fn test_flatten_normal_string_not_converted() {
    let val = json!("hello world");
    assert_eq!(flatten_value(Some(&val)), "hello world");
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test test_output`
Expected: All 13 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/output.rs tests/test_output.rs
git commit -m "test: add output formatting tests with date flattening"
```

---

### Task 5: HTTP Client with Rate Limiting and Retry

**Files:**
- Modify: `src/client.rs`
- Create: `tests/test_client.rs`

**Why:** The client is the backbone — auth header injection, rate limit detection, retry on 429/5xx.

- [ ] **Step 1: Write failing tests for the HTTP client**

`tests/test_client.rs`:
```rust
use clickup_cli::client::ClickUpClient;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn test_client(server: &MockServer) -> ClickUpClient {
    ClickUpClient::new("pk_test_token", 30)
        .unwrap()
        .with_base_url(&server.uri())
}

#[tokio::test]
async fn test_get_sends_auth_header() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/user"))
        .and(header("Authorization", "pk_test_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"user": {}})))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server).await;
    let result = client.get("/v2/user").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_401_returns_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/user"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({"err": "Token invalid"})))
        .mount(&server)
        .await;

    let client = test_client(&server).await;
    let result = client.get("/v2/user").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.exit_code(), 2);
}

#[tokio::test]
async fn test_404_returns_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/task/bad_id"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({"err": "Task not found"})))
        .mount(&server)
        .await;

    let client = test_client(&server).await;
    let result = client.get("/v2/task/bad_id").await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().exit_code(), 3);
}

#[tokio::test]
async fn test_429_returns_rate_limited() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/task/123"))
        .respond_with(ResponseTemplate::new(429).set_body_json(serde_json::json!({"err": "Rate limited"})))
        .mount(&server)
        .await;

    let client = test_client(&server).await;
    let result = client.get("/v2/task/123").await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().exit_code(), 4);
}

#[tokio::test]
async fn test_post_sends_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v2/list/123/task"))
        .and(header("Content-Type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "abc"})))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server).await;
    let body = serde_json::json!({"name": "Test Task"});
    let result = client.post("/v2/list/123/task", &body).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_500_returns_server_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/user"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({"err": "Internal"})))
        .mount(&server)
        .await;

    let client = test_client(&server).await;
    let result = client.get("/v2/user").await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().exit_code(), 5);
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --test test_client`
Expected: All 6 tests pass (client.rs already handles these status codes).

- [ ] **Step 3: Add rate limit header tracking and retry on 429**

Update `src/client.rs` to add retry logic. Add these fields and methods:

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

pub struct ClickUpClient {
    http: reqwest::Client,
    base_url: String,
    token: String,
    rate_limit_remaining: Arc<AtomicU64>,
    rate_limit_reset: Arc<AtomicU64>,
}
```

Add a private method to extract rate limit headers:
```rust
fn update_rate_limits(&self, headers: &reqwest::header::HeaderMap) {
    if let Some(remaining) = headers.get("X-RateLimit-Remaining") {
        if let Ok(val) = remaining.to_str().unwrap_or("0").parse::<u64>() {
            self.rate_limit_remaining.store(val, Ordering::Relaxed);
        }
    }
    if let Some(reset) = headers.get("X-RateLimit-Reset") {
        if let Ok(val) = reset.to_str().unwrap_or("0").parse::<u64>() {
            self.rate_limit_reset.store(val, Ordering::Relaxed);
        }
    }
}
```

Update `handle_response` to call `update_rate_limits` before checking status. For 429, read the reset header and compute wait time:

```rust
async fn handle_response(&self, resp: reqwest::Response) -> Result<serde_json::Value, CliError> {
    let status = resp.status().as_u16();
    self.update_rate_limits(resp.headers());

    if status == 200 {
        let body: serde_json::Value = resp.json().await.map_err(|e| CliError::ClientError {
            message: format!("Failed to parse response: {}", e),
            status,
        })?;
        return Ok(body);
    }

    let body_text = resp.text().await.unwrap_or_default();
    let message = serde_json::from_str::<serde_json::Value>(&body_text)
        .ok()
        .and_then(|v| v.get("err").and_then(|e| e.as_str()).map(String::from))
        .unwrap_or_else(|| format!("HTTP {}", status));

    let retry_after = {
        let reset = self.rate_limit_reset.load(Ordering::Relaxed);
        if reset > 0 {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            if reset > now { Some(reset - now) } else { None }
        } else {
            None
        }
    };

    match status {
        401 => Err(CliError::AuthError { message }),
        404 => Err(CliError::NotFound {
            message,
            resource_id: String::new(),
        }),
        429 => Err(CliError::RateLimited {
            message,
            retry_after,
        }),
        500..=599 => Err(CliError::ServerError { message }),
        _ => Err(CliError::ClientError { message, status }),
    }
}
```

Add a `request` method that wraps retry logic for 429 (once) and 5xx (3 times with backoff):

```rust
async fn request(
    &self,
    method: reqwest::Method,
    path: &str,
    body: Option<&serde_json::Value>,
) -> Result<serde_json::Value, CliError> {
    let url = format!("{}{}", self.base_url, path);
    let max_retries = 3;

    for attempt in 0..=max_retries {
        let mut req = self.http.request(method.clone(), &url)
            .header("Authorization", &self.token)
            .header("Content-Type", "application/json");

        if let Some(b) = body {
            req = req.json(b);
        }

        let resp = req.send().await.map_err(|e| CliError::ClientError {
            message: format!("Request failed: {}", e),
            status: 0,
        })?;

        let status = resp.status().as_u16();
        self.update_rate_limits(resp.headers());

        if status == 200 {
            let json: serde_json::Value = resp.json().await.map_err(|e| CliError::ClientError {
                message: format!("Failed to parse response: {}", e),
                status,
            })?;
            return Ok(json);
        }

        // Retry on 429 — wait for rate limit reset, retry once
        if status == 429 && attempt == 0 {
            let reset = self.rate_limit_reset.load(Ordering::Relaxed);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let wait = if reset > now { reset - now } else { 1 };
            eprintln!("Rate limited. Waiting {} seconds...", wait);
            sleep(Duration::from_secs(wait)).await;
            continue;
        }

        // Retry on 5xx with exponential backoff
        if (500..=599).contains(&status) && attempt < max_retries {
            let wait = 1u64 << attempt; // 1, 2, 4 seconds
            eprintln!("Server error ({}). Retrying in {}s...", status, wait);
            sleep(Duration::from_secs(wait)).await;
            continue;
        }

        // No retry — return error
        let body_text = resp.text().await.unwrap_or_default();
        let message = serde_json::from_str::<serde_json::Value>(&body_text)
            .ok()
            .and_then(|v| v.get("err").and_then(|e| e.as_str()).map(String::from))
            .unwrap_or_else(|| format!("HTTP {}", status));

        return match status {
            401 => Err(CliError::AuthError { message }),
            404 => Err(CliError::NotFound { message, resource_id: String::new() }),
            429 => Err(CliError::RateLimited { message, retry_after: None }),
            500..=599 => Err(CliError::ServerError { message }),
            _ => Err(CliError::ClientError { message, status }),
        };
    }

    Err(CliError::ServerError { message: "Max retries exceeded".into() })
}
```

Rewrite the public `get`, `post`, `put`, `delete` methods to delegate to `request`:

```rust
pub async fn get(&self, path: &str) -> Result<serde_json::Value, CliError> {
    self.request(reqwest::Method::GET, path, None).await
}

pub async fn post(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value, CliError> {
    self.request(reqwest::Method::POST, path, Some(body)).await
}

pub async fn put(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value, CliError> {
    self.request(reqwest::Method::PUT, path, Some(body)).await
}

pub async fn delete(&self, path: &str) -> Result<serde_json::Value, CliError> {
    self.request(reqwest::Method::DELETE, path, None).await
}
```

Update the constructor to initialize the atomics:
```rust
pub fn new(token: &str, timeout_secs: u64) -> Result<Self, CliError> {
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| CliError::ClientError {
            message: format!("Failed to create HTTP client: {}", e),
            status: 0,
        })?;
    Ok(Self {
        http,
        base_url: "https://api.clickup.com/api".to_string(),
        token: token.to_string(),
        rate_limit_remaining: Arc::new(AtomicU64::new(100)),
        rate_limit_reset: Arc::new(AtomicU64::new(0)),
    })
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test test_client`
Expected: All 6 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/client.rs tests/test_client.rs
git commit -m "feat: HTTP client with rate limiting, retry on 429/5xx"
```

---

### Task 6: Setup Command

**Files:**
- Modify: `src/commands/setup.rs`
- Modify: `src/client.rs` (add `get_with_query` if needed)

**Why:** First user-facing command. `clickup setup --token pk_xxx` must validate, fetch workspaces, save config.

- [ ] **Step 1: Implement the setup command**

`src/commands/setup.rs`:
```rust
use clap::Args;
use crate::client::ClickUpClient;
use crate::config::{AuthConfig, Config, DefaultsConfig};
use crate::error::CliError;
use crate::Cli;
use std::io::{self, Write};

#[derive(Args)]
pub struct SetupArgs {
    /// API token (skip interactive prompt)
    #[arg(long)]
    pub token: Option<String>,
}

pub async fn execute(args: SetupArgs, cli: &Cli) -> Result<(), CliError> {
    let token = match args.token.or_else(|| cli.token.clone()) {
        Some(t) => t,
        None => prompt_token()?,
    };

    // Validate token by hitting /v2/user
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let user_resp = client.get("/v2/user").await?;
    let username = user_resp
        .get("user")
        .and_then(|u| u.get("username"))
        .and_then(|u| u.as_str())
        .unwrap_or("Unknown");
    eprintln!("Validating... ✓ Authenticated as {}", username);

    // Fetch workspaces
    let teams_resp = client.get("/v2/team").await?;
    let teams = teams_resp
        .get("teams")
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default();

    let workspace_id = match teams.len() {
        0 => {
            return Err(CliError::ClientError {
                message: "No workspaces found for this token".into(),
                status: 0,
            });
        }
        1 => {
            let ws = &teams[0];
            let id = ws.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let name = ws.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
            eprintln!("\nOnly one workspace found — setting as default.");
            eprintln!("  {} (ID: {})", name, id);
            id.to_string()
        }
        _ => {
            eprintln!("\nFetching workspaces...");
            for (i, ws) in teams.iter().enumerate() {
                let id = ws.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let name = ws.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
                eprintln!("  [{}] {} (ID: {})", i + 1, name, id);
            }
            let choice = prompt_choice(teams.len())?;
            let ws = &teams[choice - 1];
            ws.get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        }
    };

    let config = Config {
        auth: AuthConfig {
            token: token.clone(),
        },
        defaults: DefaultsConfig {
            workspace_id: Some(workspace_id),
            output: None,
        },
    };
    config.save()?;

    let path = Config::config_path()?;
    eprintln!("Config saved to {}", path.display());
    Ok(())
}

fn prompt_token() -> Result<String, CliError> {
    eprint!("API Token (get one at Settings > Apps): ");
    io::stderr().flush()?;
    let mut token = String::new();
    io::stdin().read_line(&mut token)?;
    let token = token.trim().to_string();
    if token.is_empty() {
        return Err(CliError::ConfigError("No token provided".into()));
    }
    Ok(token)
}

fn prompt_choice(max: usize) -> Result<usize, CliError> {
    eprint!("\nSelect workspace [1-{}]: ", max);
    io::stderr().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let choice: usize = input
        .trim()
        .parse()
        .map_err(|_| CliError::ConfigError("Invalid selection".into()))?;
    if choice < 1 || choice > max {
        return Err(CliError::ConfigError(format!("Selection must be between 1 and {}", max)));
    }
    Ok(choice)
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully.

- [ ] **Step 3: Commit**

```bash
git add src/commands/setup.rs
git commit -m "feat: implement setup command with token validation and workspace selection"
```

---

### Task 7: Auth Commands

**Files:**
- Modify: `src/commands/auth.rs`

**Why:** `auth whoami` and `auth check` are essential for verifying configuration works.

- [ ] **Step 1: Implement auth commands**

`src/commands/auth.rs`:
```rust
use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::config::Config;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum AuthCommands {
    /// Show current user info
    Whoami,
    /// Quick token validation (exit code only)
    Check,
}

pub async fn execute(command: AuthCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;

    match command {
        AuthCommands::Whoami => {
            let resp = client.get("/v2/user").await?;
            let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);
            if let Some(user) = resp.get("user") {
                output.print_single(user, &["id", "username", "email"], "id");
            }
            Ok(())
        }
        AuthCommands::Check => {
            // Just hit the endpoint — success = exit 0, failure = error propagates
            client.get("/v2/user").await?;
            Ok(())
        }
    }
}

pub fn resolve_token(cli: &Cli) -> Result<String, CliError> {
    if let Some(token) = &cli.token {
        return Ok(token.clone());
    }
    let config = Config::load()?;
    if config.auth.token.is_empty() {
        return Err(CliError::ConfigError("Not configured".into()));
    }
    Ok(config.auth.token)
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles.

- [ ] **Step 3: Commit**

```bash
git add src/commands/auth.rs
git commit -m "feat: implement auth whoami and auth check commands"
```

---

### Task 8: Workspace Commands

**Files:**
- Modify: `src/commands/workspace.rs`

**Why:** Workspace list/seats/plan commands. First resource group beyond auth.

- [ ] **Step 1: Implement workspace commands**

`src/commands/workspace.rs`:
```rust
use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::config::Config;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum WorkspaceCommands {
    /// List workspaces
    List,
    /// Show seat usage
    Seats,
    /// Show current plan
    Plan,
}

pub fn resolve_workspace(cli: &Cli) -> Result<String, CliError> {
    if let Some(ws) = &cli.workspace {
        return Ok(ws.clone());
    }
    let config = Config::load()?;
    config
        .defaults
        .workspace_id
        .ok_or_else(|| CliError::ConfigError("No default workspace. Use --workspace or run 'clickup setup'".into()))
}

pub async fn execute(command: WorkspaceCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        WorkspaceCommands::List => {
            let resp = client.get("/v2/team").await?;
            let teams = resp
                .get("teams")
                .and_then(|t| t.as_array())
                .cloned()
                .unwrap_or_default();
            // Simplify for table output — extract id, name, member count
            let items: Vec<serde_json::Value> = teams
                .iter()
                .map(|ws| {
                    serde_json::json!({
                        "id": ws.get("id").and_then(|v| v.as_str()).unwrap_or("-"),
                        "name": ws.get("name").and_then(|v| v.as_str()).unwrap_or("-"),
                        "members": ws.get("members").and_then(|m| m.as_array()).map(|a| a.len()).unwrap_or(0),
                    })
                })
                .collect();
            output.print_items(&items, &["id", "name", "members"], "id");
            Ok(())
        }
        WorkspaceCommands::Seats => {
            let ws_id = resolve_workspace(cli)?;
            let resp = client.get(&format!("/v2/team/{}/seats", ws_id)).await?;
            if cli.output == "json" {
                println!("{}", serde_json::to_string_pretty(&resp).unwrap());
            } else {
                // seats response has filled_members_seats, empty_members_seats, etc.
                let filled = resp.get("seats").and_then(|s| s.get("members"))
                    .and_then(|m| m.get("filled_members_seats"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let total = resp.get("seats").and_then(|s| s.get("members"))
                    .and_then(|m| m.get("total_members_seats"))
                    .and_then(|v| v.as_u64())
                    .or_else(|| {
                        // Try alternative: top-level fields
                        resp.get("filled_members_seats").and_then(|v| v.as_u64())
                    })
                    .unwrap_or(0);
                let items = vec![serde_json::json!({
                    "filled_seats": filled,
                    "total_seats": total,
                })];
                output.print_items(&items, &["filled_seats", "total_seats"], "filled_seats");
            }
            Ok(())
        }
        WorkspaceCommands::Plan => {
            let ws_id = resolve_workspace(cli)?;
            let resp = client.get(&format!("/v2/team/{}/plan", ws_id)).await?;
            if cli.output == "json" {
                println!("{}", serde_json::to_string_pretty(&resp).unwrap());
            } else {
                let plan_name = resp.get("plan_id")
                    .or_else(|| resp.get("plan_name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                println!("Plan: {}", plan_name);
            }
            Ok(())
        }
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles.

- [ ] **Step 3: Commit**

```bash
git add src/commands/workspace.rs
git commit -m "feat: implement workspace list, seats, and plan commands"
```

---

### Task 9: Space Commands

**Files:**
- Modify: `src/commands/space.rs`

**Why:** Full CRUD for spaces. First resource with create/update/delete.

- [ ] **Step 1: Implement space commands**

`src/commands/space.rs`:
```rust
use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum SpaceCommands {
    /// List spaces in workspace
    List {
        /// Include archived spaces
        #[arg(long)]
        archived: bool,
    },
    /// Get space details
    Get {
        /// Space ID
        id: String,
    },
    /// Create a new space
    Create {
        /// Space name
        #[arg(long)]
        name: String,
        /// Make space private
        #[arg(long)]
        private: bool,
        /// Allow multiple assignees
        #[arg(long)]
        multiple_assignees: bool,
    },
    /// Update a space
    Update {
        /// Space ID
        id: String,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// Color hex
        #[arg(long)]
        color: Option<String>,
    },
    /// Delete a space
    Delete {
        /// Space ID
        id: String,
    },
}

pub async fn execute(command: SpaceCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        SpaceCommands::List { archived } => {
            let ws_id = resolve_workspace(cli)?;
            let resp = client
                .get(&format!("/v2/team/{}/space?archived={}", ws_id, archived))
                .await?;
            let spaces = resp
                .get("spaces")
                .and_then(|s| s.as_array())
                .cloned()
                .unwrap_or_default();
            output.print_items(&spaces, &["id", "name", "private", "archived"], "id");
            Ok(())
        }
        SpaceCommands::Get { id } => {
            let resp = client.get(&format!("/v2/space/{}", id)).await?;
            output.print_single(&resp, &["id", "name", "private", "archived"], "id");
            Ok(())
        }
        SpaceCommands::Create {
            name,
            private,
            multiple_assignees,
        } => {
            let ws_id = resolve_workspace(cli)?;
            let body = serde_json::json!({
                "name": name,
                "multiple_assignees": multiple_assignees,
                "features": {
                    "due_dates": { "enabled": true },
                    "priorities": { "enabled": true },
                    "tags": { "enabled": true },
                    "time_estimates": { "enabled": true },
                },
                "private": private,
            });
            let resp = client
                .post(&format!("/v2/team/{}/space", ws_id), &body)
                .await?;
            output.print_single(&resp, &["id", "name", "private"], "id");
            Ok(())
        }
        SpaceCommands::Update { id, name, color } => {
            let mut body = serde_json::Map::new();
            if let Some(n) = name {
                body.insert("name".into(), serde_json::Value::String(n));
            }
            if let Some(c) = color {
                body.insert("color".into(), serde_json::Value::String(c));
            }
            let resp = client
                .put(&format!("/v2/space/{}", id), &serde_json::Value::Object(body))
                .await?;
            output.print_single(&resp, &["id", "name", "private"], "id");
            Ok(())
        }
        SpaceCommands::Delete { id } => {
            client.delete(&format!("/v2/space/{}", id)).await?;
            output.print_message(&format!("Space {} deleted", id));
            Ok(())
        }
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles.

- [ ] **Step 3: Commit**

```bash
git add src/commands/space.rs
git commit -m "feat: implement space CRUD commands"
```

---

### Task 10: Folder Commands

**Files:**
- Modify: `src/commands/folder.rs`

**Why:** Folder CRUD. Same pattern as spaces.

- [ ] **Step 1: Implement folder commands**

`src/commands/folder.rs`:
```rust
use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum FolderCommands {
    /// List folders in a space
    List {
        /// Space ID
        #[arg(long)]
        space: String,
        /// Include archived
        #[arg(long)]
        archived: bool,
    },
    /// Get folder details
    Get {
        /// Folder ID
        id: String,
    },
    /// Create a folder
    Create {
        /// Space ID
        #[arg(long)]
        space: String,
        /// Folder name
        #[arg(long)]
        name: String,
    },
    /// Update a folder
    Update {
        /// Folder ID
        id: String,
        /// New name
        #[arg(long)]
        name: String,
    },
    /// Delete a folder
    Delete {
        /// Folder ID
        id: String,
    },
}

pub async fn execute(command: FolderCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        FolderCommands::List { space, archived } => {
            let resp = client
                .get(&format!("/v2/space/{}/folder?archived={}", space, archived))
                .await?;
            let folders = resp
                .get("folders")
                .and_then(|f| f.as_array())
                .cloned()
                .unwrap_or_default();
            // Flatten: extract list_count from lists array length
            let items: Vec<serde_json::Value> = folders
                .iter()
                .map(|f| {
                    let list_count = f
                        .get("lists")
                        .and_then(|l| l.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);
                    serde_json::json!({
                        "id": f.get("id"),
                        "name": f.get("name"),
                        "task_count": f.get("task_count"),
                        "list_count": list_count,
                    })
                })
                .collect();
            output.print_items(&items, &["id", "name", "task_count", "list_count"], "id");
            Ok(())
        }
        FolderCommands::Get { id } => {
            let resp = client.get(&format!("/v2/folder/{}", id)).await?;
            let list_count = resp
                .get("lists")
                .and_then(|l| l.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            let mut item = resp.clone();
            item.as_object_mut()
                .map(|o| o.insert("list_count".into(), serde_json::json!(list_count)));
            output.print_single(&item, &["id", "name", "task_count", "list_count"], "id");
            Ok(())
        }
        FolderCommands::Create { space, name } => {
            let body = serde_json::json!({ "name": name });
            let resp = client
                .post(&format!("/v2/space/{}/folder", space), &body)
                .await?;
            output.print_single(&resp, &["id", "name"], "id");
            Ok(())
        }
        FolderCommands::Update { id, name } => {
            let body = serde_json::json!({ "name": name });
            let resp = client
                .put(&format!("/v2/folder/{}", id), &body)
                .await?;
            output.print_single(&resp, &["id", "name"], "id");
            Ok(())
        }
        FolderCommands::Delete { id } => {
            client.delete(&format!("/v2/folder/{}", id)).await?;
            output.print_message(&format!("Folder {} deleted", id));
            Ok(())
        }
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles.

- [ ] **Step 3: Commit**

```bash
git add src/commands/folder.rs
git commit -m "feat: implement folder CRUD commands"
```

---

### Task 11: List Commands

**Files:**
- Modify: `src/commands/list.rs`

**Why:** List CRUD plus add-task/remove-task. Lists can live under folders or directly under spaces (folderless).

- [ ] **Step 1: Implement list commands**

`src/commands/list.rs`:
```rust
use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum ListCommands {
    /// List lists in a folder or space
    List {
        /// Folder ID
        #[arg(long)]
        folder: Option<String>,
        /// Space ID (folderless lists)
        #[arg(long)]
        space: Option<String>,
        /// Include archived
        #[arg(long)]
        archived: bool,
    },
    /// Get list details
    Get {
        /// List ID
        id: String,
    },
    /// Create a list
    Create {
        /// Folder ID
        #[arg(long)]
        folder: Option<String>,
        /// Space ID (folderless)
        #[arg(long)]
        space: Option<String>,
        /// List name
        #[arg(long)]
        name: String,
        /// List content/description
        #[arg(long)]
        content: Option<String>,
        /// Priority (1-4)
        #[arg(long)]
        priority: Option<u8>,
        /// Due date (YYYY-MM-DD)
        #[arg(long)]
        due_date: Option<String>,
    },
    /// Update a list
    Update {
        /// List ID
        id: String,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// New content
        #[arg(long)]
        content: Option<String>,
    },
    /// Delete a list
    Delete {
        /// List ID
        id: String,
    },
    /// Add a task to this list
    AddTask {
        /// List ID
        list_id: String,
        /// Task ID
        task_id: String,
    },
    /// Remove a task from this list
    RemoveTask {
        /// List ID
        list_id: String,
        /// Task ID
        task_id: String,
    },
}

pub async fn execute(command: ListCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);
    let default_fields = &["id", "name", "task_count", "status", "due_date"];

    match command {
        ListCommands::List { folder, space, archived } => {
            let path = match (&folder, &space) {
                (Some(f), _) => format!("/v2/folder/{}/list?archived={}", f, archived),
                (_, Some(s)) => format!("/v2/space/{}/list?archived={}", s, archived),
                _ => {
                    return Err(CliError::ClientError {
                        message: "Provide --folder or --space".into(),
                        status: 0,
                    });
                }
            };
            let resp = client.get(&path).await?;
            let lists = resp
                .get("lists")
                .and_then(|l| l.as_array())
                .cloned()
                .unwrap_or_default();
            output.print_items(&lists, default_fields, "id");
            Ok(())
        }
        ListCommands::Get { id } => {
            let resp = client.get(&format!("/v2/list/{}", id)).await?;
            output.print_single(&resp, default_fields, "id");
            Ok(())
        }
        ListCommands::Create {
            folder,
            space,
            name,
            content,
            priority,
            due_date,
        } => {
            let path = match (&folder, &space) {
                (Some(f), _) => format!("/v2/folder/{}/list", f),
                (_, Some(s)) => format!("/v2/space/{}/list", s),
                _ => {
                    return Err(CliError::ClientError {
                        message: "Provide --folder or --space".into(),
                        status: 0,
                    });
                }
            };
            let mut body = serde_json::json!({ "name": name });
            if let Some(c) = content {
                body["content"] = serde_json::Value::String(c);
            }
            if let Some(p) = priority {
                body["priority"] = serde_json::json!(p);
            }
            if let Some(d) = due_date {
                body["due_date"] = serde_json::Value::String(date_to_ms(&d)?);
            }
            let resp = client.post(&path, &body).await?;
            output.print_single(&resp, default_fields, "id");
            Ok(())
        }
        ListCommands::Update { id, name, content } => {
            let mut body = serde_json::Map::new();
            if let Some(n) = name {
                body.insert("name".into(), serde_json::Value::String(n));
            }
            if let Some(c) = content {
                body.insert("content".into(), serde_json::Value::String(c));
            }
            let resp = client
                .put(&format!("/v2/list/{}", id), &serde_json::Value::Object(body))
                .await?;
            output.print_single(&resp, default_fields, "id");
            Ok(())
        }
        ListCommands::Delete { id } => {
            client.delete(&format!("/v2/list/{}", id)).await?;
            output.print_message(&format!("List {} deleted", id));
            Ok(())
        }
        ListCommands::AddTask { list_id, task_id } => {
            client
                .post(
                    &format!("/v2/list/{}/task/{}", list_id, task_id),
                    &serde_json::json!({}),
                )
                .await?;
            output.print_message(&format!("Task {} added to list {}", task_id, list_id));
            Ok(())
        }
        ListCommands::RemoveTask { list_id, task_id } => {
            client
                .delete(&format!("/v2/list/{}/task/{}", list_id, task_id))
                .await?;
            output.print_message(&format!("Task {} removed from list {}", task_id, list_id));
            Ok(())
        }
    }
}

fn date_to_ms(date_str: &str) -> Result<String, CliError> {
    let naive = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|_| CliError::ClientError {
            message: format!("Invalid date '{}'. Use YYYY-MM-DD format.", date_str),
            status: 0,
        })?;
    let dt = naive
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc();
    Ok((dt.timestamp_millis()).to_string())
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles.

- [ ] **Step 3: Commit**

```bash
git add src/commands/list.rs
git commit -m "feat: implement list CRUD + add-task/remove-task commands"
```

---

### Task 12: Task Commands

**Files:**
- Modify: `src/commands/task.rs`

**Why:** The biggest and most important resource. Full CRUD, search, time-in-status, pagination, filters.

- [ ] **Step 1: Implement task commands**

`src/commands/task.rs`:
```rust
use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum TaskCommands {
    /// List tasks in a list
    List {
        /// List ID
        #[arg(long)]
        list: String,
        /// Filter by status
        #[arg(long)]
        status: Option<Vec<String>>,
        /// Filter by assignee
        #[arg(long)]
        assignee: Option<Vec<String>>,
        /// Filter by tag
        #[arg(long)]
        tag: Option<Vec<String>>,
        /// Include closed tasks
        #[arg(long)]
        include_closed: bool,
        /// Order by field
        #[arg(long)]
        order_by: Option<String>,
        /// Reverse sort order
        #[arg(long)]
        reverse: bool,
    },
    /// Search tasks across workspace
    Search {
        /// Filter by space
        #[arg(long)]
        space: Option<String>,
        /// Filter by folder
        #[arg(long)]
        folder: Option<String>,
        /// Filter by list
        #[arg(long)]
        list: Option<String>,
        /// Filter by status
        #[arg(long)]
        status: Option<Vec<String>>,
        /// Filter by assignee
        #[arg(long)]
        assignee: Option<Vec<String>>,
        /// Filter by tag
        #[arg(long)]
        tag: Option<Vec<String>>,
    },
    /// Get task details
    Get {
        /// Task ID
        id: String,
        /// Include subtasks
        #[arg(long)]
        subtasks: bool,
        /// Treat ID as custom task ID
        #[arg(long)]
        custom_task_id: bool,
    },
    /// Create a task
    Create {
        /// List ID
        #[arg(long)]
        list: String,
        /// Task name
        #[arg(long)]
        name: String,
        /// Description
        #[arg(long)]
        description: Option<String>,
        /// Status
        #[arg(long)]
        status: Option<String>,
        /// Priority (1=urgent, 2=high, 3=normal, 4=low)
        #[arg(long)]
        priority: Option<u8>,
        /// Assignee user ID
        #[arg(long)]
        assignee: Option<Vec<String>>,
        /// Tag name
        #[arg(long)]
        tag: Option<Vec<String>>,
        /// Due date (YYYY-MM-DD)
        #[arg(long)]
        due_date: Option<String>,
        /// Parent task ID (creates subtask)
        #[arg(long)]
        parent: Option<String>,
    },
    /// Update a task
    Update {
        /// Task ID
        id: String,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// New status
        #[arg(long)]
        status: Option<String>,
        /// New priority (1-4)
        #[arg(long)]
        priority: Option<u8>,
        /// Add assignee
        #[arg(long)]
        add_assignee: Option<Vec<String>>,
        /// Remove assignee
        #[arg(long)]
        rem_assignee: Option<Vec<String>>,
        /// New description
        #[arg(long)]
        description: Option<String>,
    },
    /// Delete a task
    Delete {
        /// Task ID
        id: String,
    },
    /// Get time in status for task(s)
    TimeInStatus {
        /// Task ID(s) — multiple IDs triggers bulk mode
        ids: Vec<String>,
    },
}

const TASK_FIELDS: &[&str] = &["id", "name", "status", "priority", "assignees", "due_date"];

pub async fn execute(command: TaskCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        TaskCommands::List {
            list,
            status,
            assignee,
            tag,
            include_closed,
            order_by,
            reverse,
        } => {
            let mut params = Vec::new();
            if include_closed {
                params.push("include_closed=true".to_string());
            }
            if let Some(statuses) = &status {
                for s in statuses {
                    params.push(format!("statuses[]={}", s));
                }
            }
            if let Some(assignees) = &assignee {
                for a in assignees {
                    params.push(format!("assignees[]={}", a));
                }
            }
            if let Some(tags) = &tag {
                for t in tags {
                    params.push(format!("tags[]={}", t));
                }
            }
            if let Some(ob) = &order_by {
                params.push(format!("order_by={}", ob));
            }
            if reverse {
                params.push("reverse=true".to_string());
            }
            if let Some(page) = cli.page {
                params.push(format!("page={}", page));
            }

            let query = if params.is_empty() {
                String::new()
            } else {
                format!("?{}", params.join("&"))
            };

            if cli.all {
                // Auto-paginate
                let mut all_tasks = Vec::new();
                let mut page = 0u32;
                loop {
                    let mut page_params = params.clone();
                    page_params.push(format!("page={}", page));
                    let page_query = format!("?{}", page_params.join("&"));
                    let resp = client
                        .get(&format!("/v2/list/{}/task{}", list, page_query))
                        .await?;
                    let tasks = resp
                        .get("tasks")
                        .and_then(|t| t.as_array())
                        .cloned()
                        .unwrap_or_default();
                    let is_last = resp
                        .get("last_page")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                    all_tasks.extend(tasks);
                    if is_last {
                        break;
                    }
                    if let Some(limit) = cli.limit {
                        if all_tasks.len() >= limit {
                            all_tasks.truncate(limit);
                            break;
                        }
                    }
                    page += 1;
                }
                output.print_items(&all_tasks, TASK_FIELDS, "id");
            } else {
                let resp = client
                    .get(&format!("/v2/list/{}/task{}", list, query))
                    .await?;
                let mut tasks = resp
                    .get("tasks")
                    .and_then(|t| t.as_array())
                    .cloned()
                    .unwrap_or_default();
                if let Some(limit) = cli.limit {
                    tasks.truncate(limit);
                }
                output.print_items(&tasks, TASK_FIELDS, "id");
            }
            Ok(())
        }
        TaskCommands::Search {
            space,
            folder,
            list,
            status,
            assignee,
            tag,
        } => {
            let ws_id = resolve_workspace(cli)?;
            let mut params = Vec::new();
            if let Some(s) = &space {
                params.push(format!("space_ids[]={}", s));
            }
            if let Some(f) = &folder {
                params.push(format!("project_ids[]={}", f));
            }
            if let Some(l) = &list {
                params.push(format!("list_ids[]={}", l));
            }
            if let Some(statuses) = &status {
                for s in statuses {
                    params.push(format!("statuses[]={}", s));
                }
            }
            if let Some(assignees) = &assignee {
                for a in assignees {
                    params.push(format!("assignees[]={}", a));
                }
            }
            if let Some(tags) = &tag {
                for t in tags {
                    params.push(format!("tags[]={}", t));
                }
            }
            if let Some(page) = cli.page {
                params.push(format!("page={}", page));
            }
            let query = if params.is_empty() {
                String::new()
            } else {
                format!("?{}", params.join("&"))
            };
            let resp = client
                .get(&format!("/v2/team/{}/task{}", ws_id, query))
                .await?;
            let mut tasks = resp
                .get("tasks")
                .and_then(|t| t.as_array())
                .cloned()
                .unwrap_or_default();
            if let Some(limit) = cli.limit {
                tasks.truncate(limit);
            }
            output.print_items(&tasks, TASK_FIELDS, "id");
            Ok(())
        }
        TaskCommands::Get {
            id,
            subtasks,
            custom_task_id,
        } => {
            let mut params = Vec::new();
            if subtasks {
                params.push("include_subtasks=true".to_string());
            }
            if custom_task_id {
                params.push("custom_task_ids=true".to_string());
                let ws_id = resolve_workspace(cli)?;
                params.push(format!("team_id={}", ws_id));
            }
            let query = if params.is_empty() {
                String::new()
            } else {
                format!("?{}", params.join("&"))
            };
            let resp = client
                .get(&format!("/v2/task/{}{}", id, query))
                .await?;
            output.print_single(&resp, TASK_FIELDS, "id");
            Ok(())
        }
        TaskCommands::Create {
            list,
            name,
            description,
            status,
            priority,
            assignee,
            tag,
            due_date,
            parent,
        } => {
            let mut body = serde_json::json!({ "name": name });
            if let Some(d) = description {
                body["description"] = serde_json::Value::String(d);
            }
            if let Some(s) = status {
                body["status"] = serde_json::Value::String(s);
            }
            if let Some(p) = priority {
                body["priority"] = serde_json::json!(p);
            }
            if let Some(assignees) = assignee {
                let ids: Vec<serde_json::Value> = assignees
                    .iter()
                    .map(|a| serde_json::json!(a.parse::<i64>().unwrap_or(0)))
                    .collect();
                body["assignees"] = serde_json::Value::Array(ids);
            }
            if let Some(tags) = tag {
                body["tags"] = serde_json::json!(tags);
            }
            if let Some(d) = due_date {
                body["due_date"] = serde_json::Value::String(date_to_ms(&d)?);
            }
            if let Some(p) = parent {
                body["parent"] = serde_json::Value::String(p);
            }
            let resp = client
                .post(&format!("/v2/list/{}/task", list), &body)
                .await?;
            output.print_single(&resp, TASK_FIELDS, "id");
            Ok(())
        }
        TaskCommands::Update {
            id,
            name,
            status,
            priority,
            add_assignee,
            rem_assignee,
            description,
        } => {
            let mut body = serde_json::Map::new();
            if let Some(n) = name {
                body.insert("name".into(), serde_json::Value::String(n));
            }
            if let Some(s) = status {
                body.insert("status".into(), serde_json::Value::String(s));
            }
            if let Some(p) = priority {
                body.insert("priority".into(), serde_json::json!(p));
            }
            if let Some(d) = description {
                body.insert("description".into(), serde_json::Value::String(d));
            }
            // Assignee add/remove uses nested object
            if add_assignee.is_some() || rem_assignee.is_some() {
                let mut assignees = serde_json::Map::new();
                if let Some(add) = add_assignee {
                    let ids: Vec<serde_json::Value> = add
                        .iter()
                        .map(|a| serde_json::json!(a.parse::<i64>().unwrap_or(0)))
                        .collect();
                    assignees.insert("add".into(), serde_json::Value::Array(ids));
                }
                if let Some(rem) = rem_assignee {
                    let ids: Vec<serde_json::Value> = rem
                        .iter()
                        .map(|a| serde_json::json!(a.parse::<i64>().unwrap_or(0)))
                        .collect();
                    assignees.insert("rem".into(), serde_json::Value::Array(ids));
                }
                body.insert("assignees".into(), serde_json::Value::Object(assignees));
            }
            let resp = client
                .put(&format!("/v2/task/{}", id), &serde_json::Value::Object(body))
                .await?;
            output.print_single(&resp, TASK_FIELDS, "id");
            Ok(())
        }
        TaskCommands::Delete { id } => {
            client.delete(&format!("/v2/task/{}", id)).await?;
            output.print_message(&format!("Task {} deleted", id));
            Ok(())
        }
        TaskCommands::TimeInStatus { ids } => {
            if ids.len() == 1 {
                let resp = client
                    .get(&format!("/v2/task/{}/time_in_status", ids[0]))
                    .await?;
                if cli.output == "json" {
                    println!("{}", serde_json::to_string_pretty(&resp).unwrap());
                } else {
                    // Print status durations
                    if let Some(statuses) = resp.get("current_status").and_then(|v| v.as_object()) {
                        println!("Current: {} ({}ms)",
                            statuses.get("status").and_then(|v| v.as_str()).unwrap_or("-"),
                            statuses.get("total_time").and_then(|v| v.as_object())
                                .and_then(|o| o.get("by_minute"))
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0)
                        );
                    }
                    // Print all statuses
                    if let Some(statuses_arr) = resp.get("status_history").and_then(|v| v.as_array()) {
                        for s in statuses_arr {
                            let name = s.get("status").and_then(|v| v.as_str()).unwrap_or("-");
                            let time = s.get("total_time").and_then(|v| v.as_object())
                                .and_then(|o| o.get("by_minute"))
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);
                            println!("  {} — {}ms", name, time);
                        }
                    }
                }
            } else {
                // Bulk mode
                let task_ids = ids.join(",");
                let resp = client
                    .get(&format!("/v2/task/bulk_time_in_status/task_ids?task_ids={}", task_ids))
                    .await?;
                if cli.output == "json" {
                    println!("{}", serde_json::to_string_pretty(&resp).unwrap());
                } else {
                    // Print per-task summary
                    if let Some(obj) = resp.as_object() {
                        for (task_id, data) in obj {
                            let current = data
                                .get("current_status")
                                .and_then(|v| v.get("status"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("-");
                            println!("{}: {}", task_id, current);
                        }
                    }
                }
            }
            Ok(())
        }
    }
}

fn date_to_ms(date_str: &str) -> Result<String, CliError> {
    let naive = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|_| CliError::ClientError {
            message: format!("Invalid date '{}'. Use YYYY-MM-DD format.", date_str),
            status: 0,
        })?;
    let dt = naive.and_hms_opt(0, 0, 0).unwrap().and_utc();
    Ok((dt.timestamp_millis()).to_string())
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles.

- [ ] **Step 3: Commit**

```bash
git add src/commands/task.rs
git commit -m "feat: implement task CRUD, search, and time-in-status commands"
```

---

### Task 13: CLI Smoke Tests

**Files:**
- Create: `tests/test_cli.rs`

**Why:** Verify the binary parses all subcommands correctly and --help works for each resource group.

- [ ] **Step 1: Write CLI smoke tests**

`tests/test_cli.rs`:
```rust
use assert_cmd::Command;
use predicates::prelude::*;

fn clickup() -> Command {
    Command::cargo_bin("clickup").unwrap()
}

#[test]
fn test_help() {
    clickup()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("CLI for the ClickUp API"));
}

#[test]
fn test_version() {
    clickup()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("clickup 0.1.0"));
}

#[test]
fn test_setup_help() {
    clickup()
        .args(["setup", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("token"));
}

#[test]
fn test_auth_help() {
    clickup()
        .args(["auth", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("whoami"))
        .stdout(predicate::str::contains("check"));
}

#[test]
fn test_workspace_help() {
    clickup()
        .args(["workspace", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("seats"))
        .stdout(predicate::str::contains("plan"));
}

#[test]
fn test_space_help() {
    clickup()
        .args(["space", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("delete"));
}

#[test]
fn test_folder_help() {
    clickup()
        .args(["folder", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("delete"));
}

#[test]
fn test_list_help() {
    clickup()
        .args(["list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("add-task"))
        .stdout(predicate::str::contains("remove-task"));
}

#[test]
fn test_task_help() {
    clickup()
        .args(["task", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("search"))
        .stdout(predicate::str::contains("time-in-status"));
}

#[test]
fn test_no_subcommand_shows_help() {
    clickup()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test test_cli`
Expected: All 10 tests pass.

- [ ] **Step 3: Commit**

```bash
git add tests/test_cli.rs
git commit -m "test: add CLI smoke tests for all subcommands"
```

---

### Task 14: Documentation

**Files:**
- Modify: `CLAUDE.md`
- Modify: `README.md`

**Why:** Issue #8 requires CLAUDE.md for AI agent consumption and updated README.

- [ ] **Step 1: Write CLAUDE.md**

`CLAUDE.md`:
```markdown
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
```

- [ ] **Step 2: Update README.md**

Update the existing README.md with Rust CLI installation and usage instructions. Keep it concise — just replace the Go-specific content with Rust equivalents:

```markdown
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
```

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md README.md
git commit -m "docs: add CLAUDE.md for AI agents, update README for Rust CLI"
```

---

### Task 15: Final Build Verification

**Files:** None (verification only)

**Why:** Ensure everything compiles clean and all tests pass before tagging v0.1.

- [ ] **Step 1: Clean build**

Run: `cargo build --release`
Expected: Compiles without errors.

- [ ] **Step 2: Run all tests**

Run: `cargo test`
Expected: All tests pass (error, config, output, client, CLI smoke tests).

- [ ] **Step 3: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings. Fix any that appear.

- [ ] **Step 4: Verify binary size**

Run: `ls -lh target/release/clickup`
Expected: Reasonable binary size (should be under 10MB with static TLS).

- [ ] **Step 5: Tag the release**

```bash
git tag v0.1.0
```

- [ ] **Step 6: Final commit if any clippy fixes were needed**

```bash
git add -A
git commit -m "fix: address clippy warnings for v0.1.0 release"
```
