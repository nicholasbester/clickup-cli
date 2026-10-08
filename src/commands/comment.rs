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
        /// Boundary timestamp in Unix ms for continuing a listing.
        /// Pair with --start-id.
        #[arg(long, requires = "start_id")]
        start: Option<i64>,
        /// Boundary comment id for continuing a listing. Pair with --start.
        #[arg(long = "start-id", requires = "start")]
        start_id: Option<String>,
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
        /// Comment text (use @path to read from a file, @- for stdin, @@ for a literal leading @). `@Display Name` / `<@user_id>` resolve to real ClickUp mentions (tag ops). Markdown is literal unless --markdown is set.
        #[arg(long, value_parser = crate::input::resolve_value_arg)]
        text: String,
        /// Assignee user ID (task comments only)
        #[arg(long)]
        assignee: Option<i64>,
        /// Notify all watchers (task comments only)
        #[arg(long)]
        notify_all: bool,
        /// Parse --text as markdown and submit ClickUp rich formatting
        /// (bold/italic/code/links, lists, code blocks; headings render
        /// bold, blockquotes indent, tables/strikethrough degrade to text)
        #[arg(long)]
        markdown: bool,
    },
    /// Update a comment
    Update {
        /// Comment ID
        id: String,
        /// New comment text (use @path to read from a file, @- for stdin, @@ for a literal leading @). `@Display Name` / `<@user_id>` resolve to real ClickUp mentions (tag ops); the whole body is replaced, so restate every mention to keep. Markdown is literal unless --markdown is set.
        #[arg(long, value_parser = crate::input::resolve_value_arg)]
        text: String,
        /// Mark as resolved
        #[arg(long)]
        resolved: bool,
        /// Assignee user ID
        #[arg(long)]
        assignee: Option<i64>,
        /// Parse --text as markdown and submit ClickUp rich formatting
        /// (bold/italic/code/links, lists, code blocks; headings render
        /// bold, blockquotes indent, tables/strikethrough degrade to text)
        #[arg(long)]
        markdown: bool,
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
        /// Boundary timestamp in Unix ms for continuing a listing.
        /// Pair with --start-id.
        #[arg(long, requires = "start_id")]
        start: Option<i64>,
        /// Boundary comment id for continuing a listing. Pair with --start.
        #[arg(long = "start-id", requires = "start")]
        start_id: Option<String>,
    },
    /// Reply to a comment
    Reply {
        /// Comment ID
        id: String,
        /// Reply text (use @path to read from a file, @- for stdin, @@ for a literal leading @). `@Display Name` / `<@user_id>` resolve to real ClickUp mentions (tag ops). Markdown is literal unless --markdown is set.
        #[arg(long, value_parser = crate::input::resolve_value_arg)]
        text: String,
        /// Assignee user ID
        #[arg(long)]
        assignee: Option<i64>,
        /// Parse --text as markdown and submit ClickUp rich formatting
        /// (bold/italic/code/links, lists, code blocks; headings render
        /// bold, blockquotes indent, tables/strikethrough degrade to text)
        #[arg(long)]
        markdown: bool,
    },
}

const COMMENT_FIELDS: &[&str] = &["id", "user", "date", "comment_text"];

/// Load workspace members for @mention resolution. Empty on fetch failure
/// so a comment still posts (as plain `@Name` text).
pub(crate) async fn mention_users(
    client: &ClickUpClient,
    workspace_id: Option<&str>,
) -> Vec<crate::markdown_ops::MentionUser> {
    match client.get("/v2/team").await {
        Ok(resp) => crate::markdown_ops::users_from_teams(&resp, workspace_id),
        Err(_) => Vec::new(),
    }
}

pub(crate) async fn comment_body_enriched(
    client: &ClickUpClient,
    workspace_id: Option<&str>,
    markdown: bool,
    text: &str,
) -> serde_json::Value {
    let mut body = crate::markdown_ops::comment_body(markdown, text);
    if text.contains('@') {
        let users = mention_users(client, workspace_id).await;
        body = crate::markdown_ops::apply_mentions_to_body(body, &users);
    }
    body
}

pub async fn execute(command: CommentCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        CommentCommands::List {
            task,
            list,
            view,
            start,
            start_id,
        } => {
            let base = if let Some(id) = list {
                format!("/v2/list/{}/comment", id)
            } else if let Some(id) = view {
                format!("/v2/view/{}/comment", id)
            } else if let Some(resolved) = git::resolve_task(cli, task.as_deref(), true)? {
                let q = crate::commands::workspace::custom_task_query(cli, &resolved)?;
                format!("/v2/task/{}/comment{}", resolved.id, q)
            } else {
                return Err(CliError::ClientError {
                    message: "One of --task, --list, or --view is required".to_string(),
                    status: 0,
                });
            };
            // The base may already carry the custom-task-id query pair.
            let boundary_sep = if base.contains('?') { '&' } else { '?' };
            let comments = crate::commands::pagination::walk_start_id(
                cli,
                &client,
                start,
                start_id,
                "comments",
                |start, start_id| match (start, start_id) {
                    (Some(s), Some(sid)) => {
                        format!("{}{}start={}&start_id={}", base, boundary_sep, s, sid)
                    }
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
            markdown,
        } => {
            let ws = crate::commands::workspace::resolve_workspace(cli).ok();
            let resp = if let Some(id) = list {
                let body = comment_body_enriched(&client, ws.as_deref(), markdown, &text).await;
                client
                    .post(&format!("/v2/list/{}/comment", id), &body)
                    .await?
            } else if let Some(id) = view {
                let body = comment_body_enriched(&client, ws.as_deref(), markdown, &text).await;
                client
                    .post(&format!("/v2/view/{}/comment", id), &body)
                    .await?
            } else if let Some(resolved) = git::resolve_task(cli, task.as_deref(), true)? {
                let mut body = comment_body_enriched(&client, ws.as_deref(), markdown, &text).await;
                body["notify_all"] = serde_json::json!(notify_all);
                if let Some(a) = assignee {
                    body["assignee"] = serde_json::json!(a);
                }
                let q = crate::commands::workspace::custom_task_query(cli, &resolved)?;
                client
                    .post(&format!("/v2/task/{}/comment{}", resolved.id, q), &body)
                    .await?
            } else {
                return Err(CliError::ClientError {
                    message: "One of --task, --list, or --view is required".to_string(),
                    status: 0,
                });
            };
            output.print_single(&resp, COMMENT_FIELDS, "id");
            Ok(())
        }
        CommentCommands::Update {
            id,
            text,
            resolved,
            assignee,
            markdown,
        } => {
            // Same @mention resolution as create/reply. The body is replaced
            // whole, so mentions that should stay must be restated.
            let ws = crate::commands::workspace::resolve_workspace(cli).ok();
            let mut body = comment_body_enriched(&client, ws.as_deref(), markdown, &text).await;
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
        CommentCommands::Replies {
            id,
            start,
            start_id,
        } => {
            let comments = crate::commands::pagination::walk_start_id(
                cli,
                &client,
                start,
                start_id,
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
        CommentCommands::Reply {
            id,
            text,
            assignee,
            markdown,
        } => {
            let ws = crate::commands::workspace::resolve_workspace(cli).ok();
            let mut body = comment_body_enriched(&client, ws.as_deref(), markdown, &text).await;
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
