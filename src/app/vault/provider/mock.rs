use std::sync::Mutex;

use anyhow::{Result, anyhow};

use crate::app::vault::model::VaultHead;
use crate::app::vault::provider::{
    ProviderCapabilities, ProviderReadResult, ProviderWriteRequest, VaultProvider,
};

#[derive(Debug)]
pub struct MockVaultProvider {
    remote_id: String,
    capabilities: ProviderCapabilities,
    state: Mutex<MockVaultProviderState>,
}

#[derive(Debug, Default)]
struct MockVaultProviderState {
    remote_head: Option<VaultHead>,
    read_error: Option<String>,
    write_error: Option<String>,
    recorded_writes: Vec<ProviderWriteRequest>,
}

impl MockVaultProvider {
    pub fn new(remote_id: &str, capabilities: ProviderCapabilities) -> Self {
        Self {
            remote_id: remote_id.to_string(),
            capabilities,
            state: Mutex::new(MockVaultProviderState::default()),
        }
    }

    pub fn set_remote_head(&self, head: Option<VaultHead>) {
        if let Ok(mut state) = self.state.lock() {
            state.remote_head = head;
        }
    }

    pub fn set_read_error(&self, message: Option<&str>) {
        if let Ok(mut state) = self.state.lock() {
            state.read_error = message.map(ToOwned::to_owned);
        }
    }

    pub fn set_write_error(&self, message: Option<&str>) {
        if let Ok(mut state) = self.state.lock() {
            state.write_error = message.map(ToOwned::to_owned);
        }
    }

    pub fn recorded_writes(&self) -> Vec<ProviderWriteRequest> {
        self.state
            .lock()
            .map(|state| state.recorded_writes.clone())
            .unwrap_or_default()
    }
}

impl VaultProvider for MockVaultProvider {
    fn remote_id(&self) -> &str {
        self.remote_id.as_str()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.capabilities.clone()
    }

    fn read_head(&self) -> Result<ProviderReadResult> {
        let state = self
            .state
            .lock()
            .map_err(|_| anyhow!("mock provider state lock poisoned"))?;
        if let Some(message) = state.read_error.as_deref() {
            return Err(anyhow!(message.to_string()));
        }

        Ok(ProviderReadResult {
            head: state.remote_head.clone(),
        })
    }

    fn write_revision(&self, request: &ProviderWriteRequest) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow!("mock provider state lock poisoned"))?;
        if let Some(message) = state.write_error.as_deref() {
            return Err(anyhow!(message.to_string()));
        }

        if request.conditional_head_write {
            let actual_revision = state
                .remote_head
                .as_ref()
                .map(|head| head.vault_revision.as_str());
            if request.expected_parent_revision.as_deref() != actual_revision {
                return Err(anyhow!(
                    "conditional head write rejected for `{}`",
                    self.remote_id
                ));
            }
        }

        state.recorded_writes.push(request.clone());
        state.remote_head = Some(request.head.clone());
        Ok(())
    }
}
