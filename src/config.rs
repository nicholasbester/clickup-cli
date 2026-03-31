use crate::error::CliError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub auth: AuthConfig,
    #[serde(default)]
    pub defaults: DefaultsConfig,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AuthConfig {
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct DefaultsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

impl Config {
    pub fn config_path() -> Result<PathBuf, CliError> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| CliError::ConfigError("Could not determine config directory".into()))?;
        Ok(config_dir.join("clickup-cli").join("config.toml"))
    }

    /// Load config: project-level .clickup.toml first, then global config
    pub fn load() -> Result<Self, CliError> {
        // 1. Project-level config (.clickup.toml in current directory)
        let project_path = PathBuf::from(".clickup.toml");
        if project_path.exists() {
            let project_config = Self::load_from(&project_path)?;
            if !project_config.auth.token.is_empty() {
                return Ok(project_config);
            }
        }
        // 2. Global config
        let path = Self::config_path()?;
        Self::load_from(&path)
    }

    pub fn load_from(path: &std::path::Path) -> Result<Self, CliError> {
        if !path.exists() {
            return Err(CliError::ConfigError("Not configured".into()));
        }
        let contents = std::fs::read_to_string(path)?;
        toml::from_str(&contents)
            .map_err(|e| CliError::ConfigError(format!("Invalid config file: {}", e)))
    }

    pub fn save(&self) -> Result<(), CliError> {
        let path = Self::config_path()?;
        self.save_to(&path)
    }

    pub fn save_to(&self, path: &std::path::Path) -> Result<(), CliError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)
            .map_err(|e| CliError::ConfigError(format!("Failed to serialize config: {}", e)))?;
        std::fs::write(path, contents)?;
        Ok(())
    }
}
