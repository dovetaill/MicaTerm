//! Credential storage adapters for SSH secrets.

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow};
use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, Payload},
};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::shell::view_model::AssetSshConnectionDraft;

const CREDENTIAL_SERVICE_NAME: &str = "mica-term";
const ENCRYPTED_CREDENTIAL_KEY_LEN: usize = 32;
const ENCRYPTED_CREDENTIAL_NONCE_LEN: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshCredentialKind {
    SavedSecrets,
}

pub fn ssh_credential_ref(asset_id: &str, kind: SshCredentialKind) -> String {
    match kind {
        SshCredentialKind::SavedSecrets => format!("ssh/saved-secrets/{asset_id}"),
    }
}

pub trait CredentialStore: Send + Sync {
    fn put_secret(&self, key: &str, value: &str) -> Result<()>;
    fn get_secret(&self, key: &str) -> Result<Option<String>>;
    fn delete_secret(&self, key: &str) -> Result<()>;
}

pub struct CachedCredentialStore {
    backing: Arc<dyn CredentialStore>,
    cache: Mutex<HashMap<String, String>>,
}

pub struct FallbackCredentialStore {
    primary: Arc<dyn CredentialStore>,
    fallback: Arc<dyn CredentialStore>,
}

impl CachedCredentialStore {
    pub fn new(backing: Arc<dyn CredentialStore>) -> Self {
        Self {
            backing,
            cache: Mutex::new(HashMap::new()),
        }
    }
}

impl FallbackCredentialStore {
    pub fn new(primary: Arc<dyn CredentialStore>, fallback: Arc<dyn CredentialStore>) -> Self {
        Self { primary, fallback }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredSshSecretBundle {
    pub password: Option<String>,
    pub private_key_content: Option<String>,
    pub passphrase: Option<String>,
    pub proxy_socks5_password: Option<String>,
}

impl StoredSshSecretBundle {
    pub fn is_empty(&self) -> bool {
        self.password
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
            && self
                .private_key_content
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            && self
                .passphrase
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            && self
                .proxy_socks5_password
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredSecretLookupError {
    MissingCredentialRef,
    MissingEntry {
        credential_ref: String,
    },
    ReadFailed {
        credential_ref: String,
        message: String,
    },
    EmptyBundleField {
        credential_ref: String,
        field: &'static str,
    },
}

impl fmt::Display for StoredSecretLookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCredentialRef => write!(f, "missing SSH credential reference"),
            Self::MissingEntry { credential_ref } => {
                write!(f, "missing saved SSH secret entry `{credential_ref}`")
            }
            Self::ReadFailed {
                credential_ref,
                message,
            } => write!(
                f,
                "failed to read saved SSH secret entry `{credential_ref}`: {message}"
            ),
            Self::EmptyBundleField {
                credential_ref,
                field,
            } => write!(
                f,
                "saved SSH secret entry `{credential_ref}` is missing field `{field}`"
            ),
        }
    }
}

impl std::error::Error for StoredSecretLookupError {}

pub fn persist_secret_bundle(
    store: &dyn CredentialStore,
    credential_ref: &str,
    bundle: &StoredSshSecretBundle,
) -> Result<()> {
    if bundle.is_empty() {
        return store.delete_secret(credential_ref);
    }

    let payload = serde_json::to_string(bundle)
        .with_context(|| format!("failed to serialize SSH secret bundle `{credential_ref}`"))?;
    store.put_secret(credential_ref, payload.as_str())
}

pub fn load_secret_bundle(
    store: &dyn CredentialStore,
    credential_ref: &str,
) -> Result<StoredSshSecretBundle> {
    let raw = store
        .get_secret(credential_ref)
        .with_context(|| format!("failed to load SSH secret bundle `{credential_ref}`"))?;
    let Some(raw) = raw else {
        return Ok(StoredSshSecretBundle::default());
    };

    Ok(
        serde_json::from_str::<StoredSshSecretBundle>(&raw).unwrap_or_else(|_| {
            StoredSshSecretBundle {
                password: Some(raw),
                ..StoredSshSecretBundle::default()
            }
        }),
    )
}

pub fn snapshot_secret_bundle(
    store: &dyn CredentialStore,
    credential_ref: Option<&str>,
) -> Result<Option<StoredSshSecretBundle>> {
    let Some(credential_ref) = credential_ref else {
        return Ok(None);
    };

    let bundle = load_secret_bundle(store, credential_ref)?;
    if bundle.is_empty() {
        Ok(None)
    } else {
        Ok(Some(bundle))
    }
}

pub fn restore_snapshot_secret_bundle(
    store: &dyn CredentialStore,
    credential_ref: Option<&str>,
    bundle: Option<&StoredSshSecretBundle>,
) -> Result<()> {
    let Some(credential_ref) = credential_ref else {
        return Ok(());
    };

    match bundle {
        Some(bundle) if !bundle.is_empty() => persist_secret_bundle(store, credential_ref, bundle),
        _ => store.delete_secret(credential_ref),
    }
}

pub fn load_secret_bundle_with_diagnostics(
    store: &dyn CredentialStore,
    credential_ref: Option<&str>,
) -> std::result::Result<StoredSshSecretBundle, StoredSecretLookupError> {
    let credential_ref = credential_ref.ok_or(StoredSecretLookupError::MissingCredentialRef)?;
    let raw = store
        .get_secret(credential_ref)
        .map_err(|err| StoredSecretLookupError::ReadFailed {
            credential_ref: credential_ref.to_string(),
            message: err.to_string(),
        })?
        .ok_or_else(|| StoredSecretLookupError::MissingEntry {
            credential_ref: credential_ref.to_string(),
        })?;

    Ok(
        serde_json::from_str::<StoredSshSecretBundle>(&raw).unwrap_or_else(|_| {
            StoredSshSecretBundle {
                password: Some(raw),
                ..StoredSshSecretBundle::default()
            }
        }),
    )
}

pub fn required_secret_bundle_field(
    bundle: &StoredSshSecretBundle,
    credential_ref: &str,
    field: &'static str,
) -> std::result::Result<String, StoredSecretLookupError> {
    let value = match field {
        "password" => bundle.password.as_deref(),
        "private_key_content" => bundle.private_key_content.as_deref(),
        "passphrase" => bundle.passphrase.as_deref(),
        "proxy_socks5_password" => bundle.proxy_socks5_password.as_deref(),
        _ => None,
    };

    value
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| StoredSecretLookupError::EmptyBundleField {
            credential_ref: credential_ref.to_string(),
            field,
        })
}

pub fn merge_edit_bundle(
    _existing: StoredSshSecretBundle,
    draft: &AssetSshConnectionDraft,
) -> StoredSshSecretBundle {
    let mut bundle = match draft.auth_method.as_str() {
        "password" => StoredSshSecretBundle {
            password: non_empty_secret(&draft.password),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: None,
        },
        "private-key" if draft.private_key_source == "content" => StoredSshSecretBundle {
            password: None,
            private_key_content: non_empty_secret(&draft.private_key_content),
            passphrase: non_empty_secret(&draft.passphrase),
            proxy_socks5_password: None,
        },
        "private-key" if draft.private_key_source == "path" => StoredSshSecretBundle {
            password: None,
            private_key_content: None,
            passphrase: non_empty_secret(&draft.passphrase),
            proxy_socks5_password: None,
        },
        _ => StoredSshSecretBundle::default(),
    };
    bundle.proxy_socks5_password = if draft.proxy_type == "socks5" {
        non_empty_secret(&draft.proxy_socks5_password)
    } else {
        None
    };
    bundle
}

fn non_empty_secret(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.to_string())
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

impl CredentialStore for CachedCredentialStore {
    fn put_secret(&self, key: &str, value: &str) -> Result<()> {
        self.backing.put_secret(key, value)?;
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| anyhow!("cached credential store lock poisoned"))?;
        cache.insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn get_secret(&self, key: &str) -> Result<Option<String>> {
        {
            let cache = self
                .cache
                .lock()
                .map_err(|_| anyhow!("cached credential store lock poisoned"))?;
            if let Some(secret) = cache.get(key) {
                return Ok(Some(secret.clone()));
            }
        }

        let value = self.backing.get_secret(key)?;
        if let Some(secret) = value.as_ref() {
            let mut cache = self
                .cache
                .lock()
                .map_err(|_| anyhow!("cached credential store lock poisoned"))?;
            cache.insert(key.to_string(), secret.clone());
        }
        Ok(value)
    }

    fn delete_secret(&self, key: &str) -> Result<()> {
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| anyhow!("cached credential store lock poisoned"))?;
        cache.remove(key);
        drop(cache);
        self.backing.delete_secret(key)
    }
}

#[derive(Debug, Clone)]
pub struct FileCredentialStore {
    root_dir: PathBuf,
}

impl FileCredentialStore {
    pub fn new(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }

    fn secret_path(&self, key: &str) -> Result<PathBuf> {
        credential_secret_path(&self.root_dir, key, "json")
    }
}

impl CredentialStore for FileCredentialStore {
    fn put_secret(&self, key: &str, value: &str) -> Result<()> {
        let path = self.secret_path(key)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create credential store directory `{}`",
                    parent.display()
                )
            })?;
        }
        fs::write(&path, value).with_context(|| {
            format!(
                "failed to persist secret in file credential store for key `{key}` at `{}`",
                path.display()
            )
        })
    }

    fn get_secret(&self, key: &str) -> Result<Option<String>> {
        let path = self.secret_path(key)?;
        match fs::read_to_string(&path) {
            Ok(secret) => Ok(Some(secret)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err).with_context(|| {
                format!(
                    "failed to read secret from file credential store for key `{key}` at `{}`",
                    path.display()
                )
            }),
        }
    }

    fn delete_secret(&self, key: &str) -> Result<()> {
        let path = self.secret_path(key)?;
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err).with_context(|| {
                format!(
                    "failed to delete secret from file credential store for key `{key}` at `{}`",
                    path.display()
                )
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EncryptedFileCredentialStore {
    root_dir: PathBuf,
}

impl EncryptedFileCredentialStore {
    pub fn new(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }

    fn secret_path(&self, key: &str) -> Result<PathBuf> {
        credential_secret_path(&self.root_dir, key, "bin")
    }

    fn encryption_key(&self) -> [u8; ENCRYPTED_CREDENTIAL_KEY_LEN] {
        derive_local_encryption_key(&self.root_dir)
    }
}

impl CredentialStore for EncryptedFileCredentialStore {
    fn put_secret(&self, key: &str, value: &str) -> Result<()> {
        let path = self.secret_path(key)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create encrypted credential store directory `{}`",
                    parent.display()
                )
            })?;
        }

        let encoded = encrypt_local_credential_bytes(value.as_bytes(), &self.encryption_key())
            .context("failed to encrypt local credential value")?;
        fs::write(&path, encoded).with_context(|| {
            format!(
                "failed to persist secret in encrypted file credential store for key `{key}` at `{}`",
                path.display()
            )
        })
    }

    fn get_secret(&self, key: &str) -> Result<Option<String>> {
        let path = self.secret_path(key)?;
        let encoded = match fs::read(&path) {
            Ok(encoded) => encoded,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => {
                return Err(err).with_context(|| {
                    format!(
                        "failed to read secret from encrypted file credential store for key `{key}` at `{}`",
                        path.display()
                    )
                });
            }
        };

        let plaintext = decrypt_local_credential_bytes(encoded.as_slice(), &self.encryption_key())
            .context("failed to decrypt local credential value")?;
        let secret = String::from_utf8(plaintext)
            .with_context(|| format!("credential entry `{key}` is not valid UTF-8"))?;

        Ok(Some(secret))
    }

    fn delete_secret(&self, key: &str) -> Result<()> {
        let path = self.secret_path(key)?;
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err).with_context(|| {
                format!(
                    "failed to delete secret from encrypted file credential store for key `{key}` at `{}`",
                    path.display()
                )
            }),
        }
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

impl CredentialStore for FallbackCredentialStore {
    fn put_secret(&self, key: &str, value: &str) -> Result<()> {
        match self.primary.put_secret(key, value) {
            Ok(()) => Ok(()),
            Err(primary_err) => self.fallback.put_secret(key, value).with_context(|| {
                format!("primary credential store failed for key `{key}`: {primary_err}")
            }),
        }
    }

    fn get_secret(&self, key: &str) -> Result<Option<String>> {
        match self.primary.get_secret(key) {
            Ok(Some(secret)) => Ok(Some(secret)),
            Ok(None) | Err(_) => self.fallback.get_secret(key),
        }
    }

    fn delete_secret(&self, key: &str) -> Result<()> {
        let primary_result = self.primary.delete_secret(key);
        let fallback_result = self.fallback.delete_secret(key);

        match (primary_result, fallback_result) {
            (Ok(()), Ok(())) | (Ok(()), Err(_)) | (Err(_), Ok(())) => Ok(()),
            (Err(primary_err), Err(fallback_err)) => Err(fallback_err).with_context(|| {
                format!("primary credential store failed to delete key `{key}`: {primary_err}")
            }),
        }
    }
}

fn credential_secret_path(root_dir: &Path, key: &str, extension: &str) -> Result<PathBuf> {
    let key_path = Path::new(key);
    if key_path.is_absolute() {
        return Err(anyhow!("credential key `{key}` must be relative"));
    }

    let mut path = root_dir.to_path_buf();
    for component in key_path.components() {
        match component {
            Component::Normal(segment) => path.push(segment),
            _ => {
                return Err(anyhow!(
                    "credential key `{key}` contains an invalid path component"
                ));
            }
        }
    }

    path.set_extension(extension);
    Ok(path)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct EncryptedCredentialRecord {
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
}

fn derive_local_encryption_key(root_dir: &Path) -> [u8; ENCRYPTED_CREDENTIAL_KEY_LEN] {
    let mut hasher = Sha256::new();
    hasher.update(CREDENTIAL_SERVICE_NAME.as_bytes());
    hasher.update(root_dir.to_string_lossy().as_bytes());

    if let Some(user) = std::env::var_os("USER").or_else(|| std::env::var_os("USERNAME")) {
        hasher.update(user.to_string_lossy().as_bytes());
    }
    if let Some(host) = std::env::var_os("HOSTNAME").or_else(|| std::env::var_os("COMPUTERNAME"))
    {
        hasher.update(host.to_string_lossy().as_bytes());
    }

    let digest = hasher.finalize();
    digest.into()
}

fn encrypt_local_credential_bytes(
    plaintext: &[u8],
    key: &[u8; ENCRYPTED_CREDENTIAL_KEY_LEN],
) -> Result<Vec<u8>> {
    let cipher =
        XChaCha20Poly1305::new_from_slice(key).context("invalid encrypted credential key")?;
    let mut nonce = [0u8; ENCRYPTED_CREDENTIAL_NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: b"mica-term-credential-cache",
            },
        )
        .map_err(|_| anyhow!("failed to encrypt credential payload"))?;

    bincode::serialize(&EncryptedCredentialRecord {
        nonce: nonce.to_vec(),
        ciphertext,
    })
    .context("failed to encode encrypted credential payload")
}

fn decrypt_local_credential_bytes(
    encoded: &[u8],
    key: &[u8; ENCRYPTED_CREDENTIAL_KEY_LEN],
) -> Result<Vec<u8>> {
    let record: EncryptedCredentialRecord =
        bincode::deserialize(encoded).context("failed to decode encrypted credential payload")?;
    if record.nonce.len() != ENCRYPTED_CREDENTIAL_NONCE_LEN {
        return Err(anyhow!("invalid encrypted credential nonce length"));
    }

    let cipher =
        XChaCha20Poly1305::new_from_slice(key).context("invalid encrypted credential key")?;
    cipher
        .decrypt(
            XNonce::from_slice(record.nonce.as_slice()),
            Payload {
                msg: record.ciphertext.as_slice(),
                aad: b"mica-term-credential-cache",
            },
        )
        .map_err(|_| anyhow!("failed to decrypt credential payload"))
}
