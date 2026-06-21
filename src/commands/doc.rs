use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::git;
use crate::output::OutputConfig;
use crate::Cli;
use clap::Subcommand;

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
        /// Parent type: SPACE, FOLDER, LIST, EVERYTHING, or WORKSPACE
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
        /// Page content (use @path to read from a file, @- for stdin, @@ for a literal leading @)
        #[arg(long, value_parser = crate::input::resolve_value_arg)]
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
        /// Page content (use @path to read from a file, @- for stdin, @@ for a literal leading @)
        #[arg(long, value_parser = crate::input::resolve_value_arg)]
        content: String,
        /// Content edit mode: replace, append, or prepend
        #[arg(long, default_value = "replace")]
        mode: String,
    },
    /// Upload an image and embed it inline in a doc page.
    ///
    /// The ClickUp API has no doc-level upload, so the image is stored as an
    /// attachment on a host task, then referenced from the page as markdown.
    #[command(name = "embed-image")]
    EmbedImage {
        /// Doc ID
        doc_id: String,
        /// Page ID
        page_id: String,
        /// Path to the image file to upload
        file: std::path::PathBuf,
        /// Host task that stores the image binary (auto-detected from git branch if omitted)
        #[arg(long)]
        via_task: Option<String>,
        /// Alt text for the image (defaults to the file name)
        #[arg(long)]
        alt: Option<String>,
        /// Where to insert the image relative to existing content
        #[arg(long, default_value = "append", value_parser = ["append", "prepend"])]
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
            let docs = crate::commands::pagination::walk_cursor(
                cli,
                &client,
                &["data", "docs"],
                |cursor| {
                    let mut params: Vec<String> = Vec::new();
                    if let Some(c) = &creator {
                        params.push(format!("creator={}", c));
                    }
                    if archived {
                        params.push("archived=true".to_string());
                    }
                    if let Some(c) = cursor {
                        params.push(format!("cursor={}", c));
                    }
                    if params.is_empty() {
                        base.clone()
                    } else {
                        format!("{}?{}", base, params.join("&"))
                    }
                },
            )
            .await?;
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
                    let type_id = match pt.to_uppercase().as_str() {
                        "SPACE" | "4" => 4,
                        "FOLDER" | "5" => 5,
                        "LIST" | "6" => 6,
                        "EVERYTHING" | "7" => 7,
                        "WORKSPACE" | "12" => 12,
                        other => {
                            return Err(CliError::ClientError {
                                message: format!(
                                    "Invalid --parent-type '{}'. Valid values: SPACE, FOLDER, LIST, EVERYTHING, WORKSPACE",
                                    other
                                ),
                                status: 0,
                            });
                        }
                    };
                    parent.insert("type".into(), serde_json::json!(type_id));
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
                let resp = client.get(&format!("{}/{}/page_listing", base, id)).await?;
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
        DocCommands::EmbedImage {
            doc_id,
            page_id,
            file,
            via_task,
            alt,
            mode,
        } => {
            let task = git::require_task(cli, via_task.as_deref(), true).map_err(|_| {
                CliError::BranchDetect {
                    message:
                        "ClickUp has no doc-level upload; the image must be attached to a host \
                         task."
                            .into(),
                    hint: "Pass --via-task TASK_ID, set CLICKUP_TASK_ID, or run from a branch \
                           containing a task ID (e.g. feat/CU-abc123-...)."
                        .into(),
                }
            })?;
            let upload_path = if task.is_custom {
                format!(
                    "/v2/task/{}/attachment?custom_task_ids=true&team_id={}",
                    task.id, ws_id
                )
            } else {
                format!("/v2/task/{}/attachment", task.id)
            };
            let uploaded = client.upload_file(&upload_path, &file).await?;
            let url = uploaded
                .get("url")
                .and_then(|u| u.as_str())
                .ok_or_else(|| CliError::ServerError {
                    message: "Upload succeeded but the response contained no attachment URL".into(),
                })?
                .to_string();
            let alt = alt.unwrap_or_else(|| {
                file.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            });
            let body = serde_json::json!({
                "content": embed_snippet(&alt, &url),
                "content_edit_mode": mode,
            });
            let edit = client
                .put(&format!("{}/{}/pages/{}", base, doc_id, page_id), &body)
                .await;
            if let Err(e) = edit {
                // The binary is already on the CDN; tell the caller how to
                // finish the embed without re-uploading.
                eprintln!(
                    "Image uploaded to {} but embedding it in page {} failed.\n\
                     Retry without re-uploading: clickup-cli doc edit-page {} {} \
                     --content \"![{}]({})\" --mode {} \
                     (keep the image markdown on its own line so ClickUp converts it)",
                    url, page_id, doc_id, page_id, alt, url, mode
                );
                return Err(e);
            }
            let result = serde_json::json!({
                "url": url,
                "page_id": page_id,
                "mode": mode,
            });
            output.print_single(&result, &["url", "page_id", "mode"], "url");
            Ok(())
        }
    }
}

/// Markdown snippet ClickUp converts into a native inline image block.
/// Surrounding newlines keep the image out of adjacent paragraphs.
pub(crate) fn embed_snippet(alt: &str, url: &str) -> String {
    format!("\n![{}]({})\n", alt, url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippet_wraps_image_in_newlines() {
        assert_eq!(
            embed_snippet("chart", "https://example.com/i.png"),
            "\n![chart](https://example.com/i.png)\n"
        );
    }

    #[test]
    fn snippet_allows_empty_alt() {
        assert_eq!(
            embed_snippet("", "https://x.test/a.png"),
            "\n![](https://x.test/a.png)\n"
        );
    }
}
