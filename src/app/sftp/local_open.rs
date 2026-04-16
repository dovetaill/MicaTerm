use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SftpOpenAction {
    DownloadAndOpen,
    EditLocally,
}

impl SftpOpenAction {
    fn staging_bucket(self) -> &'static str {
        match self {
            Self::DownloadAndOpen => "downloads",
            Self::EditLocally => "working-copies",
        }
    }
}

pub fn prepare_local_open_path(
    session_id: Uuid,
    remote_path: &str,
    action: SftpOpenAction,
) -> Result<PathBuf> {
    let file_name = remote_path
        .rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or("remote-file");
    let sanitized = sanitize_file_name(file_name);
    let path = std::env::temp_dir()
        .join("mica-term")
        .join("sftp")
        .join(action.staging_bucket())
        .join(session_id.to_string())
        .join(format!("{}-{}", Uuid::new_v4(), sanitized));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(path)
}

pub fn open_path_locally(path: &Path) {
    if let Err(err) = spawn_platform_open_command(path) {
        tracing::warn!(
            target: "app.sftp",
            local_path = %path.display(),
            error = %err,
            "failed to hand off downloaded SFTP file to the local platform opener"
        );
    }
}

fn sanitize_file_name(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => ch,
        })
        .collect::<String>();
    if sanitized.trim().is_empty() {
        "remote-file".into()
    } else {
        sanitized
    }
}

fn spawn_platform_open_command(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", path.to_string_lossy().as_ref()])
            .spawn()
            .map(|_| ())
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).spawn().map(|_| ())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(path).spawn().map(|_| ())
    }
}
