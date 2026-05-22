//! SSH runtime russh SFTP backend adapter.

use std::sync::Arc;

use anyhow::{Context, Result};
use russh::client;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::FilePermissions;
use russh_sftp::protocol::OpenFlags;
use tokio::io::AsyncWriteExt;

use crate::app::sftp::{
    BoxedSftpReader, BoxedSftpWriter, SftpBackend, SftpDirectoryEntry, SftpOperationFuture,
    SftpReaderFuture, SftpRemoteMetadata, SftpWriteMode, SftpWriterFuture,
};

use super::auth::RuntimeClientHandler;

pub(super) struct RusshSftpBackend {
    pub(super) handle: Arc<client::Handle<RuntimeClientHandler>>,
}

impl RusshSftpBackend {
    async fn open_sftp_session(&self) -> Result<SftpSession> {
        let channel = self
            .handle
            .channel_open_session()
            .await
            .context("failed to open SSH session channel for SFTP subsystem")?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .context("failed to request SFTP subsystem")?;
        SftpSession::new(channel.into_stream())
            .await
            .context("failed to initialize SFTP client session")
    }
}

impl SftpBackend for RusshSftpBackend {
    fn read_dir<'a>(&'a self, path: &'a str) -> SftpOperationFuture<'a, Vec<SftpDirectoryEntry>> {
        Box::pin(async move {
            let sftp = self.open_sftp_session().await?;
            let mut read_dir = sftp
                .read_dir(path)
                .await
                .with_context(|| format!("failed to read remote directory `{path}`"))?;
            let mut entries = Vec::new();

            for entry in &mut read_dir {
                let name = entry.file_name();
                let child_path = remote_child_path(path, &name);
                let metadata = entry.metadata();
                let kind = if metadata.is_dir() {
                    crate::app::sftp::SftpDirectoryEntryKind::Directory
                } else if metadata.is_symlink() {
                    crate::app::sftp::SftpDirectoryEntryKind::Symlink
                } else if metadata.is_regular() {
                    crate::app::sftp::SftpDirectoryEntryKind::File
                } else {
                    crate::app::sftp::SftpDirectoryEntryKind::Unknown
                };
                entries.push(SftpDirectoryEntry {
                    id: child_path.clone(),
                    name,
                    path: child_path,
                    kind,
                    modified_unix_seconds: metadata.mtime.map(u64::from),
                    size_bytes: metadata.size,
                    permissions_label: metadata
                        .permissions
                        .map(|permissions| FilePermissions::from(permissions).to_string()),
                    owner_label: metadata
                        .user
                        .clone()
                        .or_else(|| metadata.uid.map(|id| id.to_string())),
                    group_label: metadata
                        .group
                        .clone()
                        .or_else(|| metadata.gid.map(|id| id.to_string())),
                });
            }

            Ok(entries)
        })
    }

    fn mkdir<'a>(&'a self, path: &'a str) -> SftpOperationFuture<'a, ()> {
        Box::pin(async move {
            let sftp = self.open_sftp_session().await?;
            sftp.create_dir(path)
                .await
                .with_context(|| format!("failed to create remote directory `{path}`"))?;
            Ok(())
        })
    }

    fn rename<'a>(&'a self, from: &'a str, to: &'a str) -> SftpOperationFuture<'a, ()> {
        Box::pin(async move {
            let sftp = self.open_sftp_session().await?;
            sftp.rename(from, to)
                .await
                .with_context(|| format!("failed to rename remote path `{from}` -> `{to}`"))?;
            Ok(())
        })
    }

    fn path_exists<'a>(&'a self, path: &'a str) -> SftpOperationFuture<'a, bool> {
        Box::pin(async move {
            let sftp = self.open_sftp_session().await?;
            sftp.try_exists(path)
                .await
                .with_context(|| format!("failed to check remote path `{path}`"))
        })
    }

    fn stat<'a>(&'a self, path: &'a str) -> SftpOperationFuture<'a, SftpRemoteMetadata> {
        Box::pin(async move {
            let sftp = self.open_sftp_session().await?;
            let metadata = sftp
                .metadata(path)
                .await
                .with_context(|| format!("failed to stat remote path `{path}`"))?;
            Ok(SftpRemoteMetadata {
                size_bytes: metadata.size,
                modified_unix_seconds: metadata.mtime.map(u64::from),
            })
        })
    }

    fn open_file_reader<'a>(&'a self, path: &'a str) -> SftpReaderFuture<'a> {
        Box::pin(async move {
            let sftp = self.open_sftp_session().await?;
            let file = sftp
                .open(path)
                .await
                .with_context(|| format!("failed to open remote file `{path}` for reading"))?;
            Ok(Box::pin(file) as BoxedSftpReader)
        })
    }

    fn open_file_writer<'a>(&'a self, path: &'a str, mode: SftpWriteMode) -> SftpWriterFuture<'a> {
        Box::pin(async move {
            let sftp = self.open_sftp_session().await?;
            let flags = match mode {
                SftpWriteMode::CreateOrTruncate => {
                    OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE
                }
                SftpWriteMode::CreateOrAppend => {
                    OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::APPEND
                }
            };
            let file = sftp
                .open_with_flags(path, flags)
                .await
                .with_context(|| format!("failed to open remote file `{path}` for writing"))?;
            Ok(Box::pin(file) as BoxedSftpWriter)
        })
    }

    fn upload_file<'a>(
        &'a self,
        remote_path: &'a str,
        data: Vec<u8>,
    ) -> SftpOperationFuture<'a, u64> {
        Box::pin(async move {
            let sftp = self.open_sftp_session().await?;
            let mut file = sftp
                .create(remote_path)
                .await
                .with_context(|| format!("failed to create remote file `{remote_path}`"))?;
            file.write_all(&data)
                .await
                .with_context(|| format!("failed to write remote file `{remote_path}`"))?;
            Ok(data.len() as u64)
        })
    }

    fn download_file<'a>(&'a self, remote_path: &'a str) -> SftpOperationFuture<'a, Vec<u8>> {
        Box::pin(async move {
            let sftp = self.open_sftp_session().await?;
            sftp.read(remote_path)
                .await
                .with_context(|| format!("failed to read remote file `{remote_path}`"))
        })
    }

    fn remove_file<'a>(&'a self, remote_path: &'a str) -> SftpOperationFuture<'a, ()> {
        Box::pin(async move {
            let sftp = self.open_sftp_session().await?;
            sftp.remove_file(remote_path)
                .await
                .with_context(|| format!("failed to remove remote file `{remote_path}`"))?;
            Ok(())
        })
    }

    fn remove_dir<'a>(&'a self, remote_path: &'a str) -> SftpOperationFuture<'a, ()> {
        Box::pin(async move {
            let sftp = self.open_sftp_session().await?;
            sftp.remove_dir(remote_path)
                .await
                .with_context(|| format!("failed to remove remote directory `{remote_path}`"))?;
            Ok(())
        })
    }
}

fn remote_child_path(parent: &str, name: &str) -> String {
    if parent == "/" {
        format!("/{}", name.trim_start_matches('/'))
    } else {
        format!("{}/{}", parent.trim_end_matches('/'), name)
    }
}
