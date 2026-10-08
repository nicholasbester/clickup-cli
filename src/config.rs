use crate::auth_token::TokenKind;
use crate::error::CliError;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub defaults: DefaultsConfig,
    #[serde(default)]
    pub git: GitConfig,
}

#[derive(Serialize, Deserialize, Default)]
pub struct AuthConfig {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub token: String,
    #[serde(default, skip_serializing_if = "TokenKind::is_personal")]
    pub kind: TokenKind,
    #[serde(default, skip_serializing_if = "TokenStorage::is_file")]
    pub storage: TokenStorage,
}

impl std::fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthConfig")
            .field("token", &"[REDACTED]")
            .field("kind", &self.kind)
            .field("storage", &self.storage)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TokenStorage {
    #[default]
    File,
    Keychain,
}

impl TokenStorage {
    pub fn is_file(&self) -> bool {
        *self == Self::File
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct DefaultsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GitConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verbose: Option<bool>,
}

impl Config {
    pub fn config_path() -> Result<PathBuf, CliError> {
        if let Some(path) = std::env::var_os("CLICKUP_CONFIG") {
            if path.is_empty() {
                return Err(CliError::ConfigError(
                    "CLICKUP_CONFIG must not be empty".into(),
                ));
            }
            return Ok(PathBuf::from(path));
        }
        let config_dir = dirs::config_dir()
            .ok_or_else(|| CliError::ConfigError("Could not determine config directory".into()))?;
        Ok(config_dir.join("clickup-cli").join("config.toml"))
    }

    /// Walk from `start` up to filesystem root, returning the nearest `.clickup.toml`.
    pub fn find_project_config(start: &std::path::Path) -> Option<PathBuf> {
        start.ancestors().find_map(|dir| {
            let candidate = dir.join(".clickup.toml");
            candidate.is_file().then_some(candidate)
        })
    }

    /// Load config: nearest .clickup.toml walking up from CWD, then global config
    pub fn load() -> Result<Self, CliError> {
        Self::load_from(&Self::active_path()?)
    }

    /// An explicit config path isolates all config access, including ancestor lookup.
    pub fn active_path() -> Result<PathBuf, CliError> {
        if std::env::var_os("CLICKUP_CONFIG").is_none() {
            if let Ok(cwd) = std::env::current_dir() {
                if let Some(path) = Self::find_project_config(&cwd) {
                    let config = Self::load_from(&path)?;
                    if !config.auth.token.is_empty()
                        || config.auth.storage == TokenStorage::Keychain
                    {
                        return Ok(path);
                    }
                }
            }
        }
        Self::config_path()
    }

    pub fn load_from(path: &std::path::Path) -> Result<Self, CliError> {
        if !path.exists() {
            return Err(CliError::ConfigError("Not configured".into()));
        }
        let contents = std::fs::read_to_string(path)?;
        toml::from_str(&contents).map_err(|_| {
            CliError::ConfigError("Invalid config file (check TOML and auth kind/storage)".into())
        })
    }

    pub fn save(&self) -> Result<(), CliError> {
        let path = Self::config_path()?;
        self.save_to(&path)
    }

    pub fn save_to(&self, path: &std::path::Path) -> Result<(), CliError> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)
            .map_err(|e| CliError::ConfigError(format!("Failed to serialize config: {}", e)))?;
        // A same-directory atomic replacement prevents partial credentials and starts
        // with private permissions (0600 on Unix), including when replacing older files.
        if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(CliError::ConfigError(
                "Refusing to replace a symlinked config".into(),
            ));
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(contents.as_bytes())?;
        file.as_file().sync_all()?;
        file.persist(path).map_err(|e| CliError::IoError(e.error))?;
        Ok(())
    }
}
