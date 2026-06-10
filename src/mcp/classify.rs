//! Classification of MCP tool names into (Class, group).
//!
//! `classify()` is a pure function. The self-check test in
//! `tests/test_mcp_filter.rs` asserts that every tool in `tool_list()`
//! classifies without falling through to `None`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Read,
    Write,
    Destructive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolMeta {
    pub class: Class,
    pub group: &'static str,
}

/// Known resource groups. Two-word groups come first so prefix matching
/// in `classify()` prefers them over their one-word prefixes.
const KNOWN_GROUPS: &[(&str, &str)] = &[
    ("task_type", "task-type"),
    ("audit_log", "audit-log"),
    ("auth", "auth"),
    ("workspace", "workspace"),
    ("space", "space"),
    ("folder", "folder"),
    ("list", "list"),
    ("task", "task"),
    ("checklist", "checklist"),
    ("comment", "comment"),
    ("tag", "tag"),
    ("field", "field"),
    ("attachment", "attachment"),
    ("time", "time"),
    ("goal", "goal"),
    ("view", "view"),
    ("member", "member"),
    ("user", "user"),
    ("chat", "chat"),
    ("doc", "doc"),
    ("webhook", "webhook"),
    ("template", "template"),
    ("guest", "guest"),
    ("group", "group"),
    ("role", "role"),
    ("shared", "shared"),
    ("acl", "acl"),
];

const READ_VERBS: &[&str] = &[
    "list",
    "get",
    "search",
    "current",
    "pages",
    "followers",
    "members",
    "history",
    "whoami",
    "check",
    "replies",
    "tagged",
    "query",
];

const WRITE_VERBS: &[&str] = &[
    "create", "update", "set", "add", "start", "stop", "move", "apply", "invite", "rename",
    "share", "attach", "link", "reply", "send", "dm", "edit", "upload",
];

const DESTRUCTIVE_VERBS: &[&str] = &["delete", "remove", "unshare", "unlink", "unset"];

/// Tools that don't fit the naming convention. Each entry shortcircuits
/// the auto-deriver.
const OVERRIDES: &[(&str, Class, &str)] = &[
    ("clickup_search", Class::Read, "workspace"),
    ("clickup_whoami", Class::Read, "auth"),
    ("clickup_workspace_plan", Class::Read, "workspace"),
    ("clickup_workspace_seats", Class::Read, "workspace"),
    ("clickup_task_replace_estimates", Class::Write, "task"),
    ("clickup_task_time_in_status", Class::Read, "task"),
    ("clickup_time_tags", Class::Read, "time"),
    ("clickup_template_apply_list", Class::Write, "template"),
    ("clickup_doc_get_page", Class::Read, "doc"),
    ("clickup_doc_embed_image", Class::Write, "doc"),
    ("clickup_chat_tagged_users", Class::Read, "chat"),
    ("clickup_view_tasks", Class::Read, "view"),
    ("clickup_guest_share_list", Class::Write, "guest"),
];

pub fn classify(tool_name: &str) -> Option<ToolMeta> {
    // Step 1: override table
    if let Some(&(_, class, group)) = OVERRIDES.iter().find(|(n, _, _)| *n == tool_name) {
        return Some(ToolMeta { class, group });
    }

    // Step 2: group prefix (longest match wins because two-word entries come first)
    let rest = tool_name.strip_prefix("clickup_")?;
    let (raw_prefix, normalized_group) = KNOWN_GROUPS
        .iter()
        .find(|(prefix, _)| rest == *prefix || rest.starts_with(&format!("{}_", prefix)))
        .copied()?;
    let remainder = rest
        .strip_prefix(raw_prefix)
        .and_then(|r| r.strip_prefix('_'))
        .unwrap_or("");

    if remainder.is_empty() {
        return None;
    }

    let segments: Vec<&str> = remainder.split('_').collect();
    let last = *segments.last().unwrap();

    // Step 4: destructive anywhere
    if segments.iter().any(|s| DESTRUCTIVE_VERBS.contains(s)) {
        return Some(ToolMeta {
            class: Class::Destructive,
            group: normalized_group,
        });
    }

    // Step 5: trailing verb
    if WRITE_VERBS.contains(&last) {
        return Some(ToolMeta {
            class: Class::Write,
            group: normalized_group,
        });
    }
    if READ_VERBS.contains(&last) {
        return Some(ToolMeta {
            class: Class::Read,
            group: normalized_group,
        });
    }

    // Step 6: any write segment
    if segments.iter().any(|s| WRITE_VERBS.contains(s)) {
        return Some(ToolMeta {
            class: Class::Write,
            group: normalized_group,
        });
    }

    None
}

pub const ALL_GROUPS: &[&str] = &[
    "auth",
    "workspace",
    "space",
    "folder",
    "list",
    "task",
    "checklist",
    "comment",
    "tag",
    "field",
    "task-type",
    "attachment",
    "time",
    "goal",
    "view",
    "member",
    "user",
    "chat",
    "doc",
    "webhook",
    "template",
    "guest",
    "group",
    "role",
    "shared",
    "audit-log",
    "acl",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destructive_wins_over_write_in_same_name() {
        assert_eq!(
            classify("clickup_task_remove_tag").unwrap().class,
            Class::Destructive
        );
    }

    #[test]
    fn trailing_read_beats_earlier_write() {
        // reply (write verb) appears before list (read verb); trailing wins
        assert_eq!(
            classify("clickup_chat_reply_list").unwrap().class,
            Class::Read
        );
    }

    #[test]
    fn write_scan_catches_compound_verbs() {
        assert_eq!(classify("clickup_goal_add_kr").unwrap().class, Class::Write);
        assert_eq!(
            classify("clickup_task_add_dep").unwrap().class,
            Class::Write
        );
    }

    #[test]
    fn two_word_group_prefix_wins() {
        let m = classify("clickup_task_type_list").unwrap();
        assert_eq!(m.group, "task-type");
        assert_eq!(m.class, Class::Read);
    }

    #[test]
    fn override_table_short_circuits() {
        assert_eq!(
            classify("clickup_task_replace_estimates").unwrap().class,
            Class::Write
        );
        assert_eq!(classify("clickup_search").unwrap().group, "workspace");
    }

    #[test]
    fn unknown_tool_returns_none() {
        assert!(classify("clickup_not_a_real_tool").is_none());
    }
}
