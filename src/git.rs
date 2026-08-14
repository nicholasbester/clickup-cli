//! Git branch-based task ID detection.
//!
//! Resolves task IDs from three sources in priority order: explicit CLI arg, the
//! `CLICKUP_TASK_ID` env var, then the current git branch name. Destructive
//! commands (`task delete`, `task link`, etc.) opt out of branch resolution.

use crate::config::Config;
use crate::error::CliError;
use crate::Cli;
use regex::Regex;
use std::process::Command;
use std::sync::OnceLock;

/// A task ID resolved from some source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTask {
    /// The ID to send on the wire. For CU-prefixed matches, this is the
    /// stripped form (`CU-abc123` → `abc123`). For custom IDs, the whole
    /// `PROJ-42` is preserved.
    pub id: String,
    /// The raw match as it appeared in the source (branch name or CLI arg),
    /// used for user-facing messages.
    pub raw: String,
    /// True when the ID matches the custom-ID shape (`PREFIX-NUMBER`) and
    /// requires `custom_task_ids=true&team_id=<ws>` on the request.
    pub is_custom: bool,
    /// Where the ID came from.
    pub source: TaskSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskSource {
    Explicit,
    Env,
    /// Resolved from a git branch; carries the branch name for breadcrumb output.
    Branch(String),
}

/// Conventional-commits prefixes stripped from branch names before regex match.
/// Case-insensitive, trailing `/` required. Go's 10 + our 4.
const STRIPPED_PREFIXES: &[&str] = &[
    "feature/",
    "feat/",
    "fix/",
    "hotfix/",
    "bugfix/",
    "release/",
    "chore/",
    "docs/",
    "refactor/",
    "test/",
    "ci/",
    "perf/",
    "build/",
    "style/",
];

/// Prefixes that look like custom task IDs but are actually workflow keywords.
/// Any `PREFIX-NUMBER` match whose prefix (uppercased) hits this list is
/// rejected. Go's 9 + our 9.
const EXCLUDED_CUSTOM_PREFIXES: &[&str] = &[
    "FEATURE", "FEAT", "BUGFIX", "BUG", "FIX", "HOTFIX", "RELEASE", "CHORE", "DOCS", "DOC",
    "REFACTOR", "TEST", "CI", "PERF", "BUILD", "STYLE", "WIP", "TMP",
];

fn cu_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\bCU-([0-9a-z]+)").unwrap())
}

fn custom_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b([A-Z][A-Z0-9]+-\d+)\b").unwrap())
}

/// Return the current git branch name, or `None` if not inside a repo, on
/// detached HEAD, or if `git` is unavailable. Never errors — git absence is
/// treated the same as "not in a repo".
pub fn current_branch() -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let name = String::from_utf8(out.stdout).ok()?.trim().to_string();
    if name.is_empty() || name == "HEAD" {
        return None;
    }
    Some(name)
}

/// Strip one known conventional-commits prefix (case-insensitive) from the
/// start of a branch name. Returns the remainder if a prefix matched, else the
/// original input.
fn strip_prefix(branch: &str) -> &str {
    let lower = branch.to_ascii_lowercase();
    for p in STRIPPED_PREFIXES {
        if lower.starts_with(p) {
            return &branch[p.len()..];
        }
    }
    branch
}

/// Extract a task ID from a branch name, or `None` if no pattern matches.
/// CU- matches take precedence over custom IDs; first match wins within each.
pub fn extract_task_id(branch: &str) -> Option<ResolvedTask> {
    let stripped = strip_prefix(branch);

    if let Some(m) = cu_regex().captures(stripped) {
        let raw = m.get(0).unwrap().as_str().to_string();
        let id = m.get(1).unwrap().as_str().to_string();
        return Some(ResolvedTask {
            id,
            raw,
            is_custom: false,
            source: TaskSource::Branch(branch.to_string()),
        });
    }

    for m in custom_regex().captures_iter(stripped) {
        let matched = m.get(1).unwrap().as_str();
        let prefix = matched.split('-').next().unwrap_or("");
        if EXCLUDED_CUSTOM_PREFIXES.contains(&prefix) {
            continue;
        }
        return Some(ResolvedTask {
            id: matched.to_string(),
            raw: matched.to_string(),
            is_custom: true,
            source: TaskSource::Branch(branch.to_string()),
        });
    }

    None
}

/// Normalize an explicit CLI arg. Strips `CU-` transparently; detects
/// `PREFIX-NUMBER` as custom. Never fails — any string becomes a `ResolvedTask`
/// with source `Explicit`.
pub fn parse_task_id(arg: &str) -> ResolvedTask {
    let arg = arg.trim();

    if let Some(m) = cu_regex().captures(arg) {
        // Only strip if the whole arg is a CU- ID (not a branch-like string).
        if m.get(0).unwrap().as_str().len() == arg.len() {
            let id = m.get(1).unwrap().as_str().to_string();
            return ResolvedTask {
                id,
                raw: arg.to_string(),
                is_custom: false,
                source: TaskSource::Explicit,
            };
        }
    }

    if let Some(m) = custom_regex().captures(arg) {
        let matched = m.get(1).unwrap().as_str();
        let prefix = matched.split('-').next().unwrap_or("");
        if matched.len() == arg.len() && !EXCLUDED_CUSTOM_PREFIXES.contains(&prefix) {
            return ResolvedTask {
                id: arg.to_string(),
                raw: arg.to_string(),
                is_custom: true,
                source: TaskSource::Explicit,
            };
        }
    }

    // Plain ID — pass through.
    ResolvedTask {
        id: arg.to_string(),
        raw: arg.to_string(),
        is_custom: false,
        source: TaskSource::Explicit,
    }
}

/// Error message for operations whose endpoints have no documented
/// `custom_task_ids` support (issue #104): sending a custom-format ID raw
/// is a guaranteed, confusing upstream failure, so callers reject it
/// locally with this guidance instead.
pub fn custom_id_unsupported(raw: &str, operation: &str) -> String {
    format!(
        "'{raw}' is a custom task ID, but {operation} requires the regular task id \
         (its endpoint has no custom-ID support) — find it with: task get {raw}"
    )
}

/// Is branch detection enabled? Checks `CLICKUP_GIT_DETECT=0` env override,
/// then `[git] enabled` in config. Defaults to true.
fn detect_enabled() -> bool {
    if let Ok(v) = std::env::var("CLICKUP_GIT_DETECT") {
        if v == "0" || v.eq_ignore_ascii_case("false") {
            return false;
        }
    }
    let cfg = Config::load().unwrap_or_default();
    cfg.git.enabled.unwrap_or(true)
}

fn verbose_enabled() -> bool {
    let cfg = Config::load().unwrap_or_default();
    cfg.git.verbose.unwrap_or(true)
}

/// Emit the "resolved task X from branch Y" breadcrumb to stderr, unless
/// suppressed by `-q`, a non-table output mode, or `[git] verbose = false`.
fn maybe_print_breadcrumb(cli: &Cli, task: &ResolvedTask) {
    if cli.quiet || cli.output != "table" {
        return;
    }
    if !verbose_enabled() {
        return;
    }
    if let TaskSource::Branch(branch) = &task.source {
        eprintln!("resolved task {} from branch {}", task.raw, branch);
    }
}

/// Resolve a task ID from the priority chain: explicit → env → branch. Returns
/// `None` if nothing is found — callers decide whether that's an error.
///
/// `allow_branch` should be `false` for destructive or ambiguous commands
/// (`task delete`, `task link`, `task unlink`, `guest share-task`,
/// `guest unshare-task`).
pub fn resolve_task(
    cli: &Cli,
    explicit: Option<&str>,
    allow_branch: bool,
) -> Result<Option<ResolvedTask>, CliError> {
    if let Some(arg) = explicit {
        let t = parse_task_id(arg);
        return Ok(Some(t));
    }

    if let Ok(v) = std::env::var("CLICKUP_TASK_ID") {
        if !v.is_empty() {
            let mut t = parse_task_id(&v);
            t.source = TaskSource::Env;
            return Ok(Some(t));
        }
    }

    if !allow_branch || !detect_enabled() {
        return Ok(None);
    }

    let branch = match current_branch() {
        Some(b) => b,
        None => return Ok(None),
    };
    let resolved = extract_task_id(&branch);
    if let Some(t) = &resolved {
        maybe_print_breadcrumb(cli, t);
    }
    Ok(resolved)
}

/// Like `resolve_task`, but errors with a helpful hint when nothing resolves.
pub fn require_task(
    cli: &Cli,
    explicit: Option<&str>,
    allow_branch: bool,
) -> Result<ResolvedTask, CliError> {
    match resolve_task(cli, explicit, allow_branch)? {
        Some(t) => Ok(t),
        None => Err(no_task_id_error(allow_branch)),
    }
}

fn no_task_id_error(allow_branch: bool) -> CliError {
    if !allow_branch {
        return CliError::BranchDetect {
            message: "No task ID provided. This command does not auto-detect from branch.".into(),
            hint: "Pass the task ID explicitly.".into(),
        };
    }
    match current_branch() {
        Some(b) => CliError::BranchDetect {
            message: format!(
                "No task ID on the command line and none detected in branch \"{}\".",
                b
            ),
            hint: "Name your branch like feat/CU-abc123-... or PROJ-42-..., or pass the ID \
                   explicitly."
                .into(),
        },
        None => CliError::BranchDetect {
            message: "No task ID provided and not inside a git repository.".into(),
            hint: "Pass the task ID explicitly, or run from a repo whose branch contains a \
                   task ID (e.g. feat/CU-abc123-...)."
                .into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract(b: &str) -> Option<(String, bool)> {
        extract_task_id(b).map(|t| (t.id, t.is_custom))
    }

    #[test]
    fn cu_plain_branch() {
        assert_eq!(extract("CU-abc123-foo"), Some(("abc123".into(), false)));
    }

    #[test]
    fn cu_with_feat_prefix() {
        assert_eq!(
            extract("feat/CU-abc123-foo"),
            Some(("abc123".into(), false))
        );
    }

    #[test]
    fn cu_lowercase() {
        assert_eq!(extract("cu-dead01-test"), Some(("dead01".into(), false)));
    }

    #[test]
    fn cu_mixed_case_prefix() {
        assert_eq!(extract("Feature/Cu-Abc123"), Some(("Abc123".into(), false)));
    }

    #[test]
    fn cu_with_underscore_after_id() {
        // Matches Go reference test: underscore is branch separator, not part of ID.
        assert_eq!(
            extract("CU-86d1u2bz4_React-Native-Pois-gone"),
            Some(("86d1u2bz4".into(), false))
        );
    }

    #[test]
    fn cu_with_feature_prefix_and_underscore() {
        assert_eq!(
            extract("feature/CU-86d1u2bz4_something"),
            Some(("86d1u2bz4".into(), false))
        );
    }

    #[test]
    fn custom_id_plain() {
        assert_eq!(extract("PROJ-42-add-login"), Some(("PROJ-42".into(), true)));
    }

    #[test]
    fn custom_id_with_fix_prefix() {
        assert_eq!(
            extract("fix/ENG-1234-auth"),
            Some(("ENG-1234".into(), true))
        );
    }

    #[test]
    fn excluded_prefix_feature() {
        assert_eq!(extract("FEATURE-123-something"), None);
    }

    #[test]
    fn excluded_prefix_bugfix() {
        assert_eq!(extract("BUGFIX-456-foo"), None);
    }

    #[test]
    fn excluded_prefix_wip() {
        assert_eq!(extract("WIP-1-in-progress"), None);
    }

    #[test]
    fn no_match_main() {
        assert_eq!(extract("main"), None);
    }

    #[test]
    fn no_match_draft_work() {
        assert_eq!(extract("draft-work"), None);
    }

    #[test]
    fn no_match_head_literal() {
        // current_branch() filters HEAD out, but extract_task_id still sees input.
        assert_eq!(extract("HEAD"), None);
    }

    #[test]
    fn cu_first_match_wins() {
        assert_eq!(extract("CU-aaa-refs-CU-bbb"), Some(("aaa".into(), false)));
    }

    #[test]
    fn cu_wins_over_custom() {
        assert_eq!(
            extract("feat/CU-abc123-refs-PROJ-42-foo"),
            Some(("abc123".into(), false))
        );
    }

    #[test]
    fn does_not_match_mid_word() {
        // Anchored with \b, so xyzCU-abc is rejected.
        assert_eq!(extract("xyzCU-abc"), None);
    }

    #[test]
    fn empty_branch() {
        assert_eq!(extract(""), None);
    }

    #[test]
    fn parse_explicit_cu_stripped() {
        let t = parse_task_id("CU-abc123");
        assert_eq!(t.id, "abc123");
        assert!(!t.is_custom);
        assert_eq!(t.source, TaskSource::Explicit);
    }

    #[test]
    fn parse_explicit_custom_flagged() {
        let t = parse_task_id("PROJ-42");
        assert_eq!(t.id, "PROJ-42");
        assert!(t.is_custom);
    }

    #[test]
    fn parse_explicit_plain() {
        let t = parse_task_id("abc123");
        assert_eq!(t.id, "abc123");
        assert!(!t.is_custom);
    }

    #[test]
    fn parse_explicit_excluded_prefix_not_custom() {
        // FEATURE-123 as an explicit arg is not treated as custom — passes through.
        let t = parse_task_id("FEATURE-123");
        assert_eq!(t.id, "FEATURE-123");
        assert!(!t.is_custom);
    }

    #[test]
    fn parse_explicit_trims_whitespace() {
        let t = parse_task_id("  CU-abc123 ");
        assert_eq!(t.id, "abc123");
    }
}
