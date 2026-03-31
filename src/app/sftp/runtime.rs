//! Runtime abstraction for session-bound SFTP operations.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use uuid::Uuid;

use crate::app::sftp::model::SftpDirectoryEntry;

pub type SftpOperationFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

pub trait SftpBackend: Send + Sync {
    fn read_dir<'a>(&'a self, path: &'a str) -> SftpOperationFuture<'a, Vec<SftpDirectoryEntry>>;
    fn mkdir<'a>(&'a self, path: &'a str) -> SftpOperationFuture<'a, ()>;
    fn rename<'a>(&'a self, from: &'a str, to: &'a str) -> SftpOperationFuture<'a, ()>;
    fn path_exists<'a>(&'a self, path: &'a str) -> SftpOperationFuture<'a, bool>;
    fn upload_file<'a>(
        &'a self,
        remote_path: &'a str,
        data: Vec<u8>,
    ) -> SftpOperationFuture<'a, u64>;
    fn download_file<'a>(&'a self, remote_path: &'a str) -> SftpOperationFuture<'a, Vec<u8>>;
    fn remove_file<'a>(&'a self, remote_path: &'a str) -> SftpOperationFuture<'a, ()>;
    fn remove_dir<'a>(&'a self, remote_path: &'a str) -> SftpOperationFuture<'a, ()>;
}

#[derive(Clone)]
pub struct SftpRuntimeHandle {
    binding_id: Uuid,
    backend: Arc<dyn SftpBackend>,
}

impl SftpRuntimeHandle {
    pub fn new(backend: Arc<dyn SftpBackend>) -> Self {
        Self {
            binding_id: Uuid::new_v4(),
            backend,
        }
    }

    pub fn binding_id(&self) -> Uuid {
        self.binding_id
    }

    pub async fn read_dir(&self, path: &str) -> Result<Vec<SftpDirectoryEntry>> {
        self.backend.read_dir(path).await
    }

    pub async fn mkdir(&self, path: &str) -> Result<()> {
        self.backend.mkdir(path).await
    }

    pub async fn rename(&self, from: &str, to: &str) -> Result<()> {
        self.backend.rename(from, to).await
    }

    pub async fn path_exists(&self, path: &str) -> Result<bool> {
        self.backend.path_exists(path).await
    }

    pub async fn upload_file(&self, remote_path: &str, data: Vec<u8>) -> Result<u64> {
        self.backend.upload_file(remote_path, data).await
    }

    pub async fn download_file(&self, remote_path: &str) -> Result<Vec<u8>> {
        self.backend.download_file(remote_path).await
    }

    pub async fn delete_file(&self, remote_path: &str) -> Result<()> {
        self.backend.remove_file(remote_path).await
    }

    pub async fn delete_dir(&self, remote_path: &str) -> Result<()> {
        self.backend.remove_dir(remote_path).await
    }

    pub async fn move_entry(&self, from: &str, to: &str) -> Result<()> {
        self.backend.rename(from, to).await
    }
}
