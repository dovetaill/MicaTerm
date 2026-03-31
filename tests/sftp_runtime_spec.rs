use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use mica_term::app::sftp::{
    SftpBackend, SftpDirectoryEntry, SftpDirectoryEntryKind, SftpRuntimeHandle,
};

#[derive(Default)]
struct RecordingBackend {
    read_dir_requests: Mutex<Vec<String>>,
    mkdir_requests: Mutex<Vec<String>>,
    rename_requests: Mutex<Vec<(String, String)>>,
    exists_requests: Mutex<Vec<String>>,
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
                size_bytes: None,
            }])
        })
    }

    fn mkdir<'a>(
        &'a self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
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
        &[("/srv/app/current".to_string(), "/srv/app/previous".to_string())]
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

    assert!(!runtime
        .path_exists("/srv/app/config")
        .await
        .expect("query path existence"));
    assert!(runtime
        .path_exists("/srv/app/existing")
        .await
        .expect("query existing path"));

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
