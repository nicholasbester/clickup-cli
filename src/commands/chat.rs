use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum ChatCommands {
    /// List channels in the workspace
    #[command(name = "channel-list")]
    ChannelList {
        /// Include closed channels
        #[arg(long)]
        include_closed: bool,
    },
    /// Create a channel
    #[command(name = "channel-create")]
    ChannelCreate {
        /// Channel name
        #[arg(long)]
        name: String,
        /// Visibility: PUBLIC or PRIVATE
        #[arg(long)]
        visibility: Option<String>,
    },
    /// Get a channel by ID
    #[command(name = "channel-get")]
    ChannelGet {
        /// Channel ID
        id: String,
    },
    /// Update a channel
    #[command(name = "channel-update")]
    ChannelUpdate {
        /// Channel ID
        id: String,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// New topic
        #[arg(long)]
        topic: Option<String>,
    },
    /// Delete a channel
    #[command(name = "channel-delete")]
    ChannelDelete {
        /// Channel ID
        id: String,
    },
    /// List followers of a channel
    #[command(name = "channel-followers")]
    ChannelFollowers {
        /// Channel ID
        id: String,
    },
    /// List members of a channel
    #[command(name = "channel-members")]
    ChannelMembers {
        /// Channel ID
        id: String,
    },
    /// Create or get a direct message channel
    Dm {
        /// User ID(s) to send a DM to
        user_ids: Vec<String>,
    },
    /// List messages in a channel
    #[command(name = "message-list")]
    MessageList {
        /// Channel ID
        #[arg(long)]
        channel: String,
    },
    /// Send a message to a channel
    #[command(name = "message-send")]
    MessageSend {
        /// Channel ID
        #[arg(long)]
        channel: String,
        /// Message text (use @path to read from a file, @- for stdin, @@ for a literal leading @)
        #[arg(long, value_parser = crate::input::resolve_value_arg)]
        text: String,
        /// Message type: message or post
        #[arg(long, default_value = "message")]
        r#type: String,
    },
    /// Update a message
    #[command(name = "message-update")]
    MessageUpdate {
        /// Message ID
        id: String,
        /// New message text (use @path to read from a file, @- for stdin, @@ for a literal leading @)
        #[arg(long, value_parser = crate::input::resolve_value_arg)]
        text: String,
    },
    /// Delete a message
    #[command(name = "message-delete")]
    MessageDelete {
        /// Message ID
        id: String,
    },
    /// List reactions on a message
    #[command(name = "reaction-list")]
    ReactionList {
        /// Message ID
        msg_id: String,
    },
    /// Add a reaction to a message
    #[command(name = "reaction-add")]
    ReactionAdd {
        /// Message ID
        msg_id: String,
        /// Emoji name
        #[arg(long)]
        emoji: String,
    },
    /// Remove a reaction from a message
    #[command(name = "reaction-remove")]
    ReactionRemove {
        /// Message ID
        msg_id: String,
        /// Emoji name
        emoji: String,
    },
    /// List replies to a message
    #[command(name = "reply-list")]
    ReplyList {
        /// Message ID
        msg_id: String,
    },
    /// Send a reply to a message
    #[command(name = "reply-send")]
    ReplySend {
        /// Message ID
        msg_id: String,
        /// Reply text (use @path to read from a file, @- for stdin, @@ for a literal leading @)
        #[arg(long, value_parser = crate::input::resolve_value_arg)]
        text: String,
    },
    /// Get users tagged in a message
    #[command(name = "tagged-users")]
    TaggedUsers {
        /// Message ID
        msg_id: String,
    },
}

const CHANNEL_FIELDS: &[&str] = &["id", "name", "visibility", "type"];
const MESSAGE_FIELDS: &[&str] = &["id", "content", "type", "date"];

pub async fn execute(command: ChatCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let ws_id = resolve_workspace(cli)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);
    let base = format!("/v3/workspaces/{}/chat", ws_id);

    match command {
        ChatCommands::ChannelList { include_closed } => {
            let channels = crate::commands::pagination::walk_cursor(
                cli,
                &client,
                &["data", "channels"],
                |cursor| {
                    let mut qs: Vec<String> = Vec::new();
                    if include_closed {
                        qs.push("include_closed=true".to_string());
                    }
                    if let Some(c) = cursor {
                        qs.push(format!("cursor={}", c));
                    }
                    if qs.is_empty() {
                        format!("{}/channels", base)
                    } else {
                        format!("{}/channels?{}", base, qs.join("&"))
                    }
                },
            )
            .await?;
            output.print_items(&channels, CHANNEL_FIELDS, "id");
            Ok(())
        }
        ChatCommands::ChannelCreate { name, visibility } => {
            let mut body = serde_json::json!({ "name": name });
            if let Some(v) = visibility {
                body["visibility"] = serde_json::Value::String(v);
            }
            let resp = client.post(&format!("{}/channels", base), &body).await?;
            output.print_single(&resp, CHANNEL_FIELDS, "id");
            Ok(())
        }
        ChatCommands::ChannelGet { id } => {
            let resp = client.get(&format!("{}/channels/{}", base, id)).await?;
            output.print_single(&resp, CHANNEL_FIELDS, "id");
            Ok(())
        }
        ChatCommands::ChannelUpdate { id, name, topic } => {
            let mut body = serde_json::Map::new();
            if let Some(n) = name {
                body.insert("name".into(), serde_json::Value::String(n));
            }
            if let Some(t) = topic {
                body.insert("topic".into(), serde_json::Value::String(t));
            }
            let resp = client
                .patch(
                    &format!("{}/channels/{}", base, id),
                    &serde_json::Value::Object(body),
                )
                .await?;
            output.print_single(&resp, CHANNEL_FIELDS, "id");
            Ok(())
        }
        ChatCommands::ChannelDelete { id } => {
            client.delete(&format!("{}/channels/{}", base, id)).await?;
            output.print_message(&format!("Channel {} deleted", id));
            Ok(())
        }
        ChatCommands::ChannelFollowers { id } => {
            let followers =
                crate::commands::pagination::walk_cursor(cli, &client, &["data"], |cursor| {
                    match cursor {
                        Some(c) => format!("{}/channels/{}/followers?cursor={}", base, id, c),
                        None => format!("{}/channels/{}/followers", base, id),
                    }
                })
                .await?;
            output.print_items(&followers, &["id", "name", "username", "email"], "id");
            Ok(())
        }
        ChatCommands::ChannelMembers { id } => {
            let members =
                crate::commands::pagination::walk_cursor(cli, &client, &["data"], |cursor| {
                    match cursor {
                        Some(c) => format!("{}/channels/{}/members?cursor={}", base, id, c),
                        None => format!("{}/channels/{}/members", base, id),
                    }
                })
                .await?;
            output.print_items(&members, &["id", "name", "username", "email"], "id");
            Ok(())
        }
        ChatCommands::Dm { user_ids } => {
            let body = serde_json::json!({ "user_ids": user_ids });
            let resp = client
                .post(&format!("{}/channels/direct_message", base), &body)
                .await?;
            output.print_single(&resp, CHANNEL_FIELDS, "id");
            Ok(())
        }
        ChatCommands::MessageList { channel } => {
            let messages = crate::commands::pagination::walk_cursor(
                cli,
                &client,
                &["data", "messages"],
                |cursor| match cursor {
                    Some(c) => format!("{}/channels/{}/messages?cursor={}", base, channel, c),
                    None => format!("{}/channels/{}/messages", base, channel),
                },
            )
            .await?;
            output.print_items(&messages, MESSAGE_FIELDS, "id");
            Ok(())
        }
        ChatCommands::MessageSend {
            channel,
            text,
            r#type,
        } => {
            let body = serde_json::json!({ "content": text, "type": r#type });
            let resp = client
                .post(&format!("{}/channels/{}/messages", base, channel), &body)
                .await?;
            output.print_single(&resp, MESSAGE_FIELDS, "id");
            Ok(())
        }
        ChatCommands::MessageUpdate { id, text } => {
            let body = serde_json::json!({ "content": text });
            let resp = client
                .patch(&format!("{}/messages/{}", base, id), &body)
                .await?;
            output.print_single(&resp, MESSAGE_FIELDS, "id");
            Ok(())
        }
        ChatCommands::MessageDelete { id } => {
            client.delete(&format!("{}/messages/{}", base, id)).await?;
            output.print_message(&format!("Message {} deleted", id));
            Ok(())
        }
        ChatCommands::ReactionList { msg_id } => {
            let reactions =
                crate::commands::pagination::walk_cursor(cli, &client, &["data"], |cursor| {
                    match cursor {
                        Some(c) => format!("{}/messages/{}/reactions?cursor={}", base, msg_id, c),
                        None => format!("{}/messages/{}/reactions", base, msg_id),
                    }
                })
                .await?;
            output.print_items(&reactions, &["reaction", "user", "date"], "reaction");
            Ok(())
        }
        ChatCommands::ReactionAdd { msg_id, emoji } => {
            let body = serde_json::json!({ "reaction": emoji });
            let resp = client
                .post(&format!("{}/messages/{}/reactions", base, msg_id), &body)
                .await?;
            println!("{}", serde_json::to_string_pretty(&resp).unwrap());
            Ok(())
        }
        ChatCommands::ReactionRemove { msg_id, emoji } => {
            // Emoji like 👍 contain bytes outside the URL path's unreserved set;
            // percent-encode the segment so the request is well-formed.
            let encoded: String = emoji
                .bytes()
                .flat_map(|byte| match byte {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                        vec![byte as char]
                    }
                    _ => format!("%{:02X}", byte).chars().collect(),
                })
                .collect();
            client
                .delete(&format!(
                    "{}/messages/{}/reactions/{}",
                    base, msg_id, encoded
                ))
                .await?;
            output.print_message(&format!(
                "Reaction '{}' removed from message {}",
                emoji, msg_id
            ));
            Ok(())
        }
        ChatCommands::ReplyList { msg_id } => {
            let replies = crate::commands::pagination::walk_cursor(
                cli,
                &client,
                &["data", "replies"],
                |cursor| match cursor {
                    Some(c) => format!("{}/messages/{}/replies?cursor={}", base, msg_id, c),
                    None => format!("{}/messages/{}/replies", base, msg_id),
                },
            )
            .await?;
            output.print_items(&replies, MESSAGE_FIELDS, "id");
            Ok(())
        }
        ChatCommands::ReplySend { msg_id, text } => {
            let body = serde_json::json!({ "content": text });
            let resp = client
                .post(&format!("{}/messages/{}/replies", base, msg_id), &body)
                .await?;
            output.print_single(&resp, MESSAGE_FIELDS, "id");
            Ok(())
        }
        ChatCommands::TaggedUsers { msg_id } => {
            let users =
                crate::commands::pagination::walk_cursor(cli, &client, &["data"], |cursor| {
                    match cursor {
                        Some(c) => {
                            format!("{}/messages/{}/tagged_users?cursor={}", base, msg_id, c)
                        }
                        None => format!("{}/messages/{}/tagged_users", base, msg_id),
                    }
                })
                .await?;
            output.print_items(&users, &["id", "name", "username", "email"], "id");
            Ok(())
        }
    }
}
