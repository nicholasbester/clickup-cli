//! BYO-app authorization code flow. Endpoint overrides are private test seams,
//! never CLI flags or environment variables that could redirect client secrets.
use crate::auth_token::{AuthToken, TokenKind};
use crate::client::ClickUpClient;
use crate::config::{Config, TokenStorage};
use crate::credentials::{save_token, CredentialStore, SystemKeychain};
use crate::error::CliError;
use crate::Cli;
use clap::Args;
use rand::{rngs::OsRng, RngCore};
use serde::Deserialize;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use url::Url;

const AUTHORIZE_URL: &str = "https://app.clickup.com/api";
const TOKEN_URL: &str = "https://api.clickup.com/api/v2/oauth/token";
const MAX_HEADERS: usize = 8192;
const MAX_TOKEN_RESPONSE: usize = 65536;

#[derive(Args)]
pub struct LoginArgs {
    /// BYO app client ID (or CLICKUP_OAUTH_CLIENT_ID)
    #[arg(long)]
    pub client_id: Option<String>,
    /// BYO app secret (prefer CLICKUP_OAUTH_CLIENT_SECRET to avoid shell history)
    #[arg(long)]
    pub client_secret: Option<String>,
    /// Print the URL without opening a browser (callback still needs this machine)
    #[arg(long)]
    pub no_browser: bool,
    /// Store the token in the OS keychain; fail rather than fall back to plaintext
    #[arg(long)]
    pub keyring: bool,
    /// Register http://127.0.0.1:PORT/callback in your app. 0 requests an ephemeral port.
    #[arg(long, default_value_t = 53682)]
    pub redirect_port: u16,
    /// Maximum wait for the browser callback, in seconds
    #[arg(long, default_value_t = 180, value_parser = clap::value_parser!(u64).range(1..=600))]
    pub login_timeout: u64,
}

fn error(message: impl Into<String>) -> CliError {
    CliError::ConfigError(message.into())
}

pub async fn login(args: LoginArgs, cli: &Cli) -> Result<(), CliError> {
    let client_id = args
        .client_id
        .clone()
        .or_else(|| std::env::var("CLICKUP_OAUTH_CLIENT_ID").ok());
    let secret = args
        .client_secret
        .clone()
        .or_else(|| std::env::var("CLICKUP_OAUTH_CLIENT_SECRET").ok());
    let (client_id, secret) = match (client_id, secret) {
        (Some(id), Some(secret)) if !id.trim().is_empty() && !secret.trim().is_empty() => (id, secret),
        _ => return Err(error(format!("Bring your own ClickUp OAuth app: a Workspace owner/admin must open Settings > Apps > Create new app and register http://127.0.0.1:{}/callback (matching --redirect-port). Set CLICKUP_OAUTH_CLIENT_ID and CLICKUP_OAUTH_CLIENT_SECRET, or pass --client-id and --client-secret. No maintainer app or secret is bundled. See https://developer.clickup.com/docs/authentication", args.redirect_port))),
    };
    if cli.timeout == 0 {
        return Err(error("OAuth HTTP --timeout must be greater than zero"));
    }
    let path = Config::config_path()?;
    let mut config = if path.exists() {
        Config::load_from(&path)?
    } else {
        Config::default()
    };
    // Probe explicitly selected keychain before requesting consent. No lookup for file users.
    if args.keyring {
        SystemKeychain.get()?;
    }
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, args.redirect_port)).await
        .map_err(|_| error("Could not bind OAuth loopback port. Stop the other listener or select another --redirect-port and register that exact redirect URL in ClickUp."))?;
    let redirect = format!(
        "http://127.0.0.1:{}/callback",
        listener.local_addr()?.port()
    );
    let state = generate_state()?;
    let url = authorization_url(&client_id, &redirect, &state)?;
    eprintln!(
        "Authorize this app in your browser:\n{url}\nWaiting up to {} seconds on {redirect}",
        args.login_timeout
    );
    if !args.no_browser && open::that_detached(url.as_str()).is_err() {
        eprintln!("Could not open browser. Copy the URL above into a browser on this machine.");
    }
    let code = receive_callback(listener, &state, Duration::from_secs(args.login_timeout)).await?;
    let token = exchange_token(
        TOKEN_URL,
        &client_id,
        &secret,
        &code,
        Duration::from_secs(cli.timeout),
    )
    .await?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    finish_login(
        &client,
        cli.workspace.as_deref(),
        &mut config,
        &path,
        token,
        if args.keyring {
            TokenStorage::Keychain
        } else {
            TokenStorage::File
        },
        &SystemKeychain,
    )
    .await?;
    eprintln!(
        "OAuth credentials saved to {}{}",
        path.display(),
        if args.keyring {
            " (token in OS keychain)"
        } else {
            " (plaintext token; keep this file private)"
        }
    );
    eprintln!("Flag/environment tokens and ancestor .clickup.toml credentials may override this login. Use clickup-cli auth status to check the effective identity.");
    Ok(())
}

async fn finish_login(
    client: &ClickUpClient,
    preferred: Option<&str>,
    config: &mut Config,
    path: &std::path::Path,
    token: AuthToken,
    storage: TokenStorage,
    store: &dyn CredentialStore,
) -> Result<(), CliError> {
    config.defaults.workspace_id =
        Some(super::setup::validate_and_select_workspace(client, preferred).await?);
    save_token(config, path, token, storage, store)
}

fn generate_state() -> Result<String, CliError> {
    let mut bytes = [0u8; 32];
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| error("Operating system random source unavailable"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn authorization_url(client_id: &str, redirect: &str, state: &str) -> Result<Url, CliError> {
    let mut url = Url::parse(AUTHORIZE_URL).map_err(|_| error("Invalid authorization endpoint"))?;
    url.query_pairs_mut().extend_pairs([
        ("client_id", client_id),
        ("redirect_uri", redirect),
        ("state", state),
    ]);
    Ok(url)
}

async fn receive_callback(
    listener: TcpListener,
    state: &str,
    timeout: Duration,
) -> Result<String, CliError> {
    let port = listener.local_addr()?.port();
    tokio::time::timeout(timeout, async {
        let (mut stream, peer) = listener.accept().await?;
        if !peer.ip().is_loopback() { return Err(error("OAuth callback must originate from loopback")); }
        let result = tokio::time::timeout(Duration::from_secs(5), read_callback(&mut stream, state, port)).await
            .map_err(|_| error("OAuth callback request timed out"))?;
        let (status, body) = if result.is_ok() {
            ("200 OK", "Authorization received. Return to the terminal to finish login. You may close this tab.")
        } else {
            ("400 Bad Request", "Authorization callback rejected. Return to the terminal and try login again.")
        };
        let reply = format!("HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'none'\r\nReferrer-Policy: no-referrer\r\n\r\n{body}", body.len());
        // A browser disconnect must not turn a verified code into an unbounded wait.
        let _ = tokio::time::timeout(Duration::from_secs(1), stream.write_all(reply.as_bytes())).await;
        result
    }).await.map_err(|_| error("OAuth login timed out; no credentials saved. Run clickup-cli auth login to retry."))?
}

async fn read_callback(stream: &mut TcpStream, state: &str, port: u16) -> Result<String, CliError> {
    let mut data = Vec::with_capacity(MAX_HEADERS);
    loop {
        let mut chunk = [0u8; 1024];
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Err(error("Incomplete OAuth callback"));
        }
        if data.len() + n > MAX_HEADERS {
            return Err(error("OAuth callback headers too large"));
        }
        data.extend_from_slice(&chunk[..n]);
        if data.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    parse_callback(&data, state, port)
}

fn parse_callback(data: &[u8], state: &str, port: u16) -> Result<String, CliError> {
    let mut headers = [httparse::EMPTY_HEADER; 32];
    let mut request = httparse::Request::new(&mut headers);
    let parsed = request
        .parse(data)
        .map_err(|_| error("Malformed OAuth callback HTTP request"))?;
    if parsed != httparse::Status::Complete(data.len())
        || request.method != Some("GET")
        || request.version != Some(1)
    {
        return Err(error("OAuth callback requires HTTP/1.1 GET"));
    }
    let hosts: Vec<_> = request
        .headers
        .iter()
        .filter(|h| h.name.eq_ignore_ascii_case("host"))
        .collect();
    if hosts.len() != 1 || hosts[0].value != format!("127.0.0.1:{port}").as_bytes() {
        return Err(error("Invalid OAuth callback host"));
    }
    if request.headers.iter().any(|h| {
        h.name.eq_ignore_ascii_case("transfer-encoding")
            || (h.name.eq_ignore_ascii_case("content-length") && h.value != b"0")
    }) {
        return Err(error("OAuth callback must not contain a request body"));
    }
    let target = request.path.ok_or_else(|| error("Missing callback path"))?;
    if !target.starts_with("/callback?") || target.contains('#') {
        return Err(error("Unexpected OAuth callback path"));
    }
    let query = &target["/callback?".len()..];
    let bytes = query.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'%'
            && (i + 2 >= bytes.len()
                || !bytes[i + 1].is_ascii_hexdigit()
                || !bytes[i + 2].is_ascii_hexdigit())
        {
            return Err(error("Invalid callback query encoding"));
        }
    }
    let mut values = std::collections::HashMap::new();
    for (key, value) in url::form_urlencoded::parse(bytes) {
        if values
            .insert(key.into_owned(), value.into_owned())
            .is_some()
        {
            return Err(error("Duplicate OAuth callback parameter"));
        }
    }
    if values.get("state").map(String::as_str) != Some(state) {
        return Err(error("OAuth state mismatch; no credentials saved"));
    }
    if values.contains_key("error") {
        return Err(error(
            "OAuth authorization was denied or failed; no credentials saved",
        ));
    }
    let code = values
        .remove("code")
        .ok_or_else(|| error("OAuth callback has no authorization code"))?;
    if code.is_empty() || !code.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(error("Invalid OAuth authorization code"));
    }
    Ok(code)
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    // Public docs do not guarantee token_type is present. Accept absent or Bearer only.
    token_type: Option<String>,
}

async fn exchange_token(
    endpoint: &str,
    client_id: &str,
    secret: &str,
    code: &str,
    timeout: Duration,
) -> Result<AuthToken, CliError> {
    let http = reqwest::Client::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .build()
        .map_err(|_| error("Could not create OAuth HTTP client"))?;
    let mut resp = http.post(endpoint).form(&[("client_id", client_id), ("client_secret", secret), ("code", code)])
        .send().await.map_err(|_| error("OAuth token exchange failed or timed out; run login again. The code was not retried."))?;
    if !resp.status().is_success() {
        // Provider responses can echo submitted secrets/codes; never print them.
        return Err(error(format!(
            "OAuth token exchange failed (HTTP {}); run login again. The code was not retried.",
            resp.status().as_u16()
        )));
    }
    if resp
        .content_length()
        .is_some_and(|n| n > MAX_TOKEN_RESPONSE as u64)
    {
        return Err(error("OAuth token response too large"));
    }
    let mut body = Vec::new();
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|_| error("Could not read OAuth token response"))?
    {
        if body.len() + chunk.len() > MAX_TOKEN_RESPONSE {
            return Err(error("OAuth token response too large"));
        }
        body.extend_from_slice(&chunk);
    }
    let response: TokenResponse = serde_json::from_slice(&body)
        .map_err(|_| error("Invalid OAuth token response (expected access_token)"))?;
    if response
        .token_type
        .is_some_and(|t| !t.eq_ignore_ascii_case("bearer"))
    {
        return Err(error("Unsupported OAuth token_type"));
    }
    let token = AuthToken {
        kind: TokenKind::Oauth,
        token: response.access_token,
    };
    token.validate()?;
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_string, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn request(target: &str) -> Vec<u8> {
        format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1:1234\r\n\r\n").into_bytes()
    }

    #[test]
    fn state_has_256_bits_and_authorization_parameters_are_encoded() {
        let first = generate_state().unwrap();
        assert_eq!(first.len(), 64);
        assert!(first.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_ne!(first, generate_state().unwrap());
        let url = authorization_url("id&evil=x", "http://127.0.0.1:1234/callback", &first).unwrap();
        let params: std::collections::HashMap<_, _> = url.query_pairs().collect();
        assert_eq!(params.len(), 3);
        assert_eq!(params["client_id"], "id&evil=x");
        assert_eq!(params["state"], first);
    }

    #[test]
    fn callback_validates_state_method_path_host_and_unambiguous_parameters() {
        assert_eq!(
            parse_callback(
                &request("/callback?code=one%2Btwo&state=expected"),
                "expected",
                1234
            )
            .unwrap(),
            "one+two"
        );
        for target in [
            "/callback?code=secret&state=wrong",
            "/callback?code=secret",
            "/callback?state=expected",
            "/callback?code=&state=expected",
            "/callback?code=one&code=two&state=expected",
            "/callback?code=secret&state=expected&state=expected",
            "/callback?error=access_denied&state=expected",
            "/callback?code=secret&error=denied&state=expected",
            "/callback?code=%ZZ&state=expected",
            "/callback?code=%0A&state=expected",
            "/callback?code=secret&state=expected#fragment",
            "/favicon.ico?code=secret&state=expected",
            "http://127.0.0.1:1234/callback?code=secret&state=expected",
        ] {
            let err = parse_callback(&request(target), "expected", 1234).unwrap_err();
            assert!(!err.to_string().contains("secret"));
        }
        let valid = String::from_utf8(request("/callback?code=ok&state=expected")).unwrap();
        for bad in [
            valid.replace("GET ", "POST "),
            valid.replace("127.0.0.1:1234", "evil.test"),
            valid.replace("Host:", "X-Host:"),
            valid.replace("\r\n\r\n", "\r\nHost: 127.0.0.1:1234\r\n\r\n"),
            valid.replace("\r\n\r\n", "\r\nTransfer-Encoding: chunked\r\n\r\n"),
            valid.replace("\r\n\r\n", "\r\nContent-Length: 42\r\n\r\n"),
        ] {
            assert!(parse_callback(bad.as_bytes(), "expected", 1234).is_err());
        }
    }

    #[tokio::test]
    async fn listener_is_one_shot_and_times_out_or_rejects_oversize_requests() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            receive_callback(listener, "state", Duration::from_secs(2)).await
        });
        let response = reqwest::get(format!("http://{addr}/callback?code=fixture&state=state"))
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(task.await.unwrap().unwrap(), "fixture");
        assert!(TcpStream::connect(addr).await.is_err());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        assert!(
            receive_callback(listener, "state", Duration::from_millis(10))
                .await
                .unwrap_err()
                .to_string()
                .contains("timed out")
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            receive_callback(listener, "state", Duration::from_secs(2)).await
        });
        let mut stream = TcpStream::connect(addr).await.unwrap();
        let _ = stream.write_all(&vec![b'x'; MAX_HEADERS + 1]).await;
        assert!(task
            .await
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("too large"));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            receive_callback(listener, "state", Duration::from_millis(20)).await
        });
        let _slow_stream = TcpStream::connect(addr).await.unwrap();
        assert!(task.await.unwrap().is_err());
    }

    #[tokio::test]
    async fn rejected_callback_never_returns_a_code() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            receive_callback(listener, "expected", Duration::from_secs(2)).await
        });
        let response = reqwest::get(format!("http://{addr}/callback?code=fixture&state=wrong"))
            .await
            .unwrap();
        assert_eq!(response.status(), 400);
        assert!(!response.text().await.unwrap().contains("fixture"));
        assert!(task.await.unwrap().is_err());
    }

    #[tokio::test]
    async fn exchange_is_form_encoded_and_accepts_optional_bearer_type() {
        for extra in [
            serde_json::json!({}),
            serde_json::json!({"token_type":"Bearer"}),
        ] {
            let server = MockServer::start().await;
            let mut body = serde_json::json!({"access_token":"fixture"});
            body.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            Mock::given(method("POST"))
                .and(path("/token"))
                .and(header("content-type", "application/x-www-form-urlencoded"))
                .and(body_string(
                    "client_id=id%26&client_secret=secret%2B&code=code%3D",
                ))
                .respond_with(ResponseTemplate::new(200).set_body_json(body))
                .expect(1)
                .mount(&server)
                .await;
            let token = exchange_token(
                &format!("{}/token", server.uri()),
                "id&",
                "secret+",
                "code=",
                Duration::from_secs(2),
            )
            .await
            .unwrap();
            assert_eq!(token.kind, TokenKind::Oauth);
            assert_eq!(token.token, "fixture");
        }
    }

    #[tokio::test]
    async fn exchange_rejects_errors_redirects_bad_schema_and_large_bodies_without_retry_or_leaks()
    {
        let responses = vec![
            ResponseTemplate::new(400).set_body_string("echo-client-secret-and-code"),
            ResponseTemplate::new(500).set_body_string("echo-client-secret-and-code"),
            ResponseTemplate::new(302).insert_header("location", "http://127.0.0.1:1/leak"),
            ResponseTemplate::new(200).set_body_string("echo-client-secret-and-code"),
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"token_type":"Bearer"})),
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"access_token":""})),
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"access_token":"fixture","token_type":"MAC"})),
            ResponseTemplate::new(200).set_body_string("x".repeat(MAX_TOKEN_RESPONSE + 1)),
        ];
        for response in responses {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(response)
                .expect(1)
                .mount(&server)
                .await;
            let error = exchange_token(
                &server.uri(),
                "id",
                "client-secret",
                "code",
                Duration::from_secs(2),
            )
            .await
            .unwrap_err();
            assert!(!error.to_string().contains("client-secret"));
        }
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(1)))
            .expect(1)
            .mount(&server)
            .await;
        assert!(exchange_token(
            &server.uri(),
            "id",
            "secret",
            "code",
            Duration::from_millis(30)
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn fixture_flow_callback_exchange_validate_workspace_and_persist() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"access_token":"fixture-oauth"})),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/user"))
            .and(header("authorization", "Bearer fixture-oauth"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"user":{"username":"fixture"}})),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/team"))
            .and(header("authorization", "Bearer fixture-oauth"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"teams":[{"id":"123","name":"Fixture"}]})),
            )
            .expect(1)
            .mount(&server)
            .await;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = generate_state().unwrap();
        let callback_state = state.clone();
        let task = tokio::spawn(async move {
            receive_callback(listener, &callback_state, Duration::from_secs(2)).await
        });
        reqwest::get(format!(
            "http://{addr}/callback?code=fixture-code&state={state}"
        ))
        .await
        .unwrap();
        let code = task.await.unwrap().unwrap();
        let token = exchange_token(
            &format!("{}/token", server.uri()),
            "id",
            "secret",
            &code,
            Duration::from_secs(2),
        )
        .await
        .unwrap();
        let client = ClickUpClient::new(&token, 2)
            .unwrap()
            .with_base_url(&server.uri());
        let mut config = Config::default();
        struct NoKeychain;
        impl CredentialStore for NoKeychain {
            fn get(&self) -> Result<Option<String>, CliError> {
                panic!("file flow touched keychain")
            }
            fn set(&self, _: &str) -> Result<(), CliError> {
                panic!("file flow touched keychain")
            }
            fn delete(&self) -> Result<(), CliError> {
                panic!("file flow touched keychain")
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        finish_login(
            &client,
            None,
            &mut config,
            &path,
            token,
            TokenStorage::File,
            &NoKeychain,
        )
        .await
        .unwrap();
        let saved = Config::load_from(&path).unwrap();
        assert_eq!(saved.auth.kind, TokenKind::Oauth);
        assert_eq!(saved.auth.token, "fixture-oauth");
        assert_eq!(saved.defaults.workspace_id.as_deref(), Some("123"));
    }
    #[tokio::test]
    async fn failed_user_or_workspace_validation_preserves_existing_credentials() {
        for mode in ["revoked", "empty", "unauthorized-workspace"] {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/v2/user"))
                .respond_with(if mode == "revoked" {
                    ResponseTemplate::new(401)
                } else {
                    ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!({"user":{"username":"Fixture"}}))
                })
                .mount(&server)
                .await;
            Mock::given(method("GET"))
                .and(path("/v2/team"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(if mode == "empty" {
                        serde_json::json!({"teams":[]})
                    } else {
                        serde_json::json!({"teams":[{"id":"123"}]})
                    }),
                )
                .mount(&server)
                .await;
            let token = AuthToken {
                kind: TokenKind::Oauth,
                token: "new-fixture".into(),
            };
            let client = ClickUpClient::new(&token, 2)
                .unwrap()
                .with_base_url(&server.uri());
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("config.toml");
            let mut config = Config::default();
            config.auth.token = "previous-fixture".into();
            config.save_to(&path).unwrap();
            let before = std::fs::read(&path).unwrap();
            struct NoStore;
            impl CredentialStore for NoStore {
                fn get(&self) -> Result<Option<String>, CliError> {
                    panic!("failed validation touched keychain")
                }
                fn set(&self, _: &str) -> Result<(), CliError> {
                    panic!("failed validation touched keychain")
                }
                fn delete(&self) -> Result<(), CliError> {
                    panic!("failed validation touched keychain")
                }
            }
            let preferred = if mode == "unauthorized-workspace" {
                Some("999")
            } else {
                None
            };
            assert!(finish_login(
                &client,
                preferred,
                &mut config,
                &path,
                token,
                TokenStorage::File,
                &NoStore
            )
            .await
            .is_err());
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
    }
}
