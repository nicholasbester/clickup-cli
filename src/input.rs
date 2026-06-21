//! Helpers for resolving CLI string arguments that may reference a file.
//!
//! Shells differ in how they pass multiline / specially-quoted strings to a
//! native executable. On Windows PowerShell in particular, an unquoted
//! multiline value (e.g. a here-string stored in a variable) is split into
//! several whitespace-separated argv tokens at the process boundary, so a
//! flag like `--description $desc` receives only the first line and clap
//! errors on the remaining words as unexpected arguments (GH #70).
//!
//! To make multiline and large values portable across every shell, every
//! free-form text flag (e.g. `--description`, `--text`, `--content`) is wired
//! to [`resolve_value_arg`] as a clap `value_parser`, so resolution happens
//! uniformly at parse time and new text flags inherit it by attaching the same
//! parser. A leading `@` is interpreted as:
//!
//! - `@path`  → read the value from the file at `path`
//! - `@-`     → read the value from stdin
//! - `@@text` → escape: send the literal string `@text` (no file lookup)
//! - anything else → used verbatim
//!
//! A single trailing newline is stripped from file/stdin content (the common
//! editor / `echo` artifact); embedded newlines are preserved.
//!
//! Note: because a bare leading `@` is now significant, a value that should be
//! sent literally and starts with `@` (e.g. an `@mention`) must be escaped as
//! `@@…`. A missing-file error names the `@@` form so the fix is discoverable.

use crate::error::CliError;
use std::io::Read;

/// Resolve a possibly-`@`-prefixed argument into its literal string value.
///
/// See the module docs for the `@` conventions. Returns an error only when a
/// referenced file (or stdin) cannot be read.
pub fn resolve_value_arg(value: &str) -> Result<String, CliError> {
    let Some(rest) = value.strip_prefix('@') else {
        // No leading '@' — use the value exactly as given.
        return Ok(value.to_string());
    };

    // `@@text` escapes a literal leading '@': drop one '@', return the rest.
    if let Some(literal) = rest.strip_prefix('@') {
        return Ok(format!("@{literal}"));
    }

    let content = if rest == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| CliError::ConfigError(format!("failed to read value from stdin: {e}")))?;
        buf
    } else {
        std::fs::read_to_string(rest).map_err(|e| {
            CliError::ConfigError(format!(
                "failed to read value from file '{rest}': {e}. \
                 If you meant the literal text '@{rest}', escape the leading '@' as '@@{rest}'."
            ))
        })?
    };

    Ok(strip_single_trailing_newline(content))
}

fn strip_single_trailing_newline(mut s: String) -> String {
    if s.ends_with('\n') {
        s.pop();
        if s.ends_with('\r') {
            s.pop();
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn plain_value_passes_through_verbatim() {
        assert_eq!(resolve_value_arg("hello world").unwrap(), "hello world");
        assert_eq!(resolve_value_arg("").unwrap(), "");
    }

    #[test]
    fn double_at_escapes_literal_at() {
        assert_eq!(resolve_value_arg("@@everyone").unwrap(), "@everyone");
        // Only one leading '@' is consumed by the escape.
        assert_eq!(resolve_value_arg("@@@x").unwrap(), "@@x");
    }

    #[test]
    fn at_path_reads_file_and_strips_one_trailing_newline() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        write!(f, "line one\nline two\n").unwrap();
        let arg = format!("@{}", f.path().display());
        assert_eq!(resolve_value_arg(&arg).unwrap(), "line one\nline two");
    }

    #[test]
    fn at_path_preserves_interior_newlines_and_no_trailing() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        write!(f, "a\n\nb").unwrap();
        let arg = format!("@{}", f.path().display());
        assert_eq!(resolve_value_arg(&arg).unwrap(), "a\n\nb");
    }

    #[test]
    fn at_missing_file_errors() {
        let err = resolve_value_arg("@/no/such/file/here.txt").unwrap_err();
        assert!(matches!(err, CliError::ConfigError(_)));
    }
}
