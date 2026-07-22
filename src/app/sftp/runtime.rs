//! Runtime abstraction for session-bound SFTP operations.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use tokio::io::{AsyncRead, AsyncSeek, AsyncWrite, AsyncWriteExt};
use uuid::Uuid;

use crate::app::image_policy::MAX_ENCODED_IMAGE_BYTES;
use crate::app::sftp::SftpDirectoryEntryKind;
use crate::app::sftp::model::SftpDirectoryEntry;

const CLIPBOARD_CACHE_ROOT_COMPONENTS: [&str; 3] = [".cache", "mica-term", "clipboard"];
const CLIPBOARD_CACHE_FILE_PREFIX: &str = "mica-clipboard-";
const CLIPBOARD_CACHE_FILE_SUFFIX: &str = ".png";
const CLIPBOARD_CACHE_RETENTION_SECONDS: u64 = 7 * 24 * 60 * 60;
const CLIPBOARD_CACHE_DIRECTORY_PERMISSIONS: u32 = 0o700;
const CLIPBOARD_CACHE_FILE_PERMISSIONS: u32 = 0o600;
const MAX_OLD_CLIPBOARD_CACHE_SESSION_DIRS_SCANNED: usize = 32;
const MAX_CLIPBOARD_CACHE_FILES_SCANNED_PER_SESSION: usize = 128;

pub type SftpOperationFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;
pub type SftpReaderFuture<'a> = Pin<Box<dyn Future<Output = Result<BoxedSftpReader>> + Send + 'a>>;
pub type SftpWriterFuture<'a> = Pin<Box<dyn Future<Output = Result<BoxedSftpWriter>> + Send + 'a>>;

pub trait SftpAsyncReader: AsyncRead + AsyncSeek + Send + Unpin {}
impl<T> SftpAsyncReader for T where T: AsyncRead + AsyncSeek + Send + Unpin {}

pub trait SftpAsyncWriter: AsyncWrite + AsyncSeek + Send + Unpin {}
impl<T> SftpAsyncWriter for T where T: AsyncWrite + AsyncSeek + Send + Unpin {}

pub type BoxedSftpReader = Pin<Box<dyn SftpAsyncReader>>;
pub type BoxedSftpWriter = Pin<Box<dyn SftpAsyncWriter>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SftpRemoteMetadata {
    pub size_bytes: Option<u64>,
    pub modified_unix_seconds: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SftpWriteMode {
    CreateOrTruncate,
    CreateOrAppend,
    CreateNew { permissions: u32 },
}

pub trait SftpBackend: Send + Sync {
    fn canonicalize<'a>(&'a self, path: &'a str) -> SftpOperationFuture<'a, String> {
        Box::pin(async move {
            Err(anyhow!(
                "this SFTP backend cannot canonicalize remote path `{path}`"
            ))
        })
    }
    fn read_dir<'a>(&'a self, path: &'a str) -> SftpOperationFuture<'a, Vec<SftpDirectoryEntry>>;
    fn mkdir<'a>(&'a self, path: &'a str) -> SftpOperationFuture<'a, ()>;
    fn mkdir_with_permissions<'a>(
        &'a self,
        path: &'a str,
        permissions: u32,
    ) -> SftpOperationFuture<'a, ()> {
        Box::pin(async move {
            Err(anyhow!(
                "this SFTP backend cannot create remote directory `{path}` with mode {permissions:o}"
            ))
        })
    }
    fn rename<'a>(&'a self, from: &'a str, to: &'a str) -> SftpOperationFuture<'a, ()>;
    fn path_exists<'a>(&'a self, path: &'a str) -> SftpOperationFuture<'a, bool>;
    fn stat<'a>(&'a self, path: &'a str) -> SftpOperationFuture<'a, SftpRemoteMetadata>;
    fn open_file_reader<'a>(&'a self, path: &'a str) -> SftpReaderFuture<'a>;
    fn open_file_writer<'a>(&'a self, path: &'a str, mode: SftpWriteMode) -> SftpWriterFuture<'a>;
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

    pub async fn canonicalize(&self, path: &str) -> Result<String> {
        self.backend.canonicalize(path).await
    }

    pub async fn read_dir(&self, path: &str) -> Result<Vec<SftpDirectoryEntry>> {
        self.backend.read_dir(path).await
    }

    pub async fn mkdir(&self, path: &str) -> Result<()> {
        self.backend.mkdir(path).await
    }

    pub async fn mkdir_with_permissions(&self, path: &str, permissions: u32) -> Result<()> {
        self.backend.mkdir_with_permissions(path, permissions).await
    }

    pub async fn rename(&self, from: &str, to: &str) -> Result<()> {
        self.backend.rename(from, to).await
    }

    pub async fn path_exists(&self, path: &str) -> Result<bool> {
        self.backend.path_exists(path).await
    }

    pub async fn stat(&self, path: &str) -> Result<SftpRemoteMetadata> {
        self.backend.stat(path).await
    }

    pub async fn open_file_reader(&self, path: &str) -> Result<BoxedSftpReader> {
        self.backend.open_file_reader(path).await
    }

    pub async fn open_file_writer(
        &self,
        path: &str,
        mode: SftpWriteMode,
    ) -> Result<BoxedSftpWriter> {
        self.backend.open_file_writer(path, mode).await
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

    pub async fn upload_clipboard_png(&self, session_id: Uuid, data: Vec<u8>) -> Result<String> {
        let now_unix_seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.upload_clipboard_png_at(session_id, data, now_unix_seconds)
            .await
    }

    async fn upload_clipboard_png_at(
        &self,
        session_id: Uuid,
        data: Vec<u8>,
        now_unix_seconds: u64,
    ) -> Result<String> {
        if data.len() > MAX_ENCODED_IMAGE_BYTES {
            bail!(
                "clipboard PNG exceeds the {} MiB upload limit",
                MAX_ENCODED_IMAGE_BYTES / (1024 * 1024)
            );
        }

        let canonical_home = self
            .canonicalize(".")
            .await
            .context("failed to resolve the canonical remote home directory")?;
        if !canonical_home.starts_with('/') {
            bail!("the SFTP server returned a non-absolute remote home directory");
        }

        let mut clipboard_cache_root = canonical_home;
        for component in CLIPBOARD_CACHE_ROOT_COMPONENTS {
            clipboard_cache_root = remote_child_path(clipboard_cache_root.as_str(), component);
            self.ensure_clipboard_cache_directory(clipboard_cache_root.as_str())
                .await?;
        }
        let cache_dir = remote_child_path(
            clipboard_cache_root.as_str(),
            session_id.to_string().as_str(),
        );
        self.ensure_clipboard_cache_directory(cache_dir.as_str())
            .await?;

        self.cleanup_stale_clipboard_cache(
            clipboard_cache_root.as_str(),
            session_id,
            now_unix_seconds,
        )
        .await;

        let remote_path = remote_child_path(
            cache_dir.as_str(),
            format!(
                "{CLIPBOARD_CACHE_FILE_PREFIX}{}{CLIPBOARD_CACHE_FILE_SUFFIX}",
                Uuid::new_v4()
            )
            .as_str(),
        );
        let mut writer = self
            .open_file_writer(
                remote_path.as_str(),
                SftpWriteMode::CreateNew {
                    permissions: CLIPBOARD_CACHE_FILE_PERMISSIONS,
                },
            )
            .await
            .with_context(|| {
                format!("failed to exclusively create remote clipboard image `{remote_path}`")
            })?;

        let write_result = async {
            writer.write_all(data.as_slice()).await?;
            writer.flush().await?;
            writer.shutdown().await
        }
        .await;
        if let Err(error) = write_result {
            drop(writer);
            let _ = self.delete_file(remote_path.as_str()).await;
            return Err(anyhow!(error)).with_context(|| {
                format!("failed to write remote clipboard image `{remote_path}`")
            });
        }

        Ok(remote_path)
    }

    async fn ensure_clipboard_cache_directory(&self, path: &str) -> Result<()> {
        if self.path_exists(path).await? {
            // The current backend contract cannot lstat or verify modes for pre-existing paths.
            // Do not infer either property here; file protection still relies on 0600 exclusive create.
            return Ok(());
        }
        if let Err(create_error) = self
            .mkdir_with_permissions(path, CLIPBOARD_CACHE_DIRECTORY_PERMISSIONS)
            .await
            && !self.path_exists(path).await.unwrap_or(false)
        {
            return Err(create_error).with_context(|| {
                format!("failed to create remote clipboard cache directory `{path}`")
            });
        }
        Ok(())
    }

    async fn cleanup_stale_clipboard_cache(
        &self,
        clipboard_cache_root: &str,
        current_session_id: Uuid,
        now_unix_seconds: u64,
    ) {
        let cutoff = now_unix_seconds.saturating_sub(CLIPBOARD_CACHE_RETENTION_SECONDS);
        let current_cache_dir = remote_child_path(
            clipboard_cache_root,
            current_session_id.to_string().as_str(),
        );
        let _ = self
            .cleanup_clipboard_session_directory(current_cache_dir.as_str(), cutoff)
            .await;

        let entries = match self.read_dir(clipboard_cache_root).await {
            Ok(entries) => entries,
            Err(error) => {
                tracing::warn!(
                    target: "app.sftp",
                    cache_dir = clipboard_cache_root,
                    error = %error,
                    "failed to inspect remote clipboard session caches"
                );
                return;
            }
        };

        let mut old_session_dirs = entries
            .into_iter()
            .filter_map(|entry| {
                clipboard_cache_session_directory_id(&entry)
                    .map(|session_id| (session_id, entry.modified_unix_seconds))
            })
            .filter(|(session_id, _)| *session_id != current_session_id)
            .collect::<Vec<_>>();
        old_session_dirs.sort_by_key(|(_, modified)| modified.unwrap_or(u64::MAX));
        old_session_dirs.truncate(MAX_OLD_CLIPBOARD_CACHE_SESSION_DIRS_SCANNED);

        for (session_id, directory_modified) in old_session_dirs {
            let session_cache_dir =
                remote_child_path(clipboard_cache_root, session_id.to_string().as_str());
            let confirmed_empty = self
                .cleanup_clipboard_session_directory(session_cache_dir.as_str(), cutoff)
                .await;
            if !confirmed_empty || directory_modified.is_none_or(|modified| modified >= cutoff) {
                continue;
            }
            if let Err(error) = self.delete_dir(session_cache_dir.as_str()).await {
                tracing::warn!(
                    target: "app.sftp",
                    cache_dir = session_cache_dir,
                    error = %error,
                    "failed to remove an empty stale remote clipboard session cache"
                );
            }
        }
    }

    async fn cleanup_clipboard_session_directory(&self, cache_dir: &str, cutoff: u64) -> bool {
        let entries = match self.read_dir(cache_dir).await {
            Ok(entries) => entries,
            Err(error) => {
                tracing::warn!(
                    target: "app.sftp",
                    cache_dir,
                    error = %error,
                    "failed to inspect stale remote clipboard images"
                );
                return false;
            }
        };
        let scan_complete = entries.len() <= MAX_CLIPBOARD_CACHE_FILES_SCANNED_PER_SESSION;
        let was_empty = entries.is_empty();
        let mut removed_any = false;
        for entry in entries
            .into_iter()
            .take(MAX_CLIPBOARD_CACHE_FILES_SCANNED_PER_SESSION)
        {
            if !clipboard_cache_entry_is_stale(&entry, cutoff) {
                continue;
            }
            let stale_path = remote_child_path(cache_dir, entry.name.as_str());
            match self.delete_file(stale_path.as_str()).await {
                Ok(()) => removed_any = true,
                Err(error) => {
                    tracing::warn!(
                        target: "app.sftp",
                        remote_path = stale_path,
                        error = %error,
                        "failed to remove a stale remote clipboard image"
                    );
                }
            }
        }

        if !scan_complete {
            return false;
        }
        if was_empty {
            return true;
        }
        if !removed_any {
            return false;
        }
        self.read_dir(cache_dir)
            .await
            .is_ok_and(|remaining| remaining.is_empty())
    }
}

pub(crate) fn remote_child_path(parent: &str, name: &str) -> String {
    if parent == "/" {
        format!("/{}", name.trim_start_matches('/'))
    } else {
        format!("{}/{}", parent.trim_end_matches('/'), name)
    }
}

fn clipboard_cache_entry_is_stale(entry: &SftpDirectoryEntry, cutoff: u64) -> bool {
    if entry.kind != SftpDirectoryEntryKind::File
        || !entry.name.starts_with(CLIPBOARD_CACHE_FILE_PREFIX)
        || !entry.name.ends_with(CLIPBOARD_CACHE_FILE_SUFFIX)
        || entry
            .modified_unix_seconds
            .is_none_or(|modified| modified >= cutoff)
    {
        return false;
    }

    entry
        .name
        .strip_prefix(CLIPBOARD_CACHE_FILE_PREFIX)
        .and_then(|name| name.strip_suffix(CLIPBOARD_CACHE_FILE_SUFFIX))
        .is_some_and(|id| Uuid::parse_str(id).is_ok())
}

fn clipboard_cache_session_directory_id(entry: &SftpDirectoryEntry) -> Option<Uuid> {
    if entry.kind != SftpDirectoryEntryKind::Directory {
        return None;
    }
    let session_id = Uuid::parse_str(entry.name.as_str()).ok()?;
    (entry.name == session_id.to_string()).then_some(session_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(
        name: &str,
        kind: SftpDirectoryEntryKind,
        modified: Option<u64>,
    ) -> SftpDirectoryEntry {
        SftpDirectoryEntry {
            id: name.into(),
            name: name.into(),
            path: format!("/ignored/{name}"),
            kind,
            modified_unix_seconds: modified,
            size_bytes: None,
            permissions_label: None,
            owner_label: None,
            group_label: None,
        }
    }

    #[test]
    fn remote_child_paths_preserve_root_and_trim_duplicate_separators() {
        assert_eq!(remote_child_path("/", "/tmp"), "/tmp");
        assert_eq!(
            remote_child_path("/home/test/", "cache"),
            "/home/test/cache"
        );
    }

    #[test]
    fn stale_cleanup_accepts_only_owned_uuid_png_files() {
        let owned_name = format!(
            "{CLIPBOARD_CACHE_FILE_PREFIX}{}{CLIPBOARD_CACHE_FILE_SUFFIX}",
            Uuid::nil()
        );
        assert!(clipboard_cache_entry_is_stale(
            &entry(owned_name.as_str(), SftpDirectoryEntryKind::File, Some(10)),
            20
        ));
        assert!(!clipboard_cache_entry_is_stale(
            &entry(owned_name.as_str(), SftpDirectoryEntryKind::File, Some(20)),
            20
        ));
        assert!(!clipboard_cache_entry_is_stale(
            &entry(
                "mica-clipboard-not-a-uuid.png",
                SftpDirectoryEntryKind::File,
                Some(10)
            ),
            20
        ));
        assert!(!clipboard_cache_entry_is_stale(
            &entry(
                owned_name.as_str(),
                SftpDirectoryEntryKind::Symlink,
                Some(10)
            ),
            20
        ));
        assert!(!clipboard_cache_entry_is_stale(
            &entry("unrelated.png", SftpDirectoryEntryKind::File, Some(10)),
            20
        ));
    }
}
