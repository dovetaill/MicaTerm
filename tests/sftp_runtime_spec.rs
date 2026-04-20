use std::future::Future;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use anyhow::{Result, anyhow};
use mica_term::app::sftp::{
    BoxedSftpReader, BoxedSftpWriter, SftpBackend, SftpDirectoryEntry, SftpDirectoryEntryKind,
    SftpRemoteMetadata, SftpRuntimeHandle, SftpWriteMode,
};
use tokio::io::{
    AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt, ReadBuf,
};

struct MemoryFileHandle {
    cursor: Cursor<Vec<u8>>,
}

impl MemoryFileHandle {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            cursor: Cursor::new(bytes),
        }
    }
}

impl AsyncRead for MemoryFileHandle {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let mut chunk = vec![0; buf.remaining()];
        let read = Read::read(&mut self.cursor, &mut chunk)?;
        buf.put_slice(&chunk[..read]);
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for MemoryFileHandle {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let written = Write::write(&mut self.cursor, buf)?;
        Poll::Ready(Ok(written))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl AsyncSeek for MemoryFileHandle {
    fn start_seek(mut self: Pin<&mut Self>, position: SeekFrom) -> std::io::Result<()> {
        Seek::seek(&mut self.cursor, position)?;
        Ok(())
    }

    fn poll_complete(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<u64>> {
        Poll::Ready(Ok(self.cursor.position()))
    }
}

#[derive(Default)]
struct RecordingBackend {
    read_dir_requests: Mutex<Vec<String>>,
    mkdir_requests: Mutex<Vec<String>>,
    rename_requests: Mutex<Vec<(String, String)>>,
    exists_requests: Mutex<Vec<String>>,
    stat_requests: Mutex<Vec<String>>,
    open_reader_requests: Mutex<Vec<String>>,
    open_writer_requests: Mutex<Vec<(String, SftpWriteMode)>>,
    upload_requests: Mutex<Vec<(String, Vec<u8>)>>,
    download_requests: Mutex<Vec<String>>,
    remove_file_requests: Mutex<Vec<String>>,
    remove_dir_requests: Mutex<Vec<String>>,
}

impl SftpBackend for RecordingBackend {
    fn read_dir<'a>(
        &'a self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<SftpDirectoryEntry>>> + Send + 'a>> {
        Box::pin(async move {
            self.read_dir_requests
                .lock()
                .expect("lock read_dir requests")
                .push(path.to_string());
            Ok(vec![SftpDirectoryEntry {
                id: format!("{path}#app"),
                name: "app".into(),
                path: format!("{path}/app"),
                kind: SftpDirectoryEntryKind::Directory,
                modified_unix_seconds: None,
                size_bytes: None,
            }])
        })
    }

    fn mkdir<'a>(&'a self, path: &'a str) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            self.mkdir_requests
                .lock()
                .expect("lock mkdir requests")
                .push(path.to_string());
            Ok(())
        })
    }

    fn rename<'a>(
        &'a self,
        from: &'a str,
        to: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            self.rename_requests
                .lock()
                .expect("lock rename requests")
                .push((from.to_string(), to.to_string()));
            Ok(())
        })
    }

    fn path_exists<'a>(
        &'a self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + 'a>> {
        Box::pin(async move {
            self.exists_requests
                .lock()
                .expect("lock exists requests")
                .push(path.to_string());
            Ok(path.ends_with("existing"))
        })
    }

    fn stat<'a>(
        &'a self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<SftpRemoteMetadata>> + Send + 'a>> {
        Box::pin(async move {
            self.stat_requests
                .lock()
                .expect("lock stat requests")
                .push(path.to_string());
            Ok(SftpRemoteMetadata {
                size_bytes: Some(9),
                modified_unix_seconds: Some(1_710_000_000),
            })
        })
    }

    fn open_file_reader<'a>(
        &'a self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<BoxedSftpReader>> + Send + 'a>> {
        Box::pin(async move {
            self.open_reader_requests
                .lock()
                .expect("lock open reader requests")
                .push(path.to_string());
            Ok(Box::pin(MemoryFileHandle::new(b"abcdefghi".to_vec())) as BoxedSftpReader)
        })
    }

    fn open_file_writer<'a>(
        &'a self,
        path: &'a str,
        mode: SftpWriteMode,
    ) -> Pin<Box<dyn Future<Output = Result<BoxedSftpWriter>> + Send + 'a>> {
        Box::pin(async move {
            self.open_writer_requests
                .lock()
                .expect("lock open writer requests")
                .push((path.to_string(), mode));
            Ok(Box::pin(MemoryFileHandle::new(Vec::new())) as BoxedSftpWriter)
        })
    }

    fn upload_file<'a>(
        &'a self,
        remote_path: &'a str,
        data: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + 'a>> {
        Box::pin(async move {
            self.upload_requests
                .lock()
                .expect("lock upload requests")
                .push((remote_path.to_string(), data.clone()));
            Ok(data.len() as u64)
        })
    }

    fn download_file<'a>(
        &'a self,
        remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
        Box::pin(async move {
            self.download_requests
                .lock()
                .expect("lock download requests")
                .push(remote_path.to_string());
            Ok(format!("bytes:{remote_path}").into_bytes())
        })
    }

    fn remove_file<'a>(
        &'a self,
        remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            self.remove_file_requests
                .lock()
                .expect("lock remove_file requests")
                .push(remote_path.to_string());
            Ok(())
        })
    }

    fn remove_dir<'a>(
        &'a self,
        remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            self.remove_dir_requests
                .lock()
                .expect("lock remove_dir requests")
                .push(remote_path.to_string());
            Ok(())
        })
    }
}

#[derive(Default)]
struct FailingBackend;

impl SftpBackend for FailingBackend {
    fn read_dir<'a>(
        &'a self,
        _path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<SftpDirectoryEntry>>> + Send + 'a>> {
        Box::pin(async move { Err(anyhow!("read_dir failed")) })
    }

    fn mkdir<'a>(
        &'a self,
        _path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { Err(anyhow!("mkdir failed")) })
    }

    fn rename<'a>(
        &'a self,
        _from: &'a str,
        _to: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { Err(anyhow!("rename failed")) })
    }

    fn path_exists<'a>(
        &'a self,
        _path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + 'a>> {
        Box::pin(async move { Err(anyhow!("exists failed")) })
    }

    fn stat<'a>(
        &'a self,
        _path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<SftpRemoteMetadata>> + Send + 'a>> {
        Box::pin(async move { Err(anyhow!("stat failed")) })
    }

    fn open_file_reader<'a>(
        &'a self,
        _path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<BoxedSftpReader>> + Send + 'a>> {
        Box::pin(async move { Err(anyhow!("open reader failed")) })
    }

    fn open_file_writer<'a>(
        &'a self,
        _path: &'a str,
        _mode: SftpWriteMode,
    ) -> Pin<Box<dyn Future<Output = Result<BoxedSftpWriter>> + Send + 'a>> {
        Box::pin(async move { Err(anyhow!("open writer failed")) })
    }

    fn upload_file<'a>(
        &'a self,
        _remote_path: &'a str,
        _data: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + 'a>> {
        Box::pin(async move { Err(anyhow!("upload failed")) })
    }

    fn download_file<'a>(
        &'a self,
        _remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
        Box::pin(async move { Err(anyhow!("download failed")) })
    }

    fn remove_file<'a>(
        &'a self,
        _remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { Err(anyhow!("remove file failed")) })
    }

    fn remove_dir<'a>(
        &'a self,
        _remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { Err(anyhow!("remove dir failed")) })
    }
}

#[tokio::test(flavor = "current_thread")]
async fn runtime_forwards_directory_load_requests_to_backend() {
    let backend = Arc::new(RecordingBackend::default());
    let runtime = SftpRuntimeHandle::new(backend.clone());

    let entries = runtime.read_dir("/srv/app").await.expect("read directory");

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "app");
    assert_eq!(
        backend
            .read_dir_requests
            .lock()
            .expect("lock read_dir requests")
            .as_slice(),
        &["/srv/app".to_string()]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn runtime_forwards_mutating_operations_to_backend() {
    let backend = Arc::new(RecordingBackend::default());
    let runtime = SftpRuntimeHandle::new(backend.clone());

    runtime
        .mkdir("/srv/app/releases")
        .await
        .expect("create remote directory");
    runtime
        .rename("/srv/app/current", "/srv/app/previous")
        .await
        .expect("rename remote path");

    assert_eq!(
        backend
            .mkdir_requests
            .lock()
            .expect("lock mkdir requests")
            .as_slice(),
        &["/srv/app/releases".to_string()]
    );
    assert_eq!(
        backend
            .rename_requests
            .lock()
            .expect("lock rename requests")
            .as_slice(),
        &[(
            "/srv/app/current".to_string(),
            "/srv/app/previous".to_string()
        )]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn runtime_surfaces_backend_failures() {
    let runtime = SftpRuntimeHandle::new(Arc::new(FailingBackend));

    let error = runtime
        .read_dir("/srv/app")
        .await
        .expect_err("backend failure should surface");

    assert!(error.to_string().contains("read_dir failed"));
}

#[tokio::test(flavor = "current_thread")]
async fn runtime_supports_transfer_and_delete_operations() {
    let backend = Arc::new(RecordingBackend::default());
    let runtime = SftpRuntimeHandle::new(backend.clone());

    assert!(
        !runtime
            .path_exists("/srv/app/config")
            .await
            .expect("query path existence")
    );
    assert!(
        runtime
            .path_exists("/srv/app/existing")
            .await
            .expect("query existing path")
    );

    let uploaded = runtime
        .upload_file("/srv/app/config.yml", b"port=22".to_vec())
        .await
        .expect("upload file");
    assert_eq!(uploaded, 7);

    let downloaded = runtime
        .download_file("/srv/app/config.yml")
        .await
        .expect("download file");
    assert_eq!(downloaded, b"bytes:/srv/app/config.yml".to_vec());

    runtime
        .delete_file("/srv/app/config.yml")
        .await
        .expect("delete file");
    runtime
        .delete_dir("/srv/app/releases")
        .await
        .expect("delete directory");

    assert_eq!(
        backend
            .upload_requests
            .lock()
            .expect("lock upload requests")
            .as_slice(),
        &[("/srv/app/config.yml".to_string(), b"port=22".to_vec())]
    );
    assert_eq!(
        backend
            .download_requests
            .lock()
            .expect("lock download requests")
            .as_slice(),
        &["/srv/app/config.yml".to_string()]
    );
    assert_eq!(
        backend
            .remove_file_requests
            .lock()
            .expect("lock remove file requests")
            .as_slice(),
        &["/srv/app/config.yml".to_string()]
    );
    assert_eq!(
        backend
            .remove_dir_requests
            .lock()
            .expect("lock remove dir requests")
            .as_slice(),
        &["/srv/app/releases".to_string()]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn runtime_can_open_seekable_reader_and_writer() {
    let backend = Arc::new(RecordingBackend::default());
    let runtime = SftpRuntimeHandle::new(backend.clone());

    let mut writer = runtime
        .open_file_writer("/srv/app/report.zip.part", SftpWriteMode::CreateOrAppend)
        .await
        .expect("open writer");
    writer.seek(SeekFrom::Start(3)).await.expect("seek writer");
    writer.write_all(b"xyz").await.expect("write bytes");

    let mut reader = runtime
        .open_file_reader("/srv/app/report.zip.part")
        .await
        .expect("open reader");
    reader.seek(SeekFrom::Start(3)).await.expect("seek reader");
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await.expect("read bytes");

    assert_eq!(
        backend
            .open_writer_requests
            .lock()
            .expect("lock open writer requests")
            .as_slice(),
        &[(
            "/srv/app/report.zip.part".to_string(),
            SftpWriteMode::CreateOrAppend,
        )]
    );
    assert_eq!(
        backend
            .open_reader_requests
            .lock()
            .expect("lock open reader requests")
            .as_slice(),
        &["/srv/app/report.zip.part".to_string()]
    );
    assert_eq!(bytes, b"defghi".to_vec());
}

#[tokio::test(flavor = "current_thread")]
async fn runtime_loads_remote_metadata_without_downloading_file() {
    let backend = Arc::new(RecordingBackend::default());
    let runtime = SftpRuntimeHandle::new(backend.clone());

    let metadata = runtime
        .stat("/srv/app/report.zip.part")
        .await
        .expect("load metadata");

    assert_eq!(
        metadata,
        SftpRemoteMetadata {
            size_bytes: Some(9),
            modified_unix_seconds: Some(1_710_000_000),
        }
    );
    assert_eq!(
        backend
            .stat_requests
            .lock()
            .expect("lock stat requests")
            .as_slice(),
        &["/srv/app/report.zip.part".to_string()]
    );
    assert!(
        backend
            .download_requests
            .lock()
            .expect("lock download requests")
            .is_empty()
    );
}
