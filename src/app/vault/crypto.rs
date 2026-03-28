use anyhow::{Context, Result, anyhow};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, Payload},
};
use rand_core::{OsRng, RngCore};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::app::vault::model::{CipherKind, CompressionKind, KdfConfig, VaultSnapshot};

const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 24;
const ZSTD_LEVEL: i32 = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WrappedVaultKey {
    pub kdf: KdfConfig,
    pub cipher: CipherKind,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedSnapshot {
    pub cipher: CipherKind,
    pub compression: CompressionKind,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub plaintext_len: usize,
    pub compressed_len: usize,
    pub payload_sha256: String,
}

pub fn generate_vault_key() -> [u8; KEY_LEN] {
    let mut key = [0u8; KEY_LEN];
    OsRng.fill_bytes(&mut key);
    key
}

pub fn wrap_vault_key(
    password: &SecretString,
    kdf: &KdfConfig,
    vault_key: &[u8; KEY_LEN],
) -> Result<WrappedVaultKey> {
    let kek = derive_kek(password, kdf)?;
    let (nonce, ciphertext) = encrypt_bytes(vault_key, &kek)?;

    Ok(WrappedVaultKey {
        kdf: kdf.clone(),
        cipher: CipherKind::XChaCha20Poly1305,
        nonce,
        ciphertext,
    })
}

pub fn unwrap_vault_key(
    password: &SecretString,
    wrapped: &WrappedVaultKey,
) -> Result<[u8; KEY_LEN]> {
    let kek = derive_kek(password, &wrapped.kdf)?;
    let plaintext = decrypt_bytes(&wrapped.nonce, &wrapped.ciphertext, &kek)
        .context("failed to unwrap vault key with provided password")?;

    plaintext
        .try_into()
        .map_err(|_| anyhow!("wrapped vault key length mismatch"))
}

pub fn encrypt_snapshot(snapshot: &VaultSnapshot, vault_key: &[u8; KEY_LEN]) -> Result<EncryptedSnapshot> {
    let serialized = serde_json::to_vec(snapshot).context("failed to serialize vault snapshot")?;
    let plaintext_len = serialized.len();
    let compressed =
        zstd::stream::encode_all(serialized.as_slice(), ZSTD_LEVEL).context("failed to compress vault snapshot")?;
    let compressed_len = compressed.len();
    let payload_sha256 = sha256_hex(compressed.as_slice());
    let (nonce, ciphertext) = encrypt_bytes(compressed.as_slice(), vault_key)?;

    Ok(EncryptedSnapshot {
        cipher: CipherKind::XChaCha20Poly1305,
        compression: CompressionKind::Zstd,
        nonce,
        ciphertext,
        plaintext_len,
        compressed_len,
        payload_sha256,
    })
}

pub fn decrypt_snapshot(
    encrypted: &EncryptedSnapshot,
    vault_key: &[u8; KEY_LEN],
) -> Result<VaultSnapshot> {
    let compressed = decrypt_bytes(&encrypted.nonce, &encrypted.ciphertext, vault_key)
        .context("failed to decrypt vault snapshot")?;
    let serialized =
        zstd::stream::decode_all(compressed.as_slice()).context("failed to decompress vault snapshot")?;
    serde_json::from_slice(serialized.as_slice()).context("failed to decode vault snapshot")
}

fn derive_kek(password: &SecretString, kdf: &KdfConfig) -> Result<Zeroizing<[u8; KEY_LEN]>> {
    let KdfConfig::Argon2id {
        memory_cost_kib,
        time_cost,
        parallelism,
        salt_b64,
    } = kdf;

    let params = Params::new(*memory_cost_kib, *time_cost, *parallelism, Some(KEY_LEN))
        .map_err(|err| anyhow!("invalid Argon2id parameters: {err}"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut derived = Zeroizing::new([0u8; KEY_LEN]);

    argon2
        .hash_password_into(password.expose_secret().as_bytes(), salt_b64.as_bytes(), &mut *derived)
        .map_err(|err| anyhow!("failed to derive vault KEK: {err}"))?;

    Ok(derived)
}

fn encrypt_bytes(plaintext: &[u8], key: &[u8; KEY_LEN]) -> Result<(Vec<u8>, Vec<u8>)> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).context("invalid XChaCha20-Poly1305 key")?;
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    let nonce_ref = XNonce::from_slice(&nonce);
    let ciphertext = cipher
        .encrypt(
            nonce_ref,
            Payload {
                msg: plaintext,
                aad: b"mica-term-vault",
            },
        )
        .map_err(|_| anyhow!("failed to encrypt vault payload"))?;

    Ok((nonce.to_vec(), ciphertext))
}

fn decrypt_bytes(nonce: &[u8], ciphertext: &[u8], key: &[u8; KEY_LEN]) -> Result<Vec<u8>> {
    if nonce.len() != NONCE_LEN {
        return Err(anyhow!("invalid XChaCha20-Poly1305 nonce length"));
    }

    let cipher = XChaCha20Poly1305::new_from_slice(key).context("invalid XChaCha20-Poly1305 key")?;
    let nonce_ref = XNonce::from_slice(nonce);

    cipher
        .decrypt(
            nonce_ref,
            Payload {
                msg: ciphertext,
                aad: b"mica-term-vault",
            },
        )
        .map_err(|_| anyhow!("failed to decrypt vault payload"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}
