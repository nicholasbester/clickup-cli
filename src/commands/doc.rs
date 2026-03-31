use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum DocCommands {
    /// List docs in the workspace
    List {
        /// Filter by creator user ID
        #[arg(long)]
        creator: Option<String>,
        /// Include archived docs
        #[arg(long)]
        archived: bool,
    },
    /// Create a doc
    Create {
        /// Doc name
        #[arg(long)]
        name: String,
        /// Visibility: PUBLIC, PRIVATE, or PERSONAL
        #[arg(long)]
        visibility: Option<String>,
        /// Parent type: SPACE, FOLDER, or LIST
        #[arg(long)]
        parent_type: Option<String>,
        /// Parent ID
        #[arg(long)]
        parent_id: Option<String>,
    },
    /// Get a doc by ID
    Get {
        /// Doc ID
        id: String,
    },
    /// List pages in a doc
    Pages {
        /// Doc ID
        id: String,
        /// Include page content
        #[arg(long)]
        content: bool,
        /// Maximum page depth
        #[arg(long)]
        max_depth: Option<u32>,
    },
    /// Add a page to a doc
    #[command(name = "add-page")]
    AddPage {
        /// Doc ID
        doc_id: String,
        /// Page name
        #[arg(long)]
        name: String,
        /// Parent page ID
        #[arg(long)]
        parent_page: Option<String>,
        /// Page content
        #[arg(long)]
        content: Option<String>,
    },
    /// Get a specific page from a doc
    Page {
        /// Doc ID
        doc_id: String,
        /// Page ID
        page_id: String,
    },
    /// Edit a doc page
    #[command(name = "edit-page")]
    EditPage {
        /// Doc ID
        doc_id: String,
        /// Page ID
        page_id: String,
        /// Page content
        #[arg(long)]
        content: String,
        /// Content edit mode: replace, append, or prepend
        #[arg(long, default_value = "replace")]
        mode: String,
    },
}

const DOC_FIELDS: &[&str] = &["id", "name", "visibility", "date_created"];
const PAGE_FIELDS: &[&str] = &["id", "name", "date_created", "date_updated"];

pub async fn execute(command: DocCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let ws_id = resolve_workspace(cli)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);
    let base = format!("/v3/workspaces/{}/docs", ws_id);

    match command {
        DocCommands::List { creator, archived } => {
            let mut params = Vec::new();
            if let Some(c) = &creator {
                params.push(format!("creator_id={}", c));
            }
            if archived {
                params.push("archived=true".to_string());
            }
            let query = if params.is_empty() {
                String::new()
            } else {
                format!("?{}", params.join("&"))
            };
            let resp = client.get(&format!("{}{}", base, query)).await?;
            let mut docs = resp
                .get("docs")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            if let Some(limit) = cli.limit {
                docs.truncate(limit);
            }
            output.print_items(&docs, DOC_FIELDS, "id");
            Ok(())
        }
        DocCommands::Create {
            name,
            visibility,
            parent_type,
            parent_id,
        } => {
            let mut body = serde_json::json!({ "name": name });
            if let Some(v) = visibility {
                body["visibility"] = serde_json::Value::String(v);
            }
            if parent_type.is_some() || parent_id.is_some() {
                let mut parent = serde_json::Map::new();
                if let Some(pt) = parent_type {
                    parent.insert("type".into(), serde_json::Value::String(pt));
                }
                if let Some(pi) = parent_id {
                    parent.insert("id".into(), serde_json::Value::String(pi));
                }
                body["parent"] = serde_json::Value::Object(parent);
            }
            let resp = client.post(&base, &body).await?;
            output.print_single(&resp, DOC_FIELDS, "id");
            Ok(())
        }
        DocCommands::Get { id } => {
            let resp = client.get(&format!("{}/{}", base, id)).await?;
            output.print_single(&resp, DOC_FIELDS, "id");
            Ok(())
        }
        DocCommands::Pages {
            id,
            content,
            max_depth,
        } => {
            if content || max_depth.is_some() {
                let mut params = Vec::new();
                if content {
                    params.push("content=true".to_string());
                }
                if let Some(depth) = max_depth {
                    params.push(format!("max_page_depth={}", depth));
                }
                let query = if params.is_empty() {
                    String::new()
                } else {
                    format!("?{}", params.join("&"))
                };
                let resp = client
                    .get(&format!("{}/{}/pages{}", base, id, query))
                    .await?;
                let mut pages = resp
                    .get("pages")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_else(|| resp.as_array().cloned().unwrap_or_default());
                if let Some(limit) = cli.limit {
                    pages.truncate(limit);
                }
                output.print_items(&pages, PAGE_FIELDS, "id");
            } else {
                let resp = client
                    .get(&format!("{}/{}/page_listing", base, id))
                    .await?;
                let mut pages = resp
                    .get("pages")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_else(|| resp.as_array().cloned().unwrap_or_default());
                if let Some(limit) = cli.limit {
                    pages.truncate(limit);
                }
                output.print_items(&pages, PAGE_FIELDS, "id");
            }
            Ok(())
        }
        DocCommands::AddPage {
            doc_id,
            name,
            parent_page,
            content,
        } => {
            let mut body = serde_json::json!({ "name": name });
            if let Some(pp) = parent_page {
                body["parent_page_id"] = serde_json::Value::String(pp);
            }
            if let Some(c) = content {
                body["content"] = serde_json::Value::String(c);
            }
            let resp = client
                .post(&format!("{}/{}/pages", base, doc_id), &body)
                .await?;
            output.print_single(&resp, PAGE_FIELDS, "id");
            Ok(())
        }
        DocCommands::Page { doc_id, page_id } => {
            let resp = client
                .get(&format!("{}/{}/pages/{}", base, doc_id, page_id))
                .await?;
            output.print_single(&resp, PAGE_FIELDS, "id");
            Ok(())
        }
        DocCommands::EditPage {
            doc_id,
            page_id,
            content,
            mode,
        } => {
            let body = serde_json::json!({
                "content": content,
                "content_edit_mode": mode,
            });
            let resp = client
                .put(&format!("{}/{}/pages/{}", base, doc_id, page_id), &body)
                .await?;
            output.print_single(&resp, PAGE_FIELDS, "id");
            Ok(())
        }
    }
}
