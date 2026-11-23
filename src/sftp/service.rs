//! Session-bound SFTP service (`russh-sftp` 2.4) and lightweight transfer tracker.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Local, TimeZone};
use russh::client;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::FilePermissions;
use tokio::io::AsyncWriteExt;

use crate::ssh::client::SshClientHandler;

const MAX_REMOTE_EDIT_BYTES: u64 = 2 * 1024 * 1024;

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
    pub size_bytes: Option<u64>,
    pub modified_unix: Option<u64>,
    pub permissions: String,
}

impl RemoteEntry {
    pub fn formatted_size(&self) -> String {
        if self.kind.is_dir() {
            return "--".to_string();
        }
        self.size_bytes.map(format_bytes).unwrap_or_else(|| "--".to_string())
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
    pub direction: TransferDirection,
    pub file_name: String,
    pub bytes_done: u64,
    pub bytes_total: Option<u64>,
    pub status: TransferStatus,
}

#[derive(Clone)]
pub struct SftpService {
    handle: Arc<client::Handle<SshClientHandler>>,
}

impl SftpService {
    pub(crate) fn from_ssh_handle(handle: Arc<client::Handle<SshClientHandler>>) -> Self {
        Self { handle }
    }

    async fn open_sftp(&self) -> Result<SftpSession> {
        let channel = self
            .handle
            .channel_open_session()
            .await
            .context("打开 SFTP 子通道失败")?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .context("远程服务器未开启 SFTP 子系统")?;
        SftpSession::new(channel.into_stream())
            .await
            .context("初始化 SFTP 会话失败")
    }

    pub async fn canonicalize(&self, path: &str) -> Result<String> {
        let sftp = self.open_sftp().await?;
        sftp.canonicalize(path)
            .await
            .with_context(|| format!("解析远程路径 `{path}` 失败"))
    }

    pub async fn list_dir(&self, dir_path: &str) -> Result<Vec<RemoteEntry>> {
        let sftp = self.open_sftp().await?;
        let read_dir = sftp
            .read_dir(dir_path)
            .await
            .with_context(|| format!("读取远程目录 `{dir_path}` 失败"))?;

        let mut items = Vec::new();
        for entry in read_dir {
            let name = entry.file_name();
            if name == "." || name == ".." {
                continue;
            }
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
            let permissions = meta
                .permissions
                .map(|p| FilePermissions::from(p).to_string())
                .unwrap_or_default();
            items.push(RemoteEntry {
                path: join_remote_path(dir_path, &name),
                name,
                kind,
                size_bytes: meta.size,
                modified_unix: meta.mtime.map(u64::from),
                permissions,
            });
        }

        items.sort_by(|a, b| {
            b.kind
                .is_dir()
                .cmp(&a.kind.is_dir())
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(items)
    }

    pub async fn mkdir(&self, remote_path: &str) -> Result<()> {
        let sftp = self.open_sftp().await?;
        sftp.create_dir(remote_path)
            .await
            .with_context(|| format!("创建目录 `{remote_path}` 失败"))
    }

    pub async fn rename(&self, from: &str, to: &str) -> Result<()> {
        let sftp = self.open_sftp().await?;
        sftp.rename(from, to)
            .await
            .with_context(|| format!("重命名 `{from}` -> `{to}` 失败"))
    }

    pub async fn remove(&self, remote_path: &str, is_dir: bool) -> Result<()> {
        let sftp = self.open_sftp().await?;
        if is_dir {
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
        let sftp = self.open_sftp().await?;
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
        let sftp = self.open_sftp().await?;
        let mut file = sftp
            .create(remote_path)
            .await
            .with_context(|| format!("打开远程文件 `{remote_path}` 写入失败"))?;
        file.write_all(content.as_bytes()).await?;
        file.flush().await?;
        Ok(())
    }

    pub async fn upload_local_file(&self, local_path: &Path, remote_path: &str) -> Result<u64> {
        let data = tokio::fs::read(local_path)
            .await
            .with_context(|| format!("读取本地文件 `{}` 失败", local_path.display()))?;
        let total = data.len() as u64;
        let sftp = self.open_sftp().await?;
        let mut remote_file = sftp
            .create(remote_path)
            .await
            .with_context(|| format!("创建远程文件 `{remote_path}` 失败"))?;
        for chunk in data.chunks(64 * 1024) {
            remote_file.write_all(chunk).await?;
        }
        remote_file.flush().await?;
        Ok(total)
    }

    pub async fn download_to_local(&self, remote_path: &str, local_path: &Path) -> Result<u64> {
        let sftp = self.open_sftp().await?;
        let bytes = sftp
            .read(remote_path)
            .await
            .with_context(|| format!("下载远程文件 `{remote_path}` 失败"))?;
        if let Some(parent) = local_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let total = bytes.len() as u64;
        tokio::fs::write(local_path, bytes)
            .await
            .with_context(|| format!("写入本地文件 `{}` 失败", local_path.display()))?;
        Ok(total)
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
    fn remote_path_helpers_work() {
        assert_eq!(join_remote_path("/", "etc"), "/etc");
        assert_eq!(join_remote_path("/var/log/", "nginx"), "/var/log/nginx");
        assert_eq!(parent_remote_path("/var/log"), Some("/var".to_string()));
        assert_eq!(parent_remote_path("/var"), Some("/".to_string()));
        assert_eq!(parent_remote_path("/"), None);
    }
}
