use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::error::CliError;
use crate::git;
use crate::output::OutputConfig;
use crate::Cli;
use clap::Subcommand;

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
        /// Comment text (use @path to read from a file, @- for stdin, @@ for a literal leading @). Note: ClickUp's v2 comment API does not render markdown; markdown syntax is stored as literal text.
        #[arg(long, value_parser = crate::input::resolve_value_arg)]
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
        /// New comment text (use @path to read from a file, @- for stdin, @@ for a literal leading @). Note: ClickUp's v2 comment API does not render markdown; markdown syntax is stored as literal text.
        #[arg(long, value_parser = crate::input::resolve_value_arg)]
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
        /// Reply text (use @path to read from a file, @- for stdin, @@ for a literal leading @). Note: ClickUp's v2 comment API does not render markdown; markdown syntax is stored as literal text.
        #[arg(long, value_parser = crate::input::resolve_value_arg)]
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
            let base = if let Some(id) = list {
                format!("/v2/list/{}/comment", id)
            } else if let Some(id) = view {
                format!("/v2/view/{}/comment", id)
            } else if let Some(resolved) = git::resolve_task(cli, task.as_deref(), true)? {
                format!("/v2/task/{}/comment", resolved.id)
            } else {
                return Err(CliError::ClientError {
                    message: "One of --task, --list, or --view is required".to_string(),
                    status: 0,
                });
            };
            let comments = crate::commands::pagination::walk_start_id(
                cli,
                &client,
                "comments",
                |start, start_id| match (start, start_id) {
                    (Some(s), Some(sid)) => format!("{}?start={}&start_id={}", base, s, sid),
                    _ => base.clone(),
                },
            )
            .await?;
            let truncated: Vec<serde_json::Value> = comments
                .into_iter()
                .map(|mut c| {
                    if let Some(text) = c.get("comment_text").and_then(|v| v.as_str()) {
                        // Truncate by chars (not bytes) so the 60-byte boundary
                        // can't land inside a multibyte UTF-8 codepoint.
                        let truncated = if text.chars().count() > 60 {
                            let head: String = text.chars().take(60).collect();
                            format!("{}…", head)
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
            let (url, resp) = if let Some(id) = list {
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
            } else if let Some(resolved) = git::resolve_task(cli, task.as_deref(), true)? {
                let mut body = serde_json::json!({
                    "comment_text": text,
                    "notify_all": notify_all,
                });
                if let Some(a) = assignee {
                    body["assignee"] = serde_json::json!(a);
                }
                let r = client
                    .post(&format!("/v2/task/{}/comment", resolved.id), &body)
                    .await?;
                (format!("/v2/task/{}/comment", resolved.id), r)
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
            let resp = client.put(&format!("/v2/comment/{}", id), &body).await?;
            output.print_single(&resp, COMMENT_FIELDS, "id");
            Ok(())
        }
        CommentCommands::Delete { id } => {
            client.delete(&format!("/v2/comment/{}", id)).await?;
            output.print_message(&format!("Comment {} deleted", id));
            Ok(())
        }
        CommentCommands::Replies { id } => {
            let comments = crate::commands::pagination::walk_start_id(
                cli,
                &client,
                "comments",
                |start, start_id| match (start, start_id) {
                    (Some(s), Some(sid)) => {
                        format!("/v2/comment/{}/reply?start={}&start_id={}", id, s, sid)
                    }
                    _ => format!("/v2/comment/{}/reply", id),
                },
            )
            .await?;
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
