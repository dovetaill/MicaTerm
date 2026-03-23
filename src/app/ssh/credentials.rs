//! Credential storage adapters for SSH secrets.

use std::collections::HashMap;
use std::sync::Mutex;

use anyhow::{Context, Result, anyhow};

const CREDENTIAL_SERVICE_NAME: &str = "mica-term";

pub trait CredentialStore: Send + Sync {
    fn put_secret(&self, key: &str, value: &str) -> Result<()>;
    fn get_secret(&self, key: &str) -> Result<Option<String>>;
    fn delete_secret(&self, key: &str) -> Result<()>;
}

#[derive(Debug, Default)]
pub struct MemoryCredentialStore {
    secrets: Mutex<HashMap<String, String>>,
}

impl CredentialStore for MemoryCredentialStore {
    fn put_secret(&self, key: &str, value: &str) -> Result<()> {
        let mut secrets = self
            .secrets
            .lock()
            .map_err(|_| anyhow!("memory credential store lock poisoned"))?;
        secrets.insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn get_secret(&self, key: &str) -> Result<Option<String>> {
        let secrets = self
            .secrets
            .lock()
            .map_err(|_| anyhow!("memory credential store lock poisoned"))?;
        Ok(secrets.get(key).cloned())
    }

    fn delete_secret(&self, key: &str) -> Result<()> {
        let mut secrets = self
            .secrets
            .lock()
            .map_err(|_| anyhow!("memory credential store lock poisoned"))?;
        secrets.remove(key);
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemCredentialStore;

impl SystemCredentialStore {
    fn entry(&self, key: &str) -> Result<keyring::Entry> {
        keyring::Entry::new(CREDENTIAL_SERVICE_NAME, key)
            .with_context(|| format!("failed to create credential store entry for key `{key}`"))
    }
}

impl CredentialStore for SystemCredentialStore {
    fn put_secret(&self, key: &str, value: &str) -> Result<()> {
        self.entry(key)?.set_password(value).with_context(|| {
            format!("failed to persist secret in system credential store for key `{key}`")
        })
    }

    fn get_secret(&self, key: &str) -> Result<Option<String>> {
        match self.entry(key)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(err).with_context(|| {
                format!("failed to read secret from system credential store for key `{key}`")
            }),
        }
    }

    fn delete_secret(&self, key: &str) -> Result<()> {
        match self.entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(err).with_context(|| {
                format!("failed to delete secret from system credential store for key `{key}`")
            }),
        }
    }
}
