use clap::Subcommand;
use crate::error::CliError;
use crate::Cli;
use std::path::PathBuf;

/// Known AI agent instruction files, checked in order
const AGENT_FILES: &[&str] = &[
    "CLAUDE.md",
    "agent.md",
    "AGENT.md",
    ".cursorrules",
    ".github/copilot-instructions.md",
    "AGENTS.md",
    "AI.md",
];

#[derive(Subcommand)]
pub enum AgentConfigCommands {
    /// Print compressed CLI reference for AI agent instruction files
    Show,
    /// Inject CLI reference into a file (auto-detects or creates agent instruction file)
    Inject {
        /// Target file (omit to auto-detect: CLAUDE.md, agent.md, .cursorrules, etc.)
        file: Option<PathBuf>,
    },
    /// Initialize project-level ClickUp config (.clickup.toml in current directory)
    Init {
        /// API token
        #[arg(long)]
        token: Option<String>,
        /// Workspace ID
        #[arg(long)]
        workspace: Option<String>,
    },
}

const AGENT_REFERENCE: &str = "<!-- clickup-cli:begin -->To interface with ClickUp, use the `clickup` CLI (LLM-agnostic, works with any AI agent). Pattern: `clickup <resource> <action> [ID] [flags]`. Global flags: --output table|json|json-compact|csv, --fields LIST, -q (IDs only), --no-header, --all (paginate), --limit N, --page N, --token TOKEN, --workspace ID, --timeout SECS. Commands: setup [--token T]; auth whoami|check; workspace list|seats|plan; space list [--archived]|get ID|create --name N [--private]|update ID [--name N]|delete ID; folder list --space ID|get ID|create --space ID --name N|update ID --name N|delete ID; list list --folder ID|--space ID|get ID|create --folder ID|--space ID --name N [--content T] [--due-date DATE]|update ID|delete ID|add-task LIST TASK|remove-task LIST TASK; task list --list ID [--status S] [--assignee ID] [--tag T] [--include-closed]|search [--space ID] [--status S]|get ID [--subtasks] [--custom-task-id]|create --list ID --name N [--description T] [--status S] [--priority 1-4] [--assignee ID] [--tag T] [--due-date DATE] [--parent ID]|update ID [--name N] [--status S] [--priority N] [--add-assignee ID] [--rem-assignee ID]|delete ID|time-in-status ID...|add-tag ID TAG|remove-tag ID TAG|add-dep ID --depends-on ID|remove-dep ID --depends-on ID|link ID TARGET|unlink ID TARGET|move ID --list ID|set-estimate ID --assignee ID --time MS|replace-estimates ID --assignee ID --time MS; checklist create --task ID --name N|update ID [--name N]|delete ID|add-item ID --name N|update-item ID ITEM [--name N] [--resolved]|delete-item ID ITEM; comment list --task ID|--list ID|--view ID|create --task ID|--list ID|--view ID --text T [--notify-all]|update ID --text T [--resolved]|delete ID|replies ID|reply ID --text T; tag list --space ID|create --space ID --name N [--fg-color H] [--bg-color H]|update --space ID --tag N [--name NEW]|delete --space ID --tag N; field list --list ID|--folder ID|--space ID|--workspace-level|set TASK FIELD --value V|unset TASK FIELD; task-type list; attachment list --task ID|upload --task ID FILE; time list [--start-date D] [--end-date D] [--task ID]|get ID|current|create --start D --duration MS [--task ID]|update ID|delete ID|start [--task ID]|stop|tags|add-tags --entry-id ID --tag N|remove-tags --entry-id ID --tag N|rename-tag --name OLD --new-name NEW|history ID; goal list|get ID|create --name N --due-date D|update ID|delete ID|add-kr ID --name N --type T --steps-start N --steps-end N|update-kr ID --steps-current N|delete-kr ID; view list --workspace-level|--space ID|--folder ID|--list ID|get ID|create --name N --type T --space ID|--folder ID|--list ID|update ID|delete ID|tasks ID; member list --task ID|--list ID; user invite --email E|get ID|update ID|remove ID; chat channel-list|channel-create --name N|channel-get ID|channel-update ID|channel-delete ID|channel-followers ID|channel-members ID|dm USER...|message-list --channel ID|message-send --channel ID --text T|message-update ID --text T|message-delete ID|reaction-list MSG|reaction-add MSG --emoji E|reaction-remove MSG EMOJI|reply-list MSG|reply-send MSG --text T|tagged-users MSG; doc list|create --name N|get ID|pages ID [--content]|add-page DOC --name N [--content T]|page DOC PAGE|edit-page DOC PAGE --content T [--mode replace|append|prepend]; webhook list|create --endpoint URL --event E|update ID --endpoint URL --event E|delete ID; template list|apply-task TPL --list ID --name N|apply-list TPL --folder ID|--space ID --name N|apply-folder TPL --space ID --name N; guest invite --email E|get ID|update ID|remove ID|share-task TASK GUEST --permission P|unshare-task TASK GUEST|share-list LIST GUEST --permission P|unshare-list LIST GUEST|share-folder FOLDER GUEST --permission P|unshare-folder FOLDER GUEST; group list|create --name N --member ID|update ID [--add-member ID] [--rem-member ID]|delete ID; role list; shared list; audit-log query --type T [--user-id ID] [--start-date D] [--end-date D]; acl update TYPE ID [--private] [--body JSON]. Priority: 1=Urgent 2=High 3=Normal 4=Low. Dates: YYYY-MM-DD. All timestamps Unix ms. team_id=workspace_id in API. Exit codes: 0=ok 1=client-error 2=auth 3=not-found 4=rate-limited 5=server-error. Config: ~/.config/clickup-cli/config.toml or .clickup.toml (project-level). Setup: `clickup setup --token pk_XXX`.<!-- clickup-cli:end -->";

/// Find an existing agent instruction file in the current directory, or default to CLAUDE.md
fn detect_agent_file() -> PathBuf {
    for name in AGENT_FILES {
        let path = PathBuf::from(name);
        if path.exists() {
            return path;
        }
    }
    // No existing file found — default to CLAUDE.md
    PathBuf::from("CLAUDE.md")
}

pub async fn execute(command: AgentConfigCommands, _cli: &Cli) -> Result<(), CliError> {
    match command {
        AgentConfigCommands::Show => {
            println!("{}", AGENT_REFERENCE);
            Ok(())
        }
        AgentConfigCommands::Inject { file } => {
            let file = file.unwrap_or_else(detect_agent_file);
            let begin_marker = "<!-- clickup-cli:begin -->";
            let end_marker = "<!-- clickup-cli:end -->";

            let existing = if file.exists() {
                std::fs::read_to_string(&file)?
            } else {
                String::new()
            };

            let new_content = if existing.contains(begin_marker) && existing.contains(end_marker) {
                let before = existing.split(begin_marker).next().unwrap_or("");
                let after = existing.split(end_marker).nth(1).unwrap_or("");
                format!("{}{}{}", before, AGENT_REFERENCE, after)
            } else if existing.is_empty() {
                format!("# Project\n\n{}\n", AGENT_REFERENCE)
            } else {
                format!("{}\n\n{}\n", existing.trim_end(), AGENT_REFERENCE)
            };

            if let Some(parent) = file.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            std::fs::write(&file, new_content)?;
            eprintln!("CLI reference injected into {}", file.display());
            Ok(())
        }
        AgentConfigCommands::Init { token, workspace } => {
            let path = PathBuf::from(".clickup.toml");
            if path.exists() {
                eprintln!("Project config already exists: {}", path.display());
                eprintln!("Edit .clickup.toml directly to update.");
                return Ok(());
            }

            let mut content = String::from("[auth]\n");
            if let Some(t) = token {
                content.push_str(&format!("token = \"{}\"\n", t));
            } else {
                content.push_str("# token = \"pk_...\"\n");
            }
            content.push_str("\n[defaults]\n");
            if let Some(ws) = workspace {
                content.push_str(&format!("workspace_id = \"{}\"\n", ws));
            } else {
                content.push_str("# workspace_id = \"...\"\n");
            }

            std::fs::write(&path, &content)?;
            eprintln!("Project config created: .clickup.toml");
            eprintln!("Add .clickup.toml to .gitignore if it contains a token.");
            Ok(())
        }
    }
}
