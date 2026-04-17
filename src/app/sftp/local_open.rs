use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Result, anyhow, bail};
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

pub fn can_open_file_path_locally(path: &Path) -> bool {
    resolve_local_file_target(path).is_ok()
}

pub fn can_open_folder_path_locally(path: &Path) -> bool {
    resolve_local_folder_target(path).is_ok()
}

pub fn open_path_locally(path: &Path) -> Result<()> {
    let file_path = resolve_local_file_target(path)?;
    spawn_platform_file_open_command(file_path).map_err(|err| {
        anyhow!(
            "Could not open `{}` with the default app: {err}",
            file_path.display()
        )
    })
}

pub fn open_path_in_folder_locally(path: &Path) -> Result<()> {
    let target = resolve_local_folder_target(path)?;
    spawn_platform_folder_open_command(&target).map_err(|err| {
        anyhow!(
            "Could not open the local folder for `{}`: {err}",
            target.requested_path.display()
        )
    })
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct FolderOpenTarget {
    requested_path: PathBuf,
    directory_path: PathBuf,
    reveal_path: Option<PathBuf>,
}

fn resolve_local_file_target(path: &Path) -> Result<&Path> {
    if path.as_os_str().is_empty() {
        bail!("Local file path is empty.");
    }
    if !path.exists() {
        bail!("Local file no longer exists.");
    }
    if !path.is_file() {
        bail!("Local file target is not a file.");
    }

    Ok(path)
}

fn resolve_local_folder_target(path: &Path) -> Result<FolderOpenTarget> {
    if path.as_os_str().is_empty() {
        bail!("Local folder path is empty.");
    }

    if path.is_dir() {
        return Ok(FolderOpenTarget {
            requested_path: path.to_path_buf(),
            directory_path: path.to_path_buf(),
            reveal_path: None,
        });
    }

    let directory_path = path
        .parent()
        .filter(|parent| parent.is_dir())
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("Local containing folder no longer exists."))?;
    let reveal_path = if path.is_file() {
        Some(path.to_path_buf())
    } else {
        None
    };

    Ok(FolderOpenTarget {
        requested_path: path.to_path_buf(),
        directory_path,
        reveal_path,
    })
}

fn spawn_platform_file_open_command(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(path)
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

fn spawn_platform_folder_open_command(target: &FolderOpenTarget) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        if let Some(reveal_path) = target.reveal_path.as_deref()
            && Command::new("explorer")
                .arg("/select,")
                .arg(reveal_path)
                .spawn()
                .is_ok()
        {
            return Ok(());
        }

        Command::new("explorer")
            .arg(target.directory_path.as_path())
            .spawn()
            .map(|_| ())
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(reveal_path) = target.reveal_path.as_deref()
            && Command::new("open")
                .arg("-R")
                .arg(reveal_path)
                .spawn()
                .is_ok()
        {
            return Ok(());
        }

        Command::new("open")
            .arg(target.directory_path.as_path())
            .spawn()
            .map(|_| ())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(target.directory_path.as_path())
            .spawn()
            .map(|_| ())
    }
}
