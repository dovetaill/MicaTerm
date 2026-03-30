use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow, ensure};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, Payload},
};
use rand_core::{OsRng, RngCore};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::app::ssh::credentials::CredentialStore;
use crate::app::vault::model::{BootstrapBundle, CipherKind, KdfConfig};

const BOOTSTRAP_FORMAT_VERSION: u32 = 1;
const BOOTSTRAP_KEY_LEN: usize = 32;
const BOOTSTRAP_NONCE_LEN: usize = 24;
const BOOTSTRAP_SALT_LEN: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedBootstrapBundle {
    pub bundle: BootstrapBundle,
    pub provider_credentials: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalVaultBootstrapState {
    pub bundle: BootstrapBundle,
    pub wrapped_vault_key: String,
    pub kdf: KdfConfig,
    pub current_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct BootstrapExportPayload {
    bundle: BootstrapBundle,
    #[serde(default)]
    provider_credentials: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct EncryptedBootstrapExport {
    format_version: u32,
    cipher: CipherKind,
    kdf: KdfConfig,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
}

pub fn bootstrap_provider_credential_ref(remote_id: &str) -> String {
    format!("vault/bootstrap/{remote_id}")
}

pub fn persist_provider_credential(
    store: &dyn CredentialStore,
    credential_ref: &str,
    secret: Option<&str>,
) -> Result<()> {
    match secret.filter(|value| !value.trim().is_empty()) {
        Some(secret) => store.put_secret(credential_ref, secret),
        None => store.delete_secret(credential_ref),
    }
}

pub fn load_provider_credential(
    store: &dyn CredentialStore,
    credential_ref: Option<&str>,
) -> Result<Option<String>> {
    let Some(credential_ref) = credential_ref else {
        return Ok(None);
    };

    Ok(store
        .get_secret(credential_ref)?
        .filter(|value| !value.trim().is_empty()))
}

pub fn restore_provider_credentials(
    store: &dyn CredentialStore,
    imported: &ImportedBootstrapBundle,
) -> Result<()> {
    for remote in &imported.bundle.remotes {
        let Some(credential_ref) = remote.credential_ref.as_deref() else {
            continue;
        };
        persist_provider_credential(
            store,
            credential_ref,
            imported.provider_credentials.get(credential_ref).map(String::as_str),
        )?;
    }

    Ok(())
}

pub fn export_bootstrap_bundle(
    path: &Path,
    bundle: &BootstrapBundle,
    store: &dyn CredentialStore,
    password: &SecretString,
) -> Result<()> {
    validate_bootstrap_bundle(bundle)?;

    let payload = BootstrapExportPayload {
        bundle: bundle.clone(),
        provider_credentials: collect_provider_credentials(bundle, store)?,
    };
    let serialized =
        serde_json::to_vec(&payload).context("failed to encode bootstrap export payload")?;
    let kdf = generate_bootstrap_kdf();
    let key = derive_bootstrap_key(password, &kdf)?;
    let (nonce, ciphertext) = encrypt_bootstrap_bytes(serialized.as_slice(), &key)?;
    let export = EncryptedBootstrapExport {
        format_version: BOOTSTRAP_FORMAT_VERSION,
        cipher: CipherKind::XChaCha20Poly1305,
        kdf,
        nonce,
        ciphertext,
    };
    let encoded =
        bincode::serialize(&export).context("failed to encode encrypted bootstrap export")?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create bootstrap export directory `{}`",
                parent.display()
            )
        })?;
    }
    fs::write(path, encoded).with_context(|| {
        format!(
            "failed to persist encrypted bootstrap export `{}`",
            path.display()
        )
    })?;

    Ok(())
}

pub fn import_bootstrap_bundle(
    path: &Path,
    password: &SecretString,
) -> Result<ImportedBootstrapBundle> {
    let encoded = fs::read(path).with_context(|| {
        format!(
            "failed to read encrypted bootstrap export `{}`",
            path.display()
        )
    })?;
    let export: EncryptedBootstrapExport =
        bincode::deserialize(encoded.as_slice()).context("failed to decode bootstrap export")?;
    let key = derive_bootstrap_key(password, &export.kdf)?;
    let plaintext = decrypt_bootstrap_bytes(&export.nonce, &export.ciphertext, &key)?;
    let payload: BootstrapExportPayload =
        serde_json::from_slice(plaintext.as_slice()).context("failed to decode bootstrap payload")?;
    validate_bootstrap_bundle(&payload.bundle)?;

    Ok(ImportedBootstrapBundle {
        bundle: payload.bundle,
        provider_credentials: payload.provider_credentials,
    })
}

pub fn save_local_vault_bootstrap_state(
    path: &Path,
    state: &LocalVaultBootstrapState,
) -> Result<()> {
    ensure!(
        !state.bundle.vault_id.trim().is_empty(),
        "local vault bootstrap state requires a non-empty vault_id"
    );
    ensure!(
        !state.wrapped_vault_key.trim().is_empty(),
        "local vault bootstrap state requires a wrapped_vault_key"
    );

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create local vault bootstrap directory `{}`",
                parent.display()
            )
        })?;
    }

    let encoded =
        serde_json::to_vec_pretty(state).context("failed to encode local vault bootstrap state")?;
    fs::write(path, encoded).with_context(|| {
        format!(
            "failed to persist local vault bootstrap state `{}`",
            path.display()
        )
    })?;
    Ok(())
}

pub fn load_local_vault_bootstrap_state(path: &Path) -> Result<Option<LocalVaultBootstrapState>> {
    if !path.exists() {
        return Ok(None);
    }

    let encoded = fs::read(path).with_context(|| {
        format!(
            "failed to read local vault bootstrap state `{}`",
            path.display()
        )
    })?;
    let state: LocalVaultBootstrapState = serde_json::from_slice(encoded.as_slice())
        .context("failed to decode local vault bootstrap state")?;
    ensure!(
        !state.bundle.vault_id.trim().is_empty(),
        "local vault bootstrap state requires a non-empty vault_id"
    );
    Ok(Some(state))
}

pub fn validate_bootstrap_bundle(bundle: &BootstrapBundle) -> Result<()> {
    ensure!(
        !bundle.vault_id.trim().is_empty(),
        "bootstrap bundle requires a non-empty vault_id"
    );
    ensure!(
        bundle.primary_remote().is_some(),
        "bootstrap bundle requires at least one primary remote"
    );

    for remote in &bundle.remotes {
        ensure!(
            !remote.remote_id.trim().is_empty(),
            "bootstrap bundle requires every remote to have a non-empty remote_id"
        );
    }

    Ok(())
}

fn collect_provider_credentials(
    bundle: &BootstrapBundle,
    store: &dyn CredentialStore,
) -> Result<BTreeMap<String, String>> {
    let mut provider_credentials = BTreeMap::new();
    for remote in &bundle.remotes {
        let Some(credential_ref) = remote.credential_ref.as_deref() else {
            continue;
        };
        if let Some(secret) = load_provider_credential(store, Some(credential_ref))? {
            provider_credentials.insert(credential_ref.to_string(), secret);
        }
    }

    Ok(provider_credentials)
}

fn generate_bootstrap_kdf() -> KdfConfig {
    let mut salt = [0u8; BOOTSTRAP_SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    KdfConfig::Argon2id {
        memory_cost_kib: 19_456,
        time_cost: 2,
        parallelism: 1,
        salt_b64: encode_hex(salt.as_slice()),
    }
}

fn derive_bootstrap_key(
    password: &SecretString,
    kdf: &KdfConfig,
) -> Result<Zeroizing<[u8; BOOTSTRAP_KEY_LEN]>> {
    let KdfConfig::Argon2id {
        memory_cost_kib,
        time_cost,
        parallelism,
        salt_b64,
    } = kdf;

    let params = Params::new(
        *memory_cost_kib,
        *time_cost,
        *parallelism,
        Some(BOOTSTRAP_KEY_LEN),
    )
    .map_err(|err| anyhow!("invalid bootstrap Argon2id parameters: {err}"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut derived = Zeroizing::new([0u8; BOOTSTRAP_KEY_LEN]);

    argon2
        .hash_password_into(
            password.expose_secret().as_bytes(),
            salt_b64.as_bytes(),
            &mut *derived,
        )
        .map_err(|err| anyhow!("failed to derive bootstrap encryption key: {err}"))?;

    Ok(derived)
}

fn encrypt_bootstrap_bytes(
    plaintext: &[u8],
    key: &[u8; BOOTSTRAP_KEY_LEN],
) -> Result<(Vec<u8>, Vec<u8>)> {
    let cipher =
        XChaCha20Poly1305::new_from_slice(key).context("invalid bootstrap encryption key")?;
    let mut nonce = [0u8; BOOTSTRAP_NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: b"mica-term-bootstrap",
            },
        )
        .map_err(|_| anyhow!("failed to encrypt bootstrap payload"))?;

    Ok((nonce.to_vec(), ciphertext))
}

fn decrypt_bootstrap_bytes(
    nonce: &[u8],
    ciphertext: &[u8],
    key: &[u8; BOOTSTRAP_KEY_LEN],
) -> Result<Vec<u8>> {
    ensure!(
        nonce.len() == BOOTSTRAP_NONCE_LEN,
        "invalid bootstrap nonce length"
    );

    let cipher =
        XChaCha20Poly1305::new_from_slice(key).context("invalid bootstrap encryption key")?;
    cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: b"mica-term-bootstrap",
            },
        )
        .map_err(|_| anyhow!("failed to decrypt bootstrap payload"))
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}
