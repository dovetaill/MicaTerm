//! Secure credential storage: OS keyring with XChaCha20-Poly1305 encrypted file fallback.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, Payload},
};
use parking_lot::Mutex;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const SERVICE_NAME: &str = "mica-term-gpui";
const NONCE_LEN: usize = 24;
const AAD: &[u8] = b"mica-term-gpui/credential/v1";

pub fn host_secret_key(host_id: &str) -> String {
    format!("host/{host_id}")
}

pub fn keychain_secret_key(keychain_id: &str) -> String {
    format!("keychain/{keychain_id}")
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretPayload {
    pub password: Option<String>,
    pub private_key_pem: Option<String>,
    pub passphrase: Option<String>,
    pub proxy_password: Option<String>,
}

impl SecretPayload {
    pub fn is_empty(&self) -> bool {
        self.password.as_deref().is_none_or(|s| s.is_empty())
            && self
                .private_key_pem
                .as_deref()
                .is_none_or(|s| s.trim().is_empty())
            && self.passphrase.as_deref().is_none_or(|s| s.is_empty())
            && self.proxy_password.as_deref().is_none_or(|s| s.is_empty())
    }
}

pub trait CredentialStore: Send + Sync {
    fn put_raw(&self, key: &str, value: &str) -> Result<()>;
    fn get_raw(&self, key: &str) -> Result<Option<String>>;
    fn delete(&self, key: &str) -> Result<()>;

    fn put_payload(&self, key: &str, payload: &SecretPayload) -> Result<()> {
        if payload.is_empty() {
            return self.delete(key);
        }
        let json = serde_json::to_string(payload)?;
        self.put_raw(key, &json)
    }

    fn get_payload(&self, key: &str) -> Result<Option<SecretPayload>> {
        let Some(raw) = self.get_raw(key)? else {
            return Ok(None);
        };
        let parsed = serde_json::from_str::<SecretPayload>(&raw)?;
        Ok(Some(parsed))
    }
}

#[derive(Default)]
pub struct MemoryCredentialStore {
    map: Mutex<HashMap<String, String>>,
}

impl CredentialStore for MemoryCredentialStore {
    fn put_raw(&self, key: &str, value: &str) -> Result<()> {
        self.map.lock().insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn get_raw(&self, key: &str) -> Result<Option<String>> {
        Ok(self.map.lock().get(key).cloned())
    }

    fn delete(&self, key: &str) -> Result<()> {
        self.map.lock().remove(key);
        Ok(())
    }
}

#[derive(Default)]
pub struct SystemCredentialStore;

impl CredentialStore for SystemCredentialStore {
    fn put_raw(&self, key: &str, value: &str) -> Result<()> {
        let entry = keyring::Entry::new(SERVICE_NAME, key)?;
        entry.set_password(value)?;
        Ok(())
    }

    fn get_raw(&self, key: &str) -> Result<Option<String>> {
        let entry = keyring::Entry::new(SERVICE_NAME, key)?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    fn delete(&self, key: &str) -> Result<()> {
        let entry = keyring::Entry::new(SERVICE_NAME, key)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(err.into()),
        }
    }
}

pub struct EncryptedFileCredentialStore {
    dir: PathBuf,
    key: [u8; 32],
}

impl EncryptedFileCredentialStore {
    pub fn new(dir: impl AsRef<Path>) -> Self {
        let dir = dir.as_ref().to_path_buf();
        let mut hasher = Sha256::new();
        hasher.update(b"mica-term-gpui/local-vault-key/v1:");
        hasher.update(dir.to_string_lossy().as_bytes());
        let digest = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&digest);
        Self { dir, key }
    }

    fn file_path(&self, key: &str) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        let hex = format!("{:x}.bin", hasher.finalize());
        self.dir.join(hex)
    }
}

impl CredentialStore for EncryptedFileCredentialStore {
    fn put_raw(&self, key: &str, value: &str) -> Result<()> {
        fs::create_dir_all(&self.dir)
            .with_context(|| format!("failed to create secrets dir `{}`", self.dir.display()))?;
        let cipher = XChaCha20Poly1305::new((&self.key).into());
        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce_bytes),
                Payload {
                    msg: value.as_bytes(),
                    aad: AAD,
                },
            )
            .map_err(|_| anyhow::anyhow!("failed to encrypt credential"))?;
        let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&ciphertext);
        let path = self.file_path(key);
        fs::write(&path, out)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    fn get_raw(&self, key: &str) -> Result<Option<String>> {
        let path = self.file_path(key);
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err.into()),
        };
        if bytes.len() <= NONCE_LEN {
            bail!("corrupted encrypted credential file");
        }
        let (nonce, ciphertext) = bytes.split_at(NONCE_LEN);
        let cipher = XChaCha20Poly1305::new((&self.key).into());
        let plaintext = cipher
            .decrypt(
                XNonce::from_slice(nonce),
                Payload {
                    msg: ciphertext,
                    aad: AAD,
                },
            )
            .map_err(|_| anyhow::anyhow!("failed to decrypt credential"))?;
        Ok(Some(String::from_utf8(plaintext)?))
    }

    fn delete(&self, key: &str) -> Result<()> {
        let path = self.file_path(key);
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err.into()),
        }
    }
}

pub struct FallbackCredentialStore {
    primary: Arc<dyn CredentialStore>,
    fallback: Arc<dyn CredentialStore>,
    cache: Mutex<HashMap<String, String>>,
}

impl FallbackCredentialStore {
    pub fn new(primary: Arc<dyn CredentialStore>, fallback: Arc<dyn CredentialStore>) -> Self {
        Self {
            primary,
            fallback,
            cache: Mutex::new(HashMap::new()),
        }
    }
}

impl CredentialStore for FallbackCredentialStore {
    fn put_raw(&self, key: &str, value: &str) -> Result<()> {
        let primary_ok = self.primary.put_raw(key, value).is_ok();
        let fallback_res = self.fallback.put_raw(key, value);
        if !primary_ok {
            fallback_res?;
        }
        self.cache.lock().insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn get_raw(&self, key: &str) -> Result<Option<String>> {
        if let Some(cached) = self.cache.lock().get(key).cloned() {
            return Ok(Some(cached));
        }
        let found = match self.primary.get_raw(key) {
            Ok(Some(v)) => Some(v),
            _ => self.fallback.get_raw(key)?,
        };
        if let Some(ref val) = found {
            self.cache.lock().insert(key.to_string(), val.clone());
        }
        Ok(found)
    }

    fn delete(&self, key: &str) -> Result<()> {
        self.cache.lock().remove(key);
        let _ = self.primary.delete(key);
        let _ = self.fallback.delete(key);
        Ok(())
    }
}

pub fn build_default_credential_store(secrets_dir: impl AsRef<Path>) -> Arc<dyn CredentialStore> {
    Arc::new(FallbackCredentialStore::new(
        Arc::new(SystemCredentialStore),
        Arc::new(EncryptedFileCredentialStore::new(secrets_dir)),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_file_store_roundtrips_secrets() {
        let dir = std::env::temp_dir().join(format!("mica-gpui-cred-test-{}", uuid::Uuid::new_v4()));
        let store = EncryptedFileCredentialStore::new(&dir);
        let payload = SecretPayload {
            password: Some("p@ssw0rd!".into()),
            private_key_pem: None,
            passphrase: Some("phrase".into()),
            proxy_password: None,
        };
        store.put_payload("host/1", &payload).unwrap();
        let loaded = store.get_payload("host/1").unwrap().unwrap();
        assert_eq!(loaded, payload);
        store.delete("host/1").unwrap();
        assert!(store.get_payload("host/1").unwrap().is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
