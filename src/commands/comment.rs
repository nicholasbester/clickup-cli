use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum CommentCommands {
    /// List comments on a task, list, or view
    List {
        /// Task ID
        #[arg(long, conflicts_with_all = ["list", "view"])]
        task: Option<String>,
        /// List ID
        #[arg(long, conflicts_with_all = ["task", "view"])]
        list: Option<String>,
        /// View ID
        #[arg(long, conflicts_with_all = ["task", "list"])]
        view: Option<String>,
    },
    /// Create a comment on a task, list, or view
    Create {
        /// Task ID
        #[arg(long, conflicts_with_all = ["list", "view"])]
        task: Option<String>,
        /// List ID
        #[arg(long, conflicts_with_all = ["task", "view"])]
        list: Option<String>,
        /// View ID
        #[arg(long, conflicts_with_all = ["task", "list"])]
        view: Option<String>,
        /// Comment text
        #[arg(long)]
        text: String,
        /// Assignee user ID (task comments only)
        #[arg(long)]
        assignee: Option<i64>,
        /// Notify all watchers (task comments only)
        #[arg(long)]
        notify_all: bool,
    },
    /// Update a comment
    Update {
        /// Comment ID
        id: String,
        /// New comment text
        #[arg(long)]
        text: String,
        /// Mark as resolved
        #[arg(long)]
        resolved: bool,
        /// Assignee user ID
        #[arg(long)]
        assignee: Option<i64>,
    },
    /// Delete a comment
    Delete {
        /// Comment ID
        id: String,
    },
    /// List threaded replies on a comment
    Replies {
        /// Comment ID
        id: String,
    },
    /// Reply to a comment
    Reply {
        /// Comment ID
        id: String,
        /// Reply text
        #[arg(long)]
        text: String,
        /// Assignee user ID
        #[arg(long)]
        assignee: Option<i64>,
    },
}

const COMMENT_FIELDS: &[&str] = &["id", "user", "date", "comment_text"];

pub async fn execute(command: CommentCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        CommentCommands::List { task, list, view } => {
            let (url, key) = if let Some(id) = task {
                (format!("/v2/task/{}/comment", id), "comments")
            } else if let Some(id) = list {
                (format!("/v2/list/{}/comment", id), "comments")
            } else if let Some(id) = view {
                (format!("/v2/view/{}/comment", id), "comments")
            } else {
                return Err(CliError::ClientError {
                    message: "One of --task, --list, or --view is required".to_string(),
                    status: 0,
                });
            };
            let resp = client.get(&url).await?;
            let comments = resp
                .get(key)
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default();
            let truncated: Vec<serde_json::Value> = comments
                .into_iter()
                .map(|mut c| {
                    if let Some(text) = c.get("comment_text").and_then(|v| v.as_str()) {
                        let truncated = if text.len() > 60 {
                            format!("{}…", &text[..60])
                        } else {
                            text.to_string()
                        };
                        c["comment_text"] = serde_json::Value::String(truncated);
                    }
                    c
                })
                .collect();
            output.print_items(&truncated, COMMENT_FIELDS, "id");
            Ok(())
        }
        CommentCommands::Create {
            task,
            list,
            view,
            text,
            assignee,
            notify_all,
        } => {
            let (url, resp) = if let Some(id) = task {
                let mut body = serde_json::json!({
                    "comment_text": text,
                    "notify_all": notify_all,
                });
                if let Some(a) = assignee {
                    body["assignee"] = serde_json::json!(a);
                }
                let r = client
                    .post(&format!("/v2/task/{}/comment", id), &body)
                    .await?;
                (format!("/v2/task/{}/comment", id), r)
            } else if let Some(id) = list {
                let body = serde_json::json!({ "comment_text": text });
                let r = client
                    .post(&format!("/v2/list/{}/comment", id), &body)
                    .await?;
                (format!("/v2/list/{}/comment", id), r)
            } else if let Some(id) = view {
                let body = serde_json::json!({ "comment_text": text });
                let r = client
                    .post(&format!("/v2/view/{}/comment", id), &body)
                    .await?;
                (format!("/v2/view/{}/comment", id), r)
            } else {
                return Err(CliError::ClientError {
                    message: "One of --task, --list, or --view is required".to_string(),
                    status: 0,
                });
            };
            let _ = url;
            output.print_single(&resp, COMMENT_FIELDS, "id");
            Ok(())
        }
        CommentCommands::Update {
            id,
            text,
            resolved,
            assignee,
        } => {
            let mut body = serde_json::json!({ "comment_text": text });
            if resolved {
                body["resolved"] = serde_json::Value::Bool(true);
            }
            if let Some(a) = assignee {
                body["assignee"] = serde_json::json!(a);
            }
            let resp = client
                .put(&format!("/v2/comment/{}", id), &body)
                .await?;
            output.print_single(&resp, COMMENT_FIELDS, "id");
            Ok(())
        }
        CommentCommands::Delete { id } => {
            client.delete(&format!("/v2/comment/{}", id)).await?;
            output.print_message(&format!("Comment {} deleted", id));
            Ok(())
        }
        CommentCommands::Replies { id } => {
            let resp = client
                .get(&format!("/v2/comment/{}/reply", id))
                .await?;
            let comments = resp
                .get("comments")
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default();
            output.print_items(&comments, COMMENT_FIELDS, "id");
            Ok(())
        }
        CommentCommands::Reply { id, text, assignee } => {
            let mut body = serde_json::json!({ "comment_text": text });
            if let Some(a) = assignee {
                body["assignee"] = serde_json::json!(a);
            }
            let resp = client
                .post(&format!("/v2/comment/{}/reply", id), &body)
                .await?;
            output.print_single(&resp, COMMENT_FIELDS, "id");
            Ok(())
        }
    }
}
