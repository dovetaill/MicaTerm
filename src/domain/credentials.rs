//! Secure credential storage: OS keyring primary store with XChaCha20-Poly1305 encrypted file fallback.

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
use zeroize::{Zeroize, ZeroizeOnDrop};

#[cfg(any(target_os = "windows", target_os = "macos"))]
const SERVICE_NAME: &str = "mica-term-gpui";
const NONCE_LEN: usize = 24;
const KEY_LEN: usize = 32;
const KEY_FILE_NAME: &str = ".vault_master_key";
const AAD: &[u8] = b"mica-term-gpui/credential/v1";

pub fn host_secret_key(host_id: &str) -> String {
    format!("host/{host_id}")
}

pub fn keychain_secret_key(keychain_id: &str) -> String {
    format!("keychain/{keychain_id}")
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
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
        let mut json = serde_json::to_string(payload)?;
        let res = self.put_raw(key, &json);
        json.zeroize();
        res
    }

    fn get_payload(&self, key: &str) -> Result<Option<SecretPayload>> {
        let Some(mut raw) = self.get_raw(key)? else {
            return Ok(None);
        };
        let parsed = serde_json::from_str::<SecretPayload>(&raw);
        raw.zeroize();
        Ok(Some(parsed?))
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
        if let Some(mut old) = self.map.lock().remove(key) {
            old.zeroize();
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct SystemCredentialStore;

impl CredentialStore for SystemCredentialStore {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    fn put_raw(&self, key: &str, value: &str) -> Result<()> {
        let entry = keyring::Entry::new(SERVICE_NAME, key)?;
        entry.set_password(value)?;
        Ok(())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    fn put_raw(&self, _key: &str, _value: &str) -> Result<()> {
        // Never fall into `keyring::mock` (ephemeral RAM store); let `FallbackCredentialStore`
        // persist via `EncryptedFileCredentialStore` on targets without native OS keyring enabled.
        bail!("native OS keyring backend is not enabled on this target")
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    fn get_raw(&self, key: &str) -> Result<Option<String>> {
        let entry = keyring::Entry::new(SERVICE_NAME, key)?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    fn get_raw(&self, _key: &str) -> Result<Option<String>> {
        bail!("native OS keyring backend is not enabled on this target")
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    fn delete(&self, key: &str) -> Result<()> {
        let entry = keyring::Entry::new(SERVICE_NAME, key)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    fn delete(&self, _key: &str) -> Result<()> {
        Ok(())
    }
}

pub struct EncryptedFileCredentialStore {
    dir: PathBuf,
    legacy_key: [u8; KEY_LEN],
    cached_key: Mutex<Option<[u8; KEY_LEN]>>,
}

impl Drop for EncryptedFileCredentialStore {
    fn drop(&mut self) {
        self.legacy_key.zeroize();
        if let Some(mut k) = self.cached_key.lock().take() {
            k.zeroize();
        }
    }
}

impl EncryptedFileCredentialStore {
    pub fn new(dir: impl AsRef<Path>) -> Self {
        let dir = dir.as_ref().to_path_buf();
        let mut hasher = Sha256::new();
        hasher.update(b"mica-term-gpui/local-vault-key/v1:");
        hasher.update(dir.to_string_lossy().as_bytes());
        let digest = hasher.finalize();
        let mut legacy_key = [0u8; KEY_LEN];
        legacy_key.copy_from_slice(&digest);
        Self {
            dir,
            legacy_key,
            cached_key: Mutex::new(None),
        }
    }

    fn master_key(&self, create_if_missing: bool) -> Result<Option<[u8; KEY_LEN]>> {
        if let Some(k) = *self.cached_key.lock() {
            return Ok(Some(k));
        }
        let key_path = self.dir.join(KEY_FILE_NAME);
        if key_path.exists() {
            let raw = fs::read(&key_path)?;
            if raw.len() == KEY_LEN {
                let mut k = [0u8; KEY_LEN];
                k.copy_from_slice(&raw);
                *self.cached_key.lock() = Some(k);
                return Ok(Some(k));
            }
        }
        if !create_if_missing {
            return Ok(None);
        }
        fs::create_dir_all(&self.dir)
            .with_context(|| format!("failed to create secrets dir `{}`", self.dir.display()))?;
        let mut k = [0u8; KEY_LEN];
        OsRng.fill_bytes(&mut k);
        fs::write(&key_path, k)?;
        restrict_file_permissions(&key_path);
        *self.cached_key.lock() = Some(k);
        Ok(Some(k))
    }

    fn file_path(&self, key: &str) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        let hex = format!("{:x}.bin", hasher.finalize());
        self.dir.join(hex)
    }
}

fn restrict_file_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

impl CredentialStore for EncryptedFileCredentialStore {
    fn put_raw(&self, key: &str, value: &str) -> Result<()> {
        fs::create_dir_all(&self.dir)
            .with_context(|| format!("failed to create secrets dir `{}`", self.dir.display()))?;
        let mut master = self
            .master_key(true)?
            .expect("master key created when create_if_missing is true");
        let cipher = XChaCha20Poly1305::new((&master).into());
        master.zeroize();
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
        restrict_file_permissions(&path);
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

        if let Some(mut master) = self.master_key(false)? {
            let cipher = XChaCha20Poly1305::new((&master).into());
            master.zeroize();
            if let Ok(plaintext) = cipher.decrypt(
                XNonce::from_slice(nonce),
                Payload {
                    msg: ciphertext,
                    aad: AAD,
                },
            ) {
                return Ok(Some(String::from_utf8(plaintext)?));
            }
        }

        // Backward-compatible read for v0.2.0 deterministic key files
        let legacy_cipher = XChaCha20Poly1305::new((&self.legacy_key).into());
        let plaintext = legacy_cipher
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

impl Drop for FallbackCredentialStore {
    fn drop(&mut self) {
        for v in self.cache.lock().values_mut() {
            v.zeroize();
        }
    }
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
        // Always persist to the local XChaCha20-Poly1305 encrypted vault so credentials survive
        // portable folder moves, OS credential blob size limits (e.g. Win32 2560-byte cap), and
        // targets without a native keyring daemon. Also mirror best-effort to OS keyring; if OS
        // keyring rejects the update, purge any stale entry there so `get_raw` reads the fresh vault.
        self.fallback.put_raw(key, value)?;
        if self.primary.put_raw(key, value).is_err() {
            let _ = self.primary.delete(key);
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
        if let Some(mut old) = self.cache.lock().remove(key) {
            old.zeroize();
        }
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

    struct FailingStore;
    impl CredentialStore for FailingStore {
        fn put_raw(&self, _key: &str, _value: &str) -> Result<()> {
            bail!("keyring unavailable")
        }
        fn get_raw(&self, _key: &str) -> Result<Option<String>> {
            bail!("keyring unavailable")
        }
        fn delete(&self, _key: &str) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn encrypted_file_store_roundtrips_secrets() {
        let dir =
            std::env::temp_dir().join(format!("mica-gpui-cred-test-{}", uuid::Uuid::new_v4()));
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

    #[test]
    fn fallback_mirrors_to_vault_and_recovers_when_primary_fails() {
        let primary = Arc::new(MemoryCredentialStore::default());
        let fallback_mem = Arc::new(MemoryCredentialStore::default());
        let combined = FallbackCredentialStore::new(primary.clone(), fallback_mem.clone());

        combined.put_raw("host/a", "sec1").unwrap();
        assert_eq!(primary.get_raw("host/a").unwrap().as_deref(), Some("sec1"));
        assert_eq!(
            fallback_mem.get_raw("host/a").unwrap().as_deref(),
            Some("sec1")
        );

        let combined_fail =
            FallbackCredentialStore::new(Arc::new(FailingStore), fallback_mem.clone());
        combined_fail.put_raw("host/b", "sec2").unwrap();
        assert_eq!(
            fallback_mem.get_raw("host/b").unwrap().as_deref(),
            Some("sec2")
        );
    }

    #[test]
    fn default_credential_store_persists_password_across_reopen() {
        let dir = std::env::temp_dir().join(format!(
            "mica-gpui-default-cred-reopen-{}",
            uuid::Uuid::new_v4()
        ));
        let key = host_secret_key(&format!("test-host-{}", uuid::Uuid::new_v4()));
        let payload = SecretPayload {
            password: Some("PersistAcrossRestart#2026".into()),
            private_key_pem: None,
            passphrase: None,
            proxy_password: Some("px-secret".into()),
        };

        {
            let store_first = build_default_credential_store(&dir);
            store_first.put_payload(&key, &payload).unwrap();
        }

        {
            let store_reopened = build_default_credential_store(&dir);
            let loaded = store_reopened
                .get_payload(&key)
                .unwrap()
                .expect("saved SSH password should persist across store reopen");
            assert_eq!(loaded, payload);
            store_reopened.delete(&key).unwrap();
            assert!(store_reopened.get_payload(&key).unwrap().is_none());
        }

        let _ = fs::remove_dir_all(&dir);
    }
}
