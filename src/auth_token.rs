use crate::error::CliError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum TokenKind {
    #[default]
    Personal,
    Oauth,
}

impl TokenKind {
    pub fn is_personal(&self) -> bool {
        *self == Self::Personal
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Personal => "personal",
            Self::Oauth => "oauth",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub kind: TokenKind,
    pub token: String,
}

impl std::fmt::Debug for AuthToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthToken")
            .field("kind", &self.kind)
            .field("token", &"[REDACTED]")
            .finish()
    }
}

impl AuthToken {
    pub fn personal(token: impl Into<String>) -> Self {
        Self {
            kind: TokenKind::Personal,
            token: token.into(),
        }
    }

    pub fn validate(&self) -> Result<(), CliError> {
        if self.token.is_empty() || !self.token.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(CliError::ConfigError(
                "Token must be nonempty and contain no whitespace or control characters".into(),
            ));
        }
        Ok(())
    }

    pub fn header(&self) -> Result<reqwest::header::HeaderValue, CliError> {
        self.validate()?;
        let value = match self.kind {
            TokenKind::Personal => self.token.clone(),
            TokenKind::Oauth => format!("Bearer {}", self.token),
        };
        let mut header = reqwest::header::HeaderValue::from_str(&value)
            .map_err(|_| CliError::ConfigError("Invalid token header".into()))?;
        header.set_sensitive(true);
        Ok(header)
    }
}
