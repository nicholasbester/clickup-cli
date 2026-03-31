use clap::Parser;
use clickup_cli::{commands, Cli, Commands};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let exit_code = run(cli).await;
    std::process::exit(exit_code);
}

async fn run(mut cli: Cli) -> i32 {
    use std::mem;
    let output = cli.output.clone();
    // Replace command with a dummy so we can borrow cli while owning command.
    // We use a placeholder Commands::Setup variant temporarily.
    let command = mem::replace(
        &mut cli.command,
        Commands::Setup(commands::setup::SetupArgs { token: None }),
    );
    let result = match command {
        Commands::Setup(args) => commands::setup::execute(args, &cli).await,
        Commands::Auth { command } => commands::auth::execute(command, &cli).await,
        Commands::Workspace { command } => commands::workspace::execute(command, &cli).await,
        Commands::Space { command } => commands::space::execute(command, &cli).await,
        Commands::Folder { command } => commands::folder::execute(command, &cli).await,
        Commands::List { command } => commands::list::execute(command, &cli).await,
        Commands::Task { command } => commands::task::execute(command, &cli).await,
        Commands::Checklist { command } => commands::checklist::execute(command, &cli).await,
        Commands::Comment { command } => commands::comment::execute(command, &cli).await,
        Commands::Tag { command } => commands::tag::execute(command, &cli).await,
        Commands::Field { command } => commands::field::execute(command, &cli).await,
        Commands::TaskType { command } => commands::task_type::execute(command, &cli).await,
        Commands::Attachment { command } => commands::attachment::execute(command, &cli).await,
        Commands::Time { command } => commands::time::execute(command, &cli).await,
        Commands::Goal { command } => commands::goal::execute(command, &cli).await,
        Commands::View { command } => commands::view::execute(command, &cli).await,
        Commands::Member { command } => commands::member::execute(command, &cli).await,
        Commands::User { command } => commands::user::execute(command, &cli).await,
        Commands::Chat { command } => commands::chat::execute(command, &cli).await,
        Commands::Doc { command } => commands::doc::execute(command, &cli).await,
        Commands::Webhook { command } => commands::webhook::execute(command, &cli).await,
        Commands::Template { command } => commands::template::execute(command, &cli).await,
        Commands::Guest { command } => commands::guest::execute(command, &cli).await,
        Commands::Group { command } => commands::group::execute(command, &cli).await,
        Commands::Role { command } => commands::role::execute(command, &cli).await,
        Commands::Shared { command } => commands::shared::execute(command, &cli).await,
        Commands::AuditLog { command } => commands::audit_log::execute(command, &cli).await,
        Commands::Acl { command } => commands::acl::execute(command, &cli).await,
        Commands::AgentConfig { command } => commands::agent_config::execute(command, &cli).await,
        Commands::Mcp { command } => commands::mcp_cmd::execute(command).await,
        Commands::Status => commands::status::execute(&cli).await,
        Commands::Completions { shell } => commands::completions::execute(shell),
    };
    match result {
        Ok(()) => 0,
        Err(e) => {
            e.print(&output);
            e.exit_code()
        }
    }
}
