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
}

// Hand-written so a stray `{:?}` never prints the token.
impl std::fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthConfig")
            .field("token", &"[REDACTED]")
            .finish()
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
        if let Ok(cwd) = std::env::current_dir() {
            if let Some(project_path) = Self::find_project_config(&cwd) {
                let project_config = Self::load_from(&project_path)?;
                if !project_config.auth.token.is_empty() {
                    return Ok(project_config);
                }
            }
        }
        let path = Self::config_path()?;
        Self::load_from(&path)
    }

    pub fn load_from(path: &std::path::Path) -> Result<Self, CliError> {
        if !path.exists() {
            return Err(CliError::ConfigError("Not configured".into()));
        }
        let contents = std::fs::read_to_string(path)?;
        // toml's error Display quotes the offending source line, which may be the
        // token itself. Report only the location.
        toml::from_str(&contents).map_err(|e| {
            let location = e
                .span()
                .map(|span| {
                    let line = contents[..span.start.min(contents.len())]
                        .matches('\n')
                        .count()
                        + 1;
                    format!(" at line {}", line)
                })
                .unwrap_or_default();
            CliError::ConfigError(format!(
                "Invalid config file {}{}: check the TOML syntax",
                path.display(),
                location
            ))
        })
    }

    pub fn save(&self) -> Result<(), CliError> {
        let path = Self::config_path()?;
        self.save_to(&path)
    }

    pub fn save_to(&self, path: &std::path::Path) -> Result<(), CliError> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        std::fs::create_dir_all(parent)?;
        let contents = toml::to_string_pretty(self)
            .map_err(|e| CliError::ConfigError(format!("Failed to serialize config: {}", e)))?;
        // Write to a temp file in the same directory and rename it over the
        // target: a crash never leaves a half-written token behind, and the new
        // file is created private (0600 on Unix) regardless of the umask or of
        // the old file's mode.
        if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(CliError::ConfigError(format!(
                "Refusing to replace symlinked config {}",
                path.display()
            )));
        }
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(contents.as_bytes())?;
        file.as_file().sync_all()?;
        file.persist(path).map_err(|e| CliError::IoError(e.error))?;
        Ok(())
    }
}
