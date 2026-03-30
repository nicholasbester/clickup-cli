use crate::error::CliError;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

pub struct ClickUpClient {
    http: reqwest::Client,
    base_url: String,
    token: String,
    rate_limit_remaining: Arc<AtomicU64>,
    rate_limit_reset: Arc<AtomicU64>,
}

impl ClickUpClient {
    pub fn new(token: &str, timeout_secs: u64) -> Result<Self, CliError> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()
            .map_err(|e| CliError::ClientError {
                message: format!("Failed to create HTTP client: {}", e),
                status: 0,
            })?;
        Ok(Self {
            http,
            base_url: "https://api.clickup.com/api".to_string(),
            token: token.to_string(),
            rate_limit_remaining: Arc::new(AtomicU64::new(100)),
            rate_limit_reset: Arc::new(AtomicU64::new(0)),
        })
    }

    fn update_rate_limits(&self, headers: &reqwest::header::HeaderMap) {
        if let Some(remaining) = headers.get("X-RateLimit-Remaining") {
            if let Ok(val) = remaining.to_str().unwrap_or("0").parse::<u64>() {
                self.rate_limit_remaining.store(val, Ordering::Relaxed);
            }
        }
        if let Some(reset) = headers.get("X-RateLimit-Reset") {
            if let Ok(val) = reset.to_str().unwrap_or("0").parse::<u64>() {
                self.rate_limit_reset.store(val, Ordering::Relaxed);
            }
        }
    }

    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, CliError> {
        let url = format!("{}{}", self.base_url, path);
        let max_retries = 3;

        for attempt in 0..=max_retries {
            let mut req = self
                .http
                .request(method.clone(), &url)
                .header("Authorization", &self.token)
                .header("Content-Type", "application/json");

            if let Some(b) = body {
                req = req.json(b);
            }

            let resp = req.send().await.map_err(|e| CliError::ClientError {
                message: format!("Request failed: {}", e),
                status: 0,
            })?;

            let status = resp.status().as_u16();
            self.update_rate_limits(resp.headers());

            if status == 200 {
                let json: serde_json::Value = resp.json().await.map_err(|e| CliError::ClientError {
                    message: format!("Failed to parse response: {}", e),
                    status,
                })?;
                return Ok(json);
            }

            // Retry on 429 — wait for rate limit reset, retry once
            if status == 429 && attempt == 0 {
                let reset = self.rate_limit_reset.load(Ordering::Relaxed);
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let wait = if reset > now { reset - now } else { 1 };
                eprintln!("Rate limited. Waiting {} seconds...", wait);
                sleep(Duration::from_secs(wait)).await;
                continue;
            }

            // Retry on 5xx with exponential backoff
            if (500..=599).contains(&status) && attempt < max_retries {
                let wait = 1u64 << attempt; // 1, 2, 4 seconds
                eprintln!("Server error ({}). Retrying in {}s...", status, wait);
                sleep(Duration::from_secs(wait)).await;
                continue;
            }

            // No retry — return error
            let body_text = resp.text().await.unwrap_or_default();
            let message = serde_json::from_str::<serde_json::Value>(&body_text)
                .ok()
                .and_then(|v| v.get("err").and_then(|e| e.as_str()).map(String::from))
                .unwrap_or_else(|| format!("HTTP {}", status));

            return match status {
                401 => Err(CliError::AuthError { message }),
                404 => Err(CliError::NotFound {
                    message,
                    resource_id: String::new(),
                }),
                429 => Err(CliError::RateLimited {
                    message,
                    retry_after: None,
                }),
                500..=599 => Err(CliError::ServerError { message }),
                _ => Err(CliError::ClientError { message, status }),
            };
        }

        Err(CliError::ServerError {
            message: "Max retries exceeded".into(),
        })
    }

    pub async fn get(&self, path: &str) -> Result<serde_json::Value, CliError> {
        self.request(reqwest::Method::GET, path, None).await
    }

    pub async fn post(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, CliError> {
        self.request(reqwest::Method::POST, path, Some(body)).await
    }

    pub async fn put(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, CliError> {
        self.request(reqwest::Method::PUT, path, Some(body)).await
    }

    pub async fn delete(&self, path: &str) -> Result<serde_json::Value, CliError> {
        self.request(reqwest::Method::DELETE, path, None).await
    }

    /// Override the base URL. Used in tests to point at a mock server.
    pub fn with_base_url(mut self, base_url: &str) -> Self {
        self.base_url = base_url.to_string();
        self
    }
}
