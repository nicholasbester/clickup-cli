use crate::error::CliError;

pub struct ClickUpClient {
    http: reqwest::Client,
    base_url: String,
    token: String,
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
        })
    }

    pub async fn get(&self, path: &str) -> Result<serde_json::Value, CliError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .get(&url)
            .header("Authorization", &self.token)
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| CliError::ClientError {
                message: format!("Request failed: {}", e),
                status: 0,
            })?;
        self.handle_response(resp).await
    }

    pub async fn post(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value, CliError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .post(&url)
            .header("Authorization", &self.token)
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| CliError::ClientError {
                message: format!("Request failed: {}", e),
                status: 0,
            })?;
        self.handle_response(resp).await
    }

    pub async fn put(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value, CliError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .put(&url)
            .header("Authorization", &self.token)
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| CliError::ClientError {
                message: format!("Request failed: {}", e),
                status: 0,
            })?;
        self.handle_response(resp).await
    }

    pub async fn delete(&self, path: &str) -> Result<serde_json::Value, CliError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .delete(&url)
            .header("Authorization", &self.token)
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| CliError::ClientError {
                message: format!("Request failed: {}", e),
                status: 0,
            })?;
        self.handle_response(resp).await
    }

    async fn handle_response(&self, resp: reqwest::Response) -> Result<serde_json::Value, CliError> {
        let status = resp.status().as_u16();
        if status == 200 {
            let body: serde_json::Value = resp.json().await.map_err(|e| CliError::ClientError {
                message: format!("Failed to parse response: {}", e),
                status,
            })?;
            return Ok(body);
        }
        let body_text = resp.text().await.unwrap_or_default();
        let message = serde_json::from_str::<serde_json::Value>(&body_text)
            .ok()
            .and_then(|v| v.get("err").and_then(|e| e.as_str()).map(String::from))
            .unwrap_or_else(|| format!("HTTP {}", status));

        match status {
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
        }
    }

    #[cfg(test)]
    pub fn with_base_url(mut self, base_url: &str) -> Self {
        self.base_url = base_url.to_string();
        self
    }
}
