# Contributing to clickup-cli

Thanks for taking the time to contribute. This is a mostly-solo project, so the process is deliberately lightweight.

## Before you start

- For bugs and feature requests, **open an issue first** so we can agree on scope before you spend time on a PR.
- For security issues, **don't** open a public issue — see [SECURITY.md](SECURITY.md) for the private disclosure channel.
- Small docs/typo fixes are fine to send as a direct PR without a prior issue.

## Development setup

You need Rust 1.70+ (`rustup` makes this easy).

```bash
git clone https://github.com/nicholasbester/clickup-cli.git
cd clickup-cli
cargo build
cargo test
cargo run -- --help
```

A local ClickUp token is useful for manual testing but not required for tests — the test suite uses `wiremock` to stub HTTP calls.

## Project layout

- `src/main.rs` → `src/lib.rs` — entry point
- `src/commands/{resource}.rs` — one file per API resource group
- `src/models/{resource}.rs` — serde structs for API responses
- `src/client.rs`, `src/config.rs`, `src/output.rs`, `src/error.rs` — core
- `src/mcp.rs` + `src/mcp/{classify,filter}.rs` — MCP server and tool filtering
- `tests/test_*.rs` — integration tests
- `docs/` — Jekyll site for clickup-cli.com
- `packaging/aur/` — AUR PKGBUILD (auto-updated on release)

## PR expectations

- **Keep changes focused.** One logical change per PR. Refactors go in separate PRs from feature work.
- **Tests required for behavior changes.** Existing patterns live in `tests/test_*.rs`; mirror them.
- **No unrelated formatting or dependency bumps.** Dependabot handles deps; `cargo fmt` on touched lines only.
- **Commit messages** follow [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `chore:`, `docs:`, `ci:`, `refactor:`, `test:`). Subject under ~70 chars.
- **CI must pass** — build, test, `cargo deny check`, Alpine smoke-test all run automatically on push.

## Making a release

For my own reference — this section is internal. Contributors don't cut releases.

1. Bump `Cargo.toml` version
2. Bump `npm/package.json` version (keep in sync)
3. Update `CHANGELOG.md` with a new `## [X.Y.Z] - YYYY-MM-DD` section and footer compare link
4. `cargo build` (regenerates `Cargo.lock`)
5. `cargo test` (sanity)
6. Commit as `release: vX.Y.Z`
7. Tag `vX.Y.Z` (annotated)
8. `git push origin main vX.Y.Z` — the release workflow takes over from there

`docs/_config.yml` (Jekyll `version`) and `docs/changelog.md` update themselves via the sync-changelog workflow after the push lands.

Prereleases: use `X.Y.Z-rc1` style. Workflow guards skip crates.io / npm / Homebrew publish on any tag containing a hyphen, so RC tags are safe for dry-running CI gates.

## Code of conduct

By contributing you agree to abide by the [Code of Conduct](CODE_OF_CONDUCT.md).
