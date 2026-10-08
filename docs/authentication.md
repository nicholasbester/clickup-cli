---
layout: default
title: Authentication
description: Personal tokens and bring-your-own-app OAuth for clickup-cli.
permalink: /authentication/
---

# Authentication

Personal-token setup remains available:

```bash
clickup-cli setup --token pk_your_token
```

## Bring your own OAuth app

A ClickUp Workspace owner/admin creates an app in **Settings > Apps > Create new app**. Register exactly `http://127.0.0.1:53682/callback`, then supply that app's client ID and secret. The CLI does not bundle a maintainer secret or use a hosted exchange proxy. Prefer secret injection through your shell's secret manager rather than typing a secret into shell history.

```bash
# Supply these through your environment; values below are placeholders.
export CLICKUP_OAUTH_CLIENT_ID='your-app-id'
export CLICKUP_OAUTH_CLIENT_SECRET='your-app-secret'
clickup-cli auth login
clickup-cli auth login --no-browser
clickup-cli auth login --keyring
clickup-cli auth status
clickup-cli auth logout
```

`--client-id` and `--client-secret` are also supported, but command-line secrets can appear in process listings and shell history. App credentials and authorization codes are never saved in config. Login validates the user, fetches authorized workspaces, and prompts for a default if several exist. Pass `--workspace ID` to select one without prompting. Credentials are saved only after these steps succeed.

The browser must reach the loopback listener on the machine running the CLI. `--no-browser` prints the URL; it is not device authorization or polling. For SSH, arrange a loopback tunnel yourself or authorize on the same machine. `--redirect-port PORT` changes the redirect URL: register the exact emitted URL, including port and `/callback`. Port `0` is experimental and selects a random port; provider acceptance of arbitrary ports is unverified. A busy port fails with instructions instead of silently changing your registered URL. `--login-timeout` bounds the callback wait (180 seconds by default, maximum 600); `--timeout` bounds token exchange and API calls.

**Live verification outstanding:** the automated tests use mock endpoints. They do not prove ClickUp accepts this HTTP loopback redirect or confirm a real token response. An owner must register a BYO app and complete a real round trip before release acceptance. ClickUp warns that non-SSL redirects may become unsupported. See the current [authentication guide](https://developer.clickup.com/docs/authentication) and [token endpoint](https://developer.clickup.com/reference/getaccesstoken). The latter accepts JSON or form bodies; this implementation uses form encoding and accepts a nonempty `access_token`, with absent or Bearer `token_type`. It does not implement refresh or expiry assumptions beyond that documented flow.

## Token kind and precedence

Both the CLI and MCP resolve credentials in this order:

1. `--token TOKEN` (personal by default; pair with `--token-kind oauth` for OAuth).
2. Nonempty `CLICKUP_TOKEN` (personal).
3. Nonempty `CLICKUP_OAUTH_TOKEN` (OAuth).
4. The nearest ancestor `.clickup.toml` with credentials, otherwise global config.

The nearest project config's auth section uses its `storage` selector: `file` (default) reads the file token, while `keychain` reads only the keychain record. A missing or locked selected keychain fails; there is no fallback to another token. Token kind is explicit, never inferred from a token prefix. Old configurations default to personal/file.

Personal tokens send `Authorization: <token>`; OAuth sends `Authorization: Bearer <token>`, including attachments. Supply only the raw token, without `Bearer `. This follows ClickUp's [authentication contract](https://developer.clickup.com/docs/authentication).

```toml
[auth]
token = "your-raw-oauth-access-token"
kind = "oauth"
storage = "file"

[defaults]
workspace_id = "12345"
```

Login writes global config and preserves unrelated settings. An existing project token or flag/environment token may still override it. `CLICKUP_CONFIG=/absolute/path/config.toml` selects one explicit file for reads and writes and disables ancestor/global discovery, useful for isolated environments. Otherwise the global location comes from the OS config directory (`~/Library/Application Support/clickup-cli/config.toml` on macOS, `$XDG_CONFIG_HOME/clickup-cli/config.toml` or `~/.config/clickup-cli/config.toml` on Linux, and the roaming AppData directory on Windows).

`auth status` calls ClickUp and reports the effective identity, token kind, source and configured workspace. It never prints a token. `status` is an offline configuration summary; it does not claim the token is valid. A revoked OAuth token returns an authentication error with re-login guidance.

## Storage and logout

File storage is plaintext by default. Writes use atomic replacement and private file permissions (0600 on Unix); Windows inherits the destination directory's ACL, so use a private user directory. Keep credential files out of source control and backups you share. Symlinked credential files are not overwritten.

`--keyring` selects the OS keychain (`clickup-cli` service, `default` account). The keychain record includes the token and kind; the file contains only kind/storage metadata. There is one shared keychain account per OS user, not a multi-profile vault. Replacing it affects other configs pointing to that account. Locked/unavailable keychains fail without writing plaintext. Switching that config back to file storage removes its keychain credential.

The `keyring` Cargo feature is compiled by default, but no keychain connection is made unless selected in config or with `--keyring`. Backends are macOS Keychain, Windows Credential Manager, and Linux Secret Service via pure Rust D-Bus/crypto (no libsecret or OpenSSL build dependency). Linux keychain storage still needs a running, unlocked Secret Service. Headless builds can use `cargo build --no-default-features`; `--keyring` then gives an actionable error. Cargo defaults cannot vary by platform, so compilation is default-on everywhere, with storage opt-in everywhere. Native OS/WSL browser and keychain acceptance require separate platform testing.

`auth logout` clears auth fields in the nearest project and global configs (or only `CLICKUP_CONFIG` when set), and deletes a keychain credential when one of those configs selects it. Defaults and git settings remain. It does not search unrelated projects, unset environment variables, remove shell history, or revoke server-side access. Revoke app access in ClickUp Settings > Apps separately. If keychain deletion fails, the storage marker is retained so you can unlock the store and retry. Config save failures after a keychain deletion are reported; retry logout to clear the remaining marker.

MCP inherits these credentials for the server process. Per-request/multi-tenant credentials, a maintainer-hosted app, token refresh, and automatic server-side revocation are outside this implementation.
