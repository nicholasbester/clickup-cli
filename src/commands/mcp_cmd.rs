use crate::error::CliError;
use crate::mcp::filter::{Filter, RawFilter};
use clap::Subcommand;

#[derive(Subcommand)]
pub enum McpCommands {
    /// Start the MCP server (reads JSON-RPC from stdin, writes to stdout).
    Serve {
        /// Preset tool bundle: `all` (default), `read`, `safe`.
        #[arg(long, value_name = "NAME")]
        profile: Option<String>,

        /// Shortcut for `--profile read`.
        #[arg(long)]
        read_only: bool,

        /// Include only tools in these resource groups (comma-separated).
        #[arg(long, value_name = "LIST", value_delimiter = ',')]
        groups: Option<Vec<String>>,

        /// Drop tools in these resource groups (comma-separated).
        #[arg(long, value_name = "LIST", value_delimiter = ',')]
        exclude_groups: Option<Vec<String>>,

        /// Include only these tools by exact name (comma-separated).
        #[arg(long, value_name = "LIST", value_delimiter = ',')]
        tools: Option<Vec<String>>,

        /// Drop these tools by exact name (comma-separated).
        #[arg(long, value_name = "LIST", value_delimiter = ',')]
        exclude_tools: Option<Vec<String>>,
    },
}

fn env_list(var: &str) -> Option<Vec<String>> {
    std::env::var(var).ok().map(|v| {
        v.split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    })
}

fn env_bool(var: &str) -> bool {
    matches!(
        std::env::var(var).ok().as_deref(),
        Some("1" | "true" | "yes" | "on")
    )
}

fn env_string(var: &str) -> Option<String> {
    std::env::var(var).ok().filter(|s| !s.is_empty())
}

pub async fn execute(command: McpCommands) -> Result<(), CliError> {
    match command {
        McpCommands::Serve {
            profile,
            read_only,
            groups,
            exclude_groups,
            tools,
            exclude_tools,
        } => {
            let raw = RawFilter {
                profile: profile.or_else(|| env_string("CLICKUP_MCP_PROFILE")),
                read_only: read_only || env_bool("CLICKUP_MCP_READ_ONLY"),
                groups: groups.or_else(|| env_list("CLICKUP_MCP_GROUPS")),
                exclude_groups: exclude_groups.or_else(|| env_list("CLICKUP_MCP_EXCLUDE_GROUPS")),
                tools: tools.or_else(|| env_list("CLICKUP_MCP_TOOLS")),
                exclude_tools: exclude_tools.or_else(|| env_list("CLICKUP_MCP_EXCLUDE_TOOLS")),
            };
            let filter = Filter::resolve(raw).map_err(|e| CliError::ConfigError(e.to_string()))?;
            crate::mcp::serve(filter)
                .await
                .map_err(|e| CliError::ConfigError(e.to_string()))
        }
    }
}
