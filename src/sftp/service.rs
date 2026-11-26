//! Session-bound SFTP service (`russh-sftp` 2.4) with cached SFTP subsystem channel,
//! symlink target resolution, and streaming chunked transfers with live byte progress.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Local, TimeZone};
use russh::client;
use russh_sftp::client::SftpSession;
use russh_sftp::client::error::Error as SftpError;
use russh_sftp::protocol::FilePermissions;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex as AsyncMutex;

use crate::ssh::client::SshClientHandler;

const MAX_REMOTE_EDIT_BYTES: u64 = 2 * 1024 * 1024;
const STREAM_CHUNK_BYTES: usize = 64 * 1024;

fn channel_failed(err: &SftpError) -> bool {
    match err {
        SftpError::IO(_) | SftpError::Timeout => true,
        // russh-sftp 2.4 reports closed request channels through UnexpectedBehavior.
        SftpError::UnexpectedBehavior(reason) => {
            matches!(reason.as_str(), "session closed" | "sender dropped")
                || reason.starts_with("SendError: ")
                || reason.starts_with("RecvError: ")
        }
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteEntryKind {
    Directory,
    Symlink,
    File,
    Other,
}

impl RemoteEntryKind {
    pub fn is_dir(self) -> bool {
        matches!(self, Self::Directory)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteEntry {
    pub name: String,
    pub path: String,
    pub kind: RemoteEntryKind,
    pub points_to_dir: bool,
    pub size_bytes: Option<u64>,
    pub modified_unix: Option<u64>,
    pub permissions: String,
}

impl RemoteEntry {
    pub fn is_navigable_dir(&self) -> bool {
        self.kind.is_dir() || self.points_to_dir
    }

    pub fn formatted_size(&self) -> String {
        if self.is_navigable_dir() {
            return "--".to_string();
        }
        self.size_bytes
            .map(format_bytes)
            .unwrap_or_else(|| "--".to_string())
    }

    pub fn formatted_mtime(&self) -> String {
        let Some(ts) = self.modified_unix else {
            return "--".to_string();
        };
        let Some(dt_utc) = DateTime::from_timestamp(ts as i64, 0) else {
            return "--".to_string();
        };
        let local: DateTime<Local> = Local.from_utc_datetime(&dt_utc.naive_utc());
        local.format("%Y-%m-%d %H:%M").to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    Upload,
    Download,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferStatus {
    Running,
    Completed,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct TransferRecord {
    pub id: String,
    pub session_label: String,
    pub direction: TransferDirection,
    pub file_name: String,
    pub bytes_done: u64,
    pub bytes_total: Option<u64>,
    pub status: TransferStatus,
}

impl TransferRecord {
    pub fn progress_ratio(&self) -> Option<f32> {
        let total = self.bytes_total?;
        if total == 0 {
            return Some(1.0);
        }
        Some((self.bytes_done as f32 / total as f32).clamp(0.0, 1.0))
    }
}

#[derive(Clone)]
pub struct SftpService {
    handle: Arc<client::Handle<SshClientHandler>>,
    cached_session: Arc<AsyncMutex<Option<Arc<SftpSession>>>>,
}

impl SftpService {
    pub(crate) fn from_ssh_handle(handle: Arc<client::Handle<SshClientHandler>>) -> Self {
        Self {
            handle,
            cached_session: Arc::new(AsyncMutex::new(None)),
        }
    }

    async fn create_new_sftp(&self) -> Result<Arc<SftpSession>> {
        let channel = self
            .handle
            .channel_open_session()
            .await
            .context("打开 SFTP 子通道失败")?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .context("远程服务器未开启 SFTP 子系统")?;
        let session = SftpSession::new(channel.into_stream())
            .await
            .context("初始化 SFTP 会话失败")?;
        Ok(Arc::new(session))
    }

    async fn get_or_open_sftp(&self) -> Result<Arc<SftpSession>> {
        let mut guard = self.cached_session.lock().await;
        if let Some(existing) = guard.as_ref() {
            return Ok(Arc::clone(existing));
        }
        let fresh = self.create_new_sftp().await?;
        *guard = Some(Arc::clone(&fresh));
        Ok(fresh)
    }

    async fn reopen_sftp(&self, failed: &Arc<SftpSession>) -> Result<Arc<SftpSession>> {
        let mut guard = self.cached_session.lock().await;
        if let Some(session) = guard.as_ref()
            && !Arc::ptr_eq(session, failed)
        {
            return Ok(session.clone());
        }
        *guard = None;
        let fresh = self.create_new_sftp().await?;
        *guard = Some(fresh.clone());
        Ok(fresh)
    }

    pub async fn canonicalize(&self, path: &str) -> Result<String> {
        let sftp = self.get_or_open_sftp().await?;
        let result = match sftp.canonicalize(path).await {
            Err(err) if channel_failed(&err) => {
                self.reopen_sftp(&sftp).await?.canonicalize(path).await
            }
            result => result,
        };
        result.with_context(|| format!("解析远程路径 `{path}` 失败"))
    }

    pub async fn list_dir(&self, dir_path: &str) -> Result<Vec<RemoteEntry>> {
        let mut sftp = self.get_or_open_sftp().await?;
        let read_dir = match sftp.read_dir(dir_path).await {
            Err(err) if channel_failed(&err) => {
                sftp = self.reopen_sftp(&sftp).await?;
                sftp.read_dir(dir_path).await
            }
            result => result,
        }
        .with_context(|| format!("读取远程目录 `{dir_path}` 失败"))?;

        let mut items = Vec::new();
        for entry in read_dir {
            let name = entry.file_name();
            if name == "." || name == ".." {
                continue;
            }
            let full_path = join_remote_path(dir_path, &name);
            let meta = entry.metadata();
            let kind = if meta.is_dir() {
                RemoteEntryKind::Directory
            } else if meta.is_symlink() {
                RemoteEntryKind::Symlink
            } else if meta.is_regular() {
                RemoteEntryKind::File
            } else {
                RemoteEntryKind::Other
            };

            let mut points_to_dir = false;
            let mut size_bytes = meta.size;
            if kind == RemoteEntryKind::Symlink
                && let Ok(target_meta) = sftp.metadata(&full_path).await
            {
                points_to_dir = target_meta.is_dir();
                if !points_to_dir {
                    size_bytes = target_meta.size;
                }
            }

            let permissions = meta
                .permissions
                .map(|p| FilePermissions::from(p).to_string())
                .unwrap_or_default();
            items.push(RemoteEntry {
                path: full_path,
                name,
                kind,
                points_to_dir,
                size_bytes,
                modified_unix: meta.mtime.map(u64::from),
                permissions,
            });
        }

        items.sort_by(|a, b| {
            b.is_navigable_dir()
                .cmp(&a.is_navigable_dir())
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(items)
    }

    pub async fn mkdir(&self, remote_path: &str) -> Result<()> {
        let sftp = self.get_or_open_sftp().await?;
        sftp.create_dir(remote_path)
            .await
            .with_context(|| format!("创建目录 `{remote_path}` 失败"))
    }

    pub async fn rename(&self, from: &str, to: &str) -> Result<()> {
        let sftp = self.get_or_open_sftp().await?;
        sftp.rename(from, to)
            .await
            .with_context(|| format!("重命名 `{from}` -> `{to}` 失败"))
    }

    pub async fn remove(&self, remote_path: &str, is_real_dir: bool) -> Result<()> {
        let sftp = self.get_or_open_sftp().await?;
        if is_real_dir {
            sftp.remove_dir(remote_path)
                .await
                .with_context(|| format!("删除远程目录 `{remote_path}` 失败（请确认目录为空）"))
        } else {
            sftp.remove_file(remote_path)
                .await
                .with_context(|| format!("删除远程文件 `{remote_path}` 失败"))
        }
    }

    pub async fn read_text(&self, remote_path: &str) -> Result<String> {
        let sftp = self.get_or_open_sftp().await?;
        let meta = sftp
            .metadata(remote_path)
            .await
            .with_context(|| format!("读取文件属性 `{remote_path}` 失败"))?;
        if let Some(sz) = meta.size
            && sz > MAX_REMOTE_EDIT_BYTES
        {
            bail!(
                "文件大小为 {}，超过 2 MiB 在线编辑限制，请下载到本地编辑",
                format_bytes(sz)
            );
        }
        let bytes = sftp
            .read(remote_path)
            .await
            .with_context(|| format!("读取远程文件 `{remote_path}` 失败"))?;
        String::from_utf8(bytes).context("该文件不是 UTF-8 文本文件")
    }

    pub async fn write_text(&self, remote_path: &str, content: String) -> Result<()> {
        let sftp = self.get_or_open_sftp().await?;
        let mut file = sftp
            .create(remote_path)
            .await
            .with_context(|| format!("打开远程文件 `{remote_path}` 写入失败"))?;
        file.write_all(content.as_bytes()).await?;
        file.flush().await?;
        file.shutdown().await?;
        Ok(())
    }

    /// Streams a local file to `remote_path` in 64 KiB chunks using a dedicated SFTP session
    /// so concurrent uploads do not block interactive directory browsing.
    pub async fn upload_local_file_with_progress<F>(
        &self,
        local_path: &Path,
        remote_path: &str,
        mut on_progress: F,
    ) -> Result<u64>
    where
        F: FnMut(u64, u64) + Send,
    {
        let meta = tokio::fs::metadata(local_path)
            .await
            .with_context(|| format!("读取本地文件属性 `{}` 失败", local_path.display()))?;
        let total = meta.len();
        let mut local_file = tokio::fs::File::open(local_path)
            .await
            .with_context(|| format!("打开本地文件 `{}` 失败", local_path.display()))?;

        let sftp = self.create_new_sftp().await?;
        let mut remote_file = sftp
            .create(remote_path)
            .await
            .with_context(|| format!("创建远程文件 `{remote_path}` 失败"))?;

        on_progress(0, total);
        let mut buf = vec![0u8; STREAM_CHUNK_BYTES];
        let mut written = 0u64;
        loop {
            let n = local_file.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            remote_file.write_all(&buf[..n]).await?;
            written += n as u64;
            on_progress(written, total);
        }
        remote_file.flush().await?;
        remote_file.shutdown().await?;
        Ok(written)
    }

    /// Streams a remote file to `local_path` in 64 KiB chunks with live progress callbacks.
    pub async fn download_to_local_with_progress<F>(
        &self,
        remote_path: &str,
        local_path: &Path,
        mut on_progress: F,
    ) -> Result<u64>
    where
        F: FnMut(u64, Option<u64>) + Send,
    {
        let sftp = self.create_new_sftp().await?;
        let total = sftp.metadata(remote_path).await.ok().and_then(|m| m.size);
        let mut remote_file = sftp
            .open(remote_path)
            .await
            .with_context(|| format!("打开远程文件 `{remote_path}` 失败"))?;

        if let Some(parent) = local_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut local_file = tokio::fs::File::create(local_path)
            .await
            .with_context(|| format!("创建本地文件 `{}` 失败", local_path.display()))?;

        on_progress(0, total);
        let mut buf = vec![0u8; STREAM_CHUNK_BYTES];
        let mut downloaded = 0u64;
        loop {
            let n = remote_file.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            local_file.write_all(&buf[..n]).await?;
            downloaded += n as u64;
            on_progress(downloaded, total);
        }
        local_file.flush().await?;
        Ok(downloaded)
    }

    pub async fn upload_local_file(&self, local_path: &Path, remote_path: &str) -> Result<u64> {
        self.upload_local_file_with_progress(local_path, remote_path, |_, _| {})
            .await
    }

    pub async fn download_to_local(&self, remote_path: &str, local_path: &Path) -> Result<u64> {
        self.download_to_local_with_progress(remote_path, local_path, |_, _| {})
            .await
    }
}

pub fn join_remote_path(dir: &str, child: &str) -> String {
    let trimmed_dir = dir.trim_end_matches('/');
    let trimmed_child = child.trim_start_matches('/');
    if trimmed_dir.is_empty() {
        format!("/{trimmed_child}")
    } else {
        format!("{trimmed_dir}/{trimmed_child}")
    }
}

pub fn parent_remote_path(path: &str) -> Option<String> {
    let normalized = path.trim_end_matches('/');
    if normalized.is_empty() || normalized == "/" {
        return None;
    }
    match normalized.rfind('/') {
        Some(0) => Some("/".to_string()),
        Some(idx) => Some(normalized[..idx].to_string()),
        None => None,
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_transport_failures_reopen_the_subchannel() {
        use russh_sftp::protocol::{Status, StatusCode};
        for status_code in [
            StatusCode::NoSuchFile,
            StatusCode::PermissionDenied,
            StatusCode::Failure,
        ] {
            assert!(!channel_failed(&SftpError::Status(Status {
                id: 1,
                status_code,
                error_message: "server rejected request".into(),
                language_tag: String::new(),
            })));
        }
        assert!(!channel_failed(&SftpError::UnexpectedPacket));
        assert!(!channel_failed(&SftpError::Limited("packet limit".into())));
        assert!(!channel_failed(&SftpError::UnexpectedBehavior(
            "no file".into()
        )));
        assert!(channel_failed(&SftpError::IO("broken pipe".into())));
        assert!(channel_failed(&SftpError::Timeout));
        assert!(channel_failed(&SftpError::UnexpectedBehavior(
            "session closed".into()
        )));
    }

    #[test]
    fn remote_path_helpers_work() {
        assert_eq!(join_remote_path("/", "etc"), "/etc");
        assert_eq!(join_remote_path("/var/log/", "nginx"), "/var/log/nginx");
        assert_eq!(parent_remote_path("/var/log"), Some("/var".to_string()));
        assert_eq!(parent_remote_path("/var"), Some("/".to_string()));
        assert_eq!(parent_remote_path("/"), None);
    }
}
