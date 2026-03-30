use thiserror::Error;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("{message}")]
    ClientError { message: String, status: u16 },
    #[error("{message}")]
    AuthError { message: String },
    #[error("{message}")]
    NotFound { message: String, resource_id: String },
    #[error("{message}")]
    RateLimited { message: String, retry_after: Option<u64> },
    #[error("{message}")]
    ServerError { message: String },
    #[error("{0}")]
    ConfigError(String),
    #[error("{0}")]
    IoError(#[from] std::io::Error),
}

impl CliError {
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::ClientError { .. } => 1,
            CliError::AuthError { .. } => 2,
            CliError::NotFound { .. } => 3,
            CliError::RateLimited { .. } => 4,
            CliError::ServerError { .. } => 5,
            CliError::ConfigError(_) => 1,
            CliError::IoError(_) => 1,
        }
    }

    pub fn print(&self, output_mode: &str) {
        if output_mode == "json" {
            let json = serde_json::json!({
                "error": true,
                "message": self.to_string(),
                "exit_code": self.exit_code(),
                "hint": self.hint(),
            });
            eprintln!("{}", serde_json::to_string_pretty(&json).unwrap());
        } else {
            eprintln!("Error: {}", self);
            if let Some(status) = self.status() {
                eprintln!("  Status:  {}", status);
            }
            if let Some(hint) = self.hint() {
                eprintln!("  Hint:    {}", hint);
            }
        }
    }

    pub fn status(&self) -> Option<u16> {
        match self {
            CliError::ClientError { status, .. } => Some(*status),
            CliError::AuthError { .. } => Some(401),
            CliError::NotFound { .. } => Some(404),
            CliError::RateLimited { .. } => Some(429),
            CliError::ServerError { .. } => Some(500),
            _ => None,
        }
    }

    pub fn hint(&self) -> Option<String> {
        match self {
            CliError::AuthError { .. } => {
                Some("Check your API token, or run 'clickup setup' to reconfigure".into())
            }
            CliError::NotFound { resource_id, .. } => {
                Some(format!("Check the ID '{}', or use --custom-task-id if using a custom task ID", resource_id))
            }
            CliError::RateLimited { retry_after, .. } => {
                retry_after.map(|s| format!("Rate limited. Retry after {} seconds", s))
            }
            CliError::ServerError { .. } => {
                Some("ClickUp server error. Try again in a few seconds.".into())
            }
            CliError::ConfigError(_) => {
                Some("Run 'clickup setup' to configure your API token".into())
            }
            _ => None,
        }
    }
}
