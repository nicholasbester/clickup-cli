use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum TagCommands {
    /// List tags in a space
    List {
        /// Space ID
        #[arg(long)]
        space: String,
    },
    /// Create a tag in a space
    Create {
        /// Space ID
        #[arg(long)]
        space: String,
        /// Tag name
        #[arg(long)]
        name: String,
        /// Foreground color (hex)
        #[arg(long)]
        fg_color: Option<String>,
        /// Background color (hex)
        #[arg(long)]
        bg_color: Option<String>,
    },
    /// Update a tag in a space
    Update {
        /// Space ID
        #[arg(long)]
        space: String,
        /// Tag name (current)
        #[arg(long)]
        tag: String,
        /// New tag name
        #[arg(long)]
        name: Option<String>,
        /// New foreground color (hex)
        #[arg(long)]
        fg_color: Option<String>,
        /// New background color (hex)
        #[arg(long)]
        bg_color: Option<String>,
    },
    /// Delete a tag from a space
    Delete {
        /// Space ID
        #[arg(long)]
        space: String,
        /// Tag name
        #[arg(long)]
        tag: String,
    },
}

const TAG_FIELDS: &[&str] = &["name", "tag_fg", "tag_bg"];

pub async fn execute(command: TagCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        TagCommands::List { space } => {
            let resp = client
                .get(&format!("/v2/space/{}/tag", space))
                .await?;
            let tags = resp
                .get("tags")
                .and_then(|t| t.as_array())
                .cloned()
                .unwrap_or_default();
            output.print_items(&tags, TAG_FIELDS, "name");
            Ok(())
        }
        TagCommands::Create {
            space,
            name,
            fg_color,
            bg_color,
        } => {
            let mut tag_obj = serde_json::json!({ "name": name });
            if let Some(fg) = fg_color {
                tag_obj["tag_fg"] = serde_json::Value::String(fg);
            }
            if let Some(bg) = bg_color {
                tag_obj["tag_bg"] = serde_json::Value::String(bg);
            }
            let body = serde_json::json!({ "tag": tag_obj });
            let resp = client
                .post(&format!("/v2/space/{}/tag", space), &body)
                .await?;
            output.print_single(&resp, TAG_FIELDS, "name");
            Ok(())
        }
        TagCommands::Update {
            space,
            tag,
            name,
            fg_color,
            bg_color,
        } => {
            let mut tag_obj = serde_json::Map::new();
            if let Some(n) = name {
                tag_obj.insert("name".into(), serde_json::Value::String(n));
            }
            // NOTE: Update uses fg_color/bg_color (not tag_fg/tag_bg like Create)
            if let Some(fg) = fg_color {
                tag_obj.insert("fg_color".into(), serde_json::Value::String(fg));
            }
            if let Some(bg) = bg_color {
                tag_obj.insert("bg_color".into(), serde_json::Value::String(bg));
            }
            let body = serde_json::json!({ "tag": serde_json::Value::Object(tag_obj) });
            let resp = client
                .put(&format!("/v2/space/{}/tag/{}", space, tag), &body)
                .await?;
            output.print_single(&resp, TAG_FIELDS, "name");
            Ok(())
        }
        TagCommands::Delete { space, tag } => {
            client
                .delete(&format!("/v2/space/{}/tag/{}", space, tag))
                .await?;
            output.print_message(&format!("Tag '{}' deleted from space {}", tag, space));
            Ok(())
        }
    }
}
