use anyhow::{Context, bail};
use russh::keys::{HashAlg, PrivateKey, PublicKey};

use crate::app::keychain::model::{KeychainCatalog, KeychainIdentityAuthKind, KeychainNodePayload};
use crate::app::ssh::credentials::{keychain_identity_credential_ref, keychain_key_credential_ref};
use crate::app::ssh::profile::ConnectionProfile;
use crate::shell::assets::{
    AssetSshConnectionSpec, SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY, SSH_AUTH_SOURCE_MANUAL,
    normalized_ssh_auth_source,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedSshKeyMaterial {
    pub algorithm: String,
    pub public_key: String,
    pub fingerprint: String,
    pub comment: String,
}

pub fn resolve_saved_ssh_profile(
    asset_id: &str,
    title: &str,
    spec: &AssetSshConnectionSpec,
    keychain_catalog: &KeychainCatalog,
) -> anyhow::Result<ConnectionProfile> {
    match normalized_ssh_auth_source(&spec.auth_source) {
        SSH_AUTH_SOURCE_MANUAL => ConnectionProfile::from_saved_asset(asset_id, title, spec),
        SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY => {
            let resolved_spec = resolve_identity_backed_spec(title, spec, keychain_catalog)?;
            ConnectionProfile::from_saved_asset(asset_id, title, &resolved_spec)
        }
        other => bail!("unsupported ssh auth source: {other}"),
    }
}

fn resolve_identity_backed_spec(
    title: &str,
    spec: &AssetSshConnectionSpec,
    keychain_catalog: &KeychainCatalog,
) -> anyhow::Result<AssetSshConnectionSpec> {
    let identity_id = spec
        .keychain_identity_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!("SSH asset `{title}` is missing keychain identity reference")
        })?;
    let identity_node = keychain_catalog.nodes.get(identity_id).ok_or_else(|| {
        anyhow::anyhow!(
            "keychain identity `{identity_id}` referenced by SSH asset `{title}` was not found"
        )
    })?;
    let identity = match &identity_node.payload {
        KeychainNodePayload::Identity(identity) => identity,
        _ => {
            bail!(
                "keychain identity `{identity_id}` referenced by SSH asset `{title}` was not found"
            );
        }
    };

    let username = identity.username.trim();
    if username.is_empty() {
        bail!("keychain identity `{identity_id}` is missing username");
    }

    let mut resolved_spec = spec.clone();
    resolved_spec.user = username.to_string();
    resolved_spec.auth_source = SSH_AUTH_SOURCE_MANUAL.into();
    resolved_spec.keychain_identity_id = None;
    resolved_spec.private_key_path.clear();

    match identity.auth_kind {
        KeychainIdentityAuthKind::Password => {
            resolved_spec.auth_method = "password".into();
            resolved_spec.private_key_source = "content".into();
            resolved_spec.credential_ref = Some(
                identity
                    .credential_ref
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| keychain_identity_credential_ref(identity_id)),
            );
        }
        KeychainIdentityAuthKind::SshKey => {
            let ssh_key_id = identity
                .ssh_key_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "keychain identity `{identity_id}` is missing SSH key reference"
                    )
                })?;
            let ssh_key_node = keychain_catalog.nodes.get(ssh_key_id).ok_or_else(|| {
                anyhow::anyhow!(
                    "keychain SSH key `{ssh_key_id}` referenced by identity `{identity_id}` was not found"
                )
            })?;
            let ssh_key = match &ssh_key_node.payload {
                KeychainNodePayload::SshKey(ssh_key) => ssh_key,
                _ => {
                    bail!(
                        "keychain SSH key `{ssh_key_id}` referenced by identity `{identity_id}` was not found"
                    );
                }
            };

            resolved_spec.auth_method = "private-key".into();
            resolved_spec.private_key_source = "content".into();
            resolved_spec.credential_ref = Some(
                ssh_key
                    .credential_ref
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| keychain_key_credential_ref(ssh_key_id)),
            );
        }
    }

    ConnectionProfile::from_saved_asset("__resolver_validation__", title, &resolved_spec)
        .with_context(|| format!("failed to normalize resolved SSH asset `{title}`"))?;

    Ok(resolved_spec)
}

pub fn derive_public_key_material_from_private_key(
    private_key: &str,
) -> anyhow::Result<DerivedSshKeyMaterial> {
    let private_key =
        PrivateKey::from_openssh(private_key).context("failed to parse SSH private key")?;
    let public_key = private_key.public_key();
    let public_key_openssh = public_key
        .to_openssh()
        .context("failed to encode SSH public key")?;
    Ok(derived_ssh_key_material_from_public_key(
        public_key,
        public_key_openssh.as_str(),
    ))
}

pub fn derive_public_key_material_from_public_key(
    public_key: &str,
) -> anyhow::Result<DerivedSshKeyMaterial> {
    let trimmed = public_key.trim();
    let public_key = PublicKey::from_openssh(trimmed).context("failed to parse SSH public key")?;
    Ok(derived_ssh_key_material_from_public_key(
        &public_key,
        trimmed,
    ))
}

fn derived_ssh_key_material_from_public_key(
    public_key: &PublicKey,
    openssh: &str,
) -> DerivedSshKeyMaterial {
    DerivedSshKeyMaterial {
        algorithm: public_key.algorithm().as_str().to_string(),
        public_key: openssh.trim().to_string(),
        fingerprint: public_key.fingerprint(HashAlg::Sha256).to_string(),
        comment: openssh
            .split_whitespace()
            .nth(2)
            .unwrap_or_default()
            .to_string(),
    }
}
