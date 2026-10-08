//! Credential storage. Native keychain access is lazy and strictly opt-in.
use crate::auth_token::AuthToken;
use crate::config::{AuthConfig, Config, TokenStorage};
use crate::error::CliError;
use std::path::Path;

pub trait CredentialStore {
    fn get(&self) -> Result<Option<String>, CliError>;
    fn set(&self, value: &str) -> Result<(), CliError>;
    fn delete(&self) -> Result<(), CliError>;
}

pub struct SystemKeychain;

#[cfg(all(
    feature = "keyring",
    any(target_os = "macos", target_os = "windows", target_os = "linux")
))]
fn entry() -> Result<keyring::Entry, CliError> {
    keyring::Entry::new("clickup-cli", "default").map_err(|_| keychain_error())
}

fn keychain_error() -> CliError {
    CliError::ConfigError("Keychain unavailable. Enable the keyring Cargo feature on macOS/Windows/Linux and unlock your credential store (Linux requires a running Secret Service). No plaintext fallback was made.".into())
}

impl CredentialStore for SystemKeychain {
    fn get(&self) -> Result<Option<String>, CliError> {
        #[cfg(all(
            feature = "keyring",
            any(target_os = "macos", target_os = "windows", target_os = "linux")
        ))]
        {
            read_entry(&entry()?)
        }
        #[cfg(not(all(
            feature = "keyring",
            any(target_os = "macos", target_os = "windows", target_os = "linux")
        )))]
        {
            Err(keychain_error())
        }
    }
    fn set(&self, _value: &str) -> Result<(), CliError> {
        #[cfg(all(
            feature = "keyring",
            any(target_os = "macos", target_os = "windows", target_os = "linux")
        ))]
        {
            entry()?.set_password(_value).map_err(|_| keychain_error())
        }
        #[cfg(not(all(
            feature = "keyring",
            any(target_os = "macos", target_os = "windows", target_os = "linux")
        )))]
        {
            Err(keychain_error())
        }
    }
    fn delete(&self) -> Result<(), CliError> {
        #[cfg(all(
            feature = "keyring",
            any(target_os = "macos", target_os = "windows", target_os = "linux")
        ))]
        {
            delete_entry(&entry()?)
        }
        #[cfg(not(all(
            feature = "keyring",
            any(target_os = "macos", target_os = "windows", target_os = "linux")
        )))]
        {
            Err(keychain_error())
        }
    }
}

#[cfg(all(
    feature = "keyring",
    any(target_os = "macos", target_os = "windows", target_os = "linux")
))]
fn read_entry(entry: &keyring::Entry) -> Result<Option<String>, CliError> {
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(keychain_error()),
    }
}

#[cfg(all(
    feature = "keyring",
    any(target_os = "macos", target_os = "windows", target_os = "linux")
))]
fn delete_entry(entry: &keyring::Entry) -> Result<(), CliError> {
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(keychain_error()),
    }
}

impl AuthConfig {
    pub fn load_token(&self, store: &dyn CredentialStore) -> Result<AuthToken, CliError> {
        let token = match self.storage {
            TokenStorage::File => AuthToken {
                kind: self.kind,
                token: self.token.clone(),
            },
            TokenStorage::Keychain => {
                if !self.token.is_empty() {
                    return Err(CliError::ConfigError(
                        "Keychain config must not also contain a file token".into(),
                    ));
                }
                let value = store.get()?.ok_or_else(|| {
                    CliError::ConfigError(
                        "Keychain token missing; run clickup-cli auth login --keyring".into(),
                    )
                })?;
                let token: AuthToken = serde_json::from_str(&value).map_err(|_| {
                    CliError::ConfigError("Invalid keychain credential record".into())
                })?;
                if token.kind != self.kind {
                    return Err(CliError::ConfigError(
                        "Keychain token kind differs from config; log in again".into(),
                    ));
                }
                token
            }
        };
        token.validate()?;
        Ok(token)
    }
}

/// Save the record and config as a recoverable pair; never fall back to plaintext.
/// Existing keychain entries are removed when switching to file storage.
pub fn save_token(
    config: &mut Config,
    path: &Path,
    token: AuthToken,
    storage: TokenStorage,
    store: &dyn CredentialStore,
) -> Result<(), CliError> {
    token.validate()?;
    let needs_keychain =
        storage == TokenStorage::Keychain || config.auth.storage == TokenStorage::Keychain;
    let previous = if needs_keychain { store.get()? } else { None };
    if storage == TokenStorage::Keychain {
        store.set(
            &serde_json::to_string(&token)
                .map_err(|_| CliError::ConfigError("Cannot encode credential".into()))?,
        )?;
    } else if needs_keychain {
        store.delete()?;
    }
    let old = std::mem::replace(
        &mut config.auth,
        AuthConfig {
            token: if storage == TokenStorage::File {
                token.token
            } else {
                String::new()
            },
            kind: token.kind,
            storage,
        },
    );
    if let Err(error) = config.save_to(path) {
        config.auth = old;
        if needs_keychain {
            let rollback = match previous {
                Some(value) => store.set(&value),
                None => store.delete(),
            };
            if rollback.is_err() {
                return Err(CliError::ConfigError("Config save and keychain rollback failed; inspect credential storage before retrying".into()));
            }
        }
        return Err(error);
    }
    Ok(())
}

pub fn clear_token(
    config: &mut Config,
    path: &Path,
    store: &dyn CredentialStore,
) -> Result<(), CliError> {
    // Delete before removing the marker: a keychain failure remains discoverable.
    if config.auth.storage == TokenStorage::Keychain {
        store.delete()?;
    }
    config.auth = AuthConfig::default();
    config.save_to(path)
}

#[cfg(all(
    test,
    feature = "keyring",
    any(target_os = "macos", target_os = "windows", target_os = "linux")
))]
mod tests {
    use super::*;
    // Per-entry mock: never changes the global backend or accesses the native store.
    #[test]
    fn keyring_mock_record_roundtrip_and_no_entry() {
        let entry =
            keyring::Entry::new_with_credential(Box::new(keyring::mock::MockCredential::default()));
        assert!(read_entry(&entry).unwrap().is_none());
        let token = AuthToken {
            kind: crate::auth_token::TokenKind::Oauth,
            token: "fixture".into(),
        };
        entry
            .set_password(&serde_json::to_string(&token).unwrap())
            .unwrap();
        let value: AuthToken = serde_json::from_str(&read_entry(&entry).unwrap().unwrap()).unwrap();
        assert_eq!(value.kind, crate::auth_token::TokenKind::Oauth);
        assert_eq!(value.token, "fixture");
        delete_entry(&entry).unwrap();
        assert!(read_entry(&entry).unwrap().is_none());
        delete_entry(&entry).unwrap();
    }
}
