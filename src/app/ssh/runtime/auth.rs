//! SSH runtime authentication, host-key verification, and progress helpers.

use std::{error::Error as StdError, fmt, sync::Arc};

use anyhow::{Context, Result, anyhow, bail};
use russh::client;
use russh::client::AuthResult;
use russh::keys::{self, PrivateKeyWithHashAlg};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::app::ssh::connection_progress::{
    ConnectionHeadlineState, ConnectionProgressEvent, ConnectionStepState, ConnectionStepStateItem,
};
use crate::app::ssh::credentials::{
    CredentialStore, StoredSecretLookupError, StoredSshSecretBundle,
    load_secret_bundle_with_diagnostics, required_secret_bundle_field,
};
use crate::app::ssh::known_hosts::{KnownHostCheck, KnownHostsService};
use crate::app::ssh::profile::{ConnectionProfile, SshAuthMethod};

use super::SessionRuntimeEvent;

pub(super) struct ConnectionProgressReporter {
    attempt_id: Uuid,
    next_step_index: usize,
    event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
}

pub(super) struct ConnectionProgressStep {
    attempt_id: Uuid,
    event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    step_id: String,
    step_kind: String,
    title: String,
    hop_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownHostKeyError {
    pub host: String,
    pub port: u16,
    pub fingerprint: String,
    pub public_key_openssh: String,
}

impl fmt::Display for UnknownHostKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown SSH host key for `{}`:{} ({})",
            self.host, self.port, self.fingerprint
        )
    }
}

impl StdError for UnknownHostKeyError {}

pub(super) struct RuntimeClientHandler {
    pub(super) host: String,
    pub(super) port: u16,
    pub(super) known_hosts: KnownHostsService,
}

impl ConnectionProgressReporter {
    pub(super) fn new(
        attempt_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
        headline: ConnectionHeadlineState,
    ) -> Self {
        let reporter = Self {
            attempt_id,
            next_step_index: 0,
            event_tx,
        };
        let _ = reporter
            .event_tx
            .send(SessionRuntimeEvent::ConnectionProgress(
                ConnectionProgressEvent::AttemptStarted {
                    attempt_id,
                    headline,
                },
            ));
        reporter
    }

    pub(super) fn start_step(
        &mut self,
        step_kind: &str,
        title: impl Into<String>,
        detail: impl Into<String>,
        hop_label: impl Into<String>,
    ) -> ConnectionProgressStep {
        let step = ConnectionStepStateItem {
            step_id: format!("{:02}-{}", self.next_step_index, step_kind),
            step_kind: step_kind.to_string(),
            title: title.into(),
            detail: detail.into(),
            hop_label: hop_label.into(),
            state: ConnectionStepState::Running,
        };
        self.next_step_index = self.next_step_index.saturating_add(1);
        let _ = self.event_tx.send(SessionRuntimeEvent::ConnectionProgress(
            ConnectionProgressEvent::StepUpdated {
                attempt_id: self.attempt_id,
                step: step.clone(),
            },
        ));
        ConnectionProgressStep {
            attempt_id: self.attempt_id,
            event_tx: self.event_tx.clone(),
            step_id: step.step_id,
            step_kind: step.step_kind,
            title: step.title,
            hop_label: step.hop_label,
        }
    }

    pub(super) fn set_headline(&self, headline: ConnectionHeadlineState) {
        let _ = self.event_tx.send(SessionRuntimeEvent::ConnectionProgress(
            ConnectionProgressEvent::HeadlineChanged {
                attempt_id: self.attempt_id,
                headline,
            },
        ));
    }
}

impl ConnectionProgressStep {
    pub(super) fn finish(self, detail: impl Into<String>) {
        let detail = detail.into();
        let _ = self.event_tx.send(SessionRuntimeEvent::ConnectionProgress(
            ConnectionProgressEvent::StepUpdated {
                attempt_id: self.attempt_id,
                step: ConnectionStepStateItem {
                    step_id: self.step_id.clone(),
                    step_kind: self.step_kind.clone(),
                    title: self.title.clone(),
                    detail: detail.clone(),
                    hop_label: self.hop_label.clone(),
                    state: ConnectionStepState::Done,
                },
            },
        ));
        let _ = self.event_tx.send(SessionRuntimeEvent::ConnectionProgress(
            ConnectionProgressEvent::DiagnosticAppended {
                attempt_id: self.attempt_id,
                message: detail,
            },
        ));
    }

    pub(super) fn fail(self, detail: impl Into<String>) {
        let detail = detail.into();
        let _ = self.event_tx.send(SessionRuntimeEvent::ConnectionProgress(
            ConnectionProgressEvent::StepUpdated {
                attempt_id: self.attempt_id,
                step: ConnectionStepStateItem {
                    step_id: self.step_id.clone(),
                    step_kind: self.step_kind.clone(),
                    title: self.title.clone(),
                    detail: detail.clone(),
                    hop_label: self.hop_label.clone(),
                    state: ConnectionStepState::Failed,
                },
            },
        ));
        let _ = self.event_tx.send(SessionRuntimeEvent::ConnectionProgress(
            ConnectionProgressEvent::DiagnosticAppended {
                attempt_id: self.attempt_id,
                message: detail,
            },
        ));
    }

    pub(super) fn block(self, detail: impl Into<String>) {
        let detail = detail.into();
        let _ = self.event_tx.send(SessionRuntimeEvent::ConnectionProgress(
            ConnectionProgressEvent::StepUpdated {
                attempt_id: self.attempt_id,
                step: ConnectionStepStateItem {
                    step_id: self.step_id.clone(),
                    step_kind: self.step_kind.clone(),
                    title: self.title.clone(),
                    detail: detail.clone(),
                    hop_label: self.hop_label.clone(),
                    state: ConnectionStepState::Blocked,
                },
            },
        ));
        let _ = self.event_tx.send(SessionRuntimeEvent::ConnectionProgress(
            ConnectionProgressEvent::DiagnosticAppended {
                attempt_id: self.attempt_id,
                message: detail,
            },
        ));
    }
}

impl client::Handler for RuntimeClientHandler {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        match self
            .known_hosts
            .check(&self.host, self.port, server_public_key)?
        {
            KnownHostCheck::Trusted => Ok(true),
            KnownHostCheck::Unknown { fingerprint } => Err(UnknownHostKeyError {
                host: self.host.clone(),
                port: self.port,
                fingerprint,
                public_key_openssh: server_public_key
                    .to_openssh()
                    .context("failed to encode unknown SSH host key")?,
            }
            .into()),
            KnownHostCheck::Changed { expected, actual } => bail!(
                "SSH host key changed for `{}`:{} (expected {}, got {})",
                self.host,
                self.port,
                expected,
                actual
            ),
        }
    }
}

pub(super) async fn authenticate_client(
    handle: &mut client::Handle<RuntimeClientHandler>,
    profile: &ConnectionProfile,
    credential_store: &dyn CredentialStore,
) -> Result<()> {
    match profile.auth_method {
        SshAuthMethod::Password => {
            let password = match profile
                .password
                .clone()
                .filter(|value| !value.trim().is_empty())
            {
                Some(password) => password,
                None => {
                    let stored_bundle = load_required_stored_secret_bundle(
                        profile,
                        credential_store,
                        "SSH password secret",
                    )?;
                    require_profile_secret_field(
                        profile,
                        "SSH password secret",
                        stored_bundle.as_ref(),
                        "password",
                    )?
                }
            };
            let auth_result = handle
                .authenticate_password(profile.user.clone(), password)
                .await
                .context("password authentication failed")?;
            ensure_auth_success(auth_result, "password")?;
        }
        SshAuthMethod::PrivateKeyPath => {
            let private_key_path = profile
                .private_key_path
                .as_deref()
                .filter(|path| !path.trim().is_empty())
                .ok_or_else(|| anyhow!("missing private key path for `{}`", profile.name))?;
            let stored_bundle = if profile
                .passphrase
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty())
            {
                None
            } else {
                load_optional_stored_secret_bundle(profile, credential_store).map_err(|err| {
                    anyhow!(stored_secret_lookup_message(
                        profile,
                        "SSH passphrase secret",
                        &err,
                    ))
                })?
            };
            let passphrase = profile
                .passphrase
                .clone()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    stored_bundle
                        .as_ref()
                        .and_then(|(_, bundle)| non_empty_secret(bundle.passphrase.as_deref()))
                });
            let private_key = keys::load_secret_key(private_key_path, passphrase.as_deref())
                .with_context(|| {
                    format!("failed to load SSH private key from `{private_key_path}`")
                })?;
            let auth_result = handle
                .authenticate_publickey(
                    profile.user.clone(),
                    PrivateKeyWithHashAlg::new(
                        Arc::new(private_key),
                        handle.best_supported_rsa_hash().await?.flatten(),
                    ),
                )
                .await
                .context("private key path authentication failed")?;
            ensure_auth_success(auth_result, "private key path")?;
        }
        SshAuthMethod::PrivateKeyContent => {
            let stored_bundle = if profile
                .private_key_content
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty())
            {
                load_optional_stored_secret_bundle(profile, credential_store).map_err(|err| {
                    anyhow!(stored_secret_lookup_message(
                        profile,
                        "SSH inline private key secret",
                        &err,
                    ))
                })?
            } else {
                load_required_stored_secret_bundle(
                    profile,
                    credential_store,
                    "SSH inline private key secret",
                )?
            };
            let private_key_content = match profile
                .private_key_content
                .clone()
                .filter(|value| !value.trim().is_empty())
            {
                Some(private_key_content) => private_key_content,
                None => require_profile_secret_field(
                    profile,
                    "SSH inline private key secret",
                    stored_bundle.as_ref(),
                    "private_key_content",
                )?,
            };
            let passphrase = profile
                .passphrase
                .clone()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    stored_bundle
                        .as_ref()
                        .and_then(|(_, bundle)| non_empty_secret(bundle.passphrase.as_deref()))
                });
            let private_key = keys::decode_secret_key(&private_key_content, passphrase.as_deref())
                .context("failed to decode inline SSH private key")?;
            let auth_result = handle
                .authenticate_publickey(
                    profile.user.clone(),
                    PrivateKeyWithHashAlg::new(
                        Arc::new(private_key),
                        handle.best_supported_rsa_hash().await?.flatten(),
                    ),
                )
                .await
                .context("inline private key authentication failed")?;
            ensure_auth_success(auth_result, "inline private key")?;
        }
    }

    Ok(())
}

fn ensure_auth_success(result: AuthResult, method: &str) -> Result<()> {
    if result.success() {
        Ok(())
    } else {
        bail!("SSH authentication was rejected for {method}")
    }
}

pub(crate) fn load_optional_stored_secret_bundle(
    profile: &ConnectionProfile,
    credential_store: &dyn CredentialStore,
) -> std::result::Result<Option<(String, StoredSshSecretBundle)>, StoredSecretLookupError> {
    let Some(credential_ref) = profile.credential_ref.as_deref() else {
        return Ok(None);
    };

    let bundle = load_secret_bundle_with_diagnostics(credential_store, Some(credential_ref))?;
    let bundle = match profile.auth_method {
        SshAuthMethod::Password => bundle,
        SshAuthMethod::PrivateKeyContent
            if bundle
                .private_key_content
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty()) =>
        {
            bundle
        }
        SshAuthMethod::PrivateKeyContent => StoredSshSecretBundle {
            private_key_content: bundle.password,
            passphrase: bundle.passphrase,
            proxy_socks5_password: None,
            ..StoredSshSecretBundle::default()
        },
        SshAuthMethod::PrivateKeyPath => bundle,
    };
    Ok(Some((credential_ref.to_string(), bundle)))
}

fn load_required_stored_secret_bundle(
    profile: &ConnectionProfile,
    credential_store: &dyn CredentialStore,
    secret_label: &str,
) -> Result<Option<(String, StoredSshSecretBundle)>> {
    load_optional_stored_secret_bundle(profile, credential_store)
        .map_err(|err| anyhow!(stored_secret_lookup_message(profile, secret_label, &err)))
}

fn require_profile_secret_field(
    profile: &ConnectionProfile,
    secret_label: &str,
    stored_bundle: Option<&(String, StoredSshSecretBundle)>,
    field: &'static str,
) -> Result<String> {
    let Some((credential_ref, bundle)) = stored_bundle else {
        return Err(anyhow!(stored_secret_lookup_message(
            profile,
            secret_label,
            &StoredSecretLookupError::MissingCredentialRef,
        )));
    };

    required_secret_bundle_field(bundle, credential_ref, field)
        .map_err(|err| anyhow!(stored_secret_lookup_message(profile, secret_label, &err)))
}

pub(crate) fn stored_secret_lookup_message(
    profile: &ConnectionProfile,
    secret_label: &str,
    error: &StoredSecretLookupError,
) -> String {
    match error {
        StoredSecretLookupError::MissingCredentialRef => format!(
            "missing credential binding for {secret_label} on `{}`",
            profile.name
        ),
        StoredSecretLookupError::MissingEntry { credential_ref } => format!(
            "missing saved entry `{credential_ref}` for {secret_label} on `{}`",
            profile.name
        ),
        StoredSecretLookupError::ReadFailed {
            credential_ref,
            message,
        } => format!(
            "failed to read saved entry `{credential_ref}` for {secret_label} on `{}`: {message}",
            profile.name
        ),
        StoredSecretLookupError::EmptyBundleField {
            credential_ref,
            field,
        } => format!(
            "saved entry `{credential_ref}` for `{}` is missing field `{field}` required by {secret_label}",
            profile.name
        ),
    }
}

fn non_empty_secret(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}
