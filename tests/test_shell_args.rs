//! Shell-boundary tests for the `@file` value convention (GH #70).
//!
//! The other tests in this crate spawn the binary directly via `assert_cmd`,
//! which never exercises a shell's argument-passing rules — but that boundary
//! is the entire failure mode #70 addresses: Windows PowerShell splits an
//! unquoted multiline value into several argv tokens, so `--description $desc`
//! reached clap as multiple args and errored. The `@file` workaround sidesteps
//! that because `@path` is a single whitespace-free token that survives any
//! shell.
//!
//! These tests invoke the *built binary through a real shell* against an
//! in-process wiremock server and assert the multiline file content arrives
//! intact. The Rust test owns the mock; only the invocation goes through the
//! shell. The PowerShell test runs for real on the `windows-latest` CI runner
//! (and self-skips where no PowerShell is installed); the POSIX-`sh` test runs
//! everywhere else and proves the same harness end-to-end.

use std::path::Path;
use std::process::Command;
use wiremock::matchers::{body_json, method, path as path_matcher};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Multiline content that an unquoted PowerShell variable would split at the
/// process boundary. After the file read, our helper strips one trailing
/// newline, so the body the API receives is the two lines joined by `\n`.
const FILE_CONTENT: &str = "First line\nsecond line\n";
const EXPECTED_BODY_TEXT: &str = "First line\nsecond line";

/// Start a mock that only returns 200 when `task create` posts the exact
/// multiline body — so a successful exit proves the `@file` value was read and
/// transmitted whole.
async fn mount_create_mock(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path_matcher("/v2/list/list-1/task"))
        .and(body_json(serde_json::json!({
            "name": "demo",
            "markdown_content": EXPECTED_BODY_TEXT,
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "t1",
            "name": "demo"
        })))
        .expect(1)
        .mount(server)
        .await;
}

fn binary() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("clickup-cli")
}

fn with_env(cmd: &mut Command, server: &MockServer) {
    cmd.env("CLICKUP_API_URL", server.uri())
        .env("CLICKUP_TOKEN", "pk_test")
        .env("CLICKUP_WORKSPACE", "99")
        .env_remove("CLICKUP_GIT_DETECT")
        .env_remove("CLICKUP_TASK_ID");
}

fn write_desc_file(dir: &Path) -> std::path::PathBuf {
    let p = dir.join("desc.md");
    std::fs::write(&p, FILE_CONTENT).unwrap();
    p
}

/// Locate an available PowerShell. Prefer Windows PowerShell 5.1
/// (`powershell.exe` — the exact environment from the bug report) over
/// PowerShell Core (`pwsh`); fall back to `pwsh` on non-Windows.
fn find_powershell() -> Option<&'static str> {
    for cand in ["powershell", "pwsh"] {
        let ok = Command::new(cand)
            .args(["-NoProfile", "-Command", "exit 0"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if ok {
            return Some(cand);
        }
    }
    None
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn at_file_survives_posix_shell_boundary() {
    let dir = tempfile::TempDir::new().unwrap();
    let server = MockServer::start().await;
    mount_create_mock(&server).await;
    let desc = write_desc_file(dir.path());

    // `sh -c '<script>' sh <bin> <descfile>` → $0=sh, $1=bin, $2=descfile.
    // Passing paths as positional params avoids interpolating them into the
    // script string. `"@$2"` becomes a single `@<path>` argument.
    let mut cmd = Command::new("sh");
    with_env(&mut cmd, &server);
    cmd.arg("-c")
        .arg(r#""$1" task create --list list-1 --name demo --description "@$2""#)
        .arg("sh")
        .arg(binary())
        .arg(&desc);

    let out = cmd.output().expect("failed to run sh");
    assert!(
        out.status.success(),
        "sh invocation failed: status={:?}\nstdout={}\nstderr={}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn at_file_survives_powershell_boundary() {
    let Some(ps) = find_powershell() else {
        eprintln!("skipping: no PowerShell (powershell/pwsh) found on PATH");
        return;
    };

    let dir = tempfile::TempDir::new().unwrap();
    let server = MockServer::start().await;
    mount_create_mock(&server).await;
    let desc = write_desc_file(dir.path());

    // A script file receives the binary + file paths as named parameters, so
    // no path interpolation/quoting into a command string is needed. The
    // `"@$DescFile"` mirrors exactly how a user would pass the workaround.
    let script = dir.path().join("invoke.ps1");
    std::fs::write(
        &script,
        "param([string]$Bin,[string]$DescFile)\n\
         & $Bin task create --list list-1 --name demo --description \"@$DescFile\"\n\
         exit $LASTEXITCODE\n",
    )
    .unwrap();

    let mut cmd = Command::new(ps);
    with_env(&mut cmd, &server);
    cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script)
        .arg("-Bin")
        .arg(binary())
        .arg("-DescFile")
        .arg(&desc);

    let out = cmd.output().expect("failed to run PowerShell");
    assert!(
        out.status.success(),
        "PowerShell ({ps}) invocation failed: status={:?}\nstdout={}\nstderr={}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}
