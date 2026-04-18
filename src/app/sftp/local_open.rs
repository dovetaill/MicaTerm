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
    resolve_local_reveal_target(path).is_ok()
}

pub fn open_path_locally(path: &Path) -> Result<()> {
    let open_path = resolve_local_open_target(path)?;
    spawn_platform_shell_open_command(open_path).map_err(|err| {
        anyhow!(
            "Could not open `{}` with the default app: {err}",
            open_path.display()
        )
    })
}

pub fn reveal_path_locally(path: &Path) -> Result<()> {
    let target = resolve_local_reveal_target(path)?;
    spawn_platform_reveal_command(&target).map_err(|err| {
        anyhow!(
            "Could not open the local folder for `{}`: {err}",
            target.requested_path.display()
        )
    })
}

pub fn open_path_in_folder_locally(path: &Path) -> Result<()> {
    reveal_path_locally(path)
}

pub fn trash_path_locally(path: &Path) -> Result<()> {
    let trash_path = resolve_local_trash_target(path)?;
    trash::delete(&trash_path)
        .map_err(|err| anyhow!("Could not move `{}` to Trash: {err}", trash_path.display()))
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

fn resolve_local_open_target(path: &Path) -> Result<&Path> {
    if path.as_os_str().is_empty() {
        bail!("Local path is empty.");
    }
    if !path.exists() {
        bail!("Local path no longer exists.");
    }

    Ok(path)
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

fn resolve_local_reveal_target(path: &Path) -> Result<FolderOpenTarget> {
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

fn resolve_local_trash_target(path: &Path) -> Result<PathBuf> {
    if path.as_os_str().is_empty() {
        bail!("Local path is empty.");
    }
    if !path.exists() {
        bail!("Local file already missing.");
    }

    Ok(path.to_path_buf())
}

fn spawn_platform_shell_open_command(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        // shell-open
        spawn_windows_shell_open(path)
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

fn spawn_platform_reveal_command(target: &FolderOpenTarget) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        if let Some(reveal_path) = target.reveal_path.as_deref()
            && spawn_windows_reveal_path(reveal_path).is_ok()
        {
            return Ok(());
        }

        spawn_windows_shell_open(target.directory_path.as_path())
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
        if let Some(reveal_path) = target.reveal_path.as_deref()
            && spawn_linux_file_manager_show_items(reveal_path).is_ok()
        {
            return Ok(());
        }

        Command::new("xdg-open")
            .arg(target.directory_path.as_path())
            .spawn()
            .map(|_| ())
    }
}

#[cfg(target_os = "windows")]
fn spawn_windows_shell_open(path: &Path) -> std::io::Result<()> {
    let operation = windows_wide("open");
    let target = windows_wide_os(path.as_os_str());
    let result = unsafe {
        ShellExecuteW(
            0,
            operation.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result as usize <= 32 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("ShellExecuteW returned status code {result}"),
        ));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn spawn_windows_reveal_path(path: &Path) -> std::io::Result<()> {
    let _com = WindowsComGuard::initialize()?;
    let target = windows_wide_os(path.as_os_str());
    let pidl = unsafe { ILCreateFromPathW(target.as_ptr()) };
    if pidl.is_null() {
        return Err(std::io::Error::last_os_error());
    }

    let result = unsafe { SHOpenFolderAndSelectItems(pidl, 0, std::ptr::null(), 0) };
    unsafe { ILFree(pidl.cast()) };

    if result < 0 {
        return Err(windows_hresult_error("SHOpenFolderAndSelectItems", result));
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

#[cfg(target_os = "windows")]
fn windows_wide_os(value: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    value.encode_wide().chain(Some(0)).collect()
}

#[cfg(target_os = "windows")]
fn windows_hresult_error(function: &str, hresult: i32) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::Other,
        format!("{function} failed with HRESULT 0x{:08X}", hresult as u32),
    )
}

#[cfg(target_os = "windows")]
struct WindowsComGuard {
    should_uninitialize: bool,
}

#[cfg(target_os = "windows")]
impl WindowsComGuard {
    fn initialize() -> std::io::Result<Self> {
        let result = unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED) };
        if result >= 0 {
            return Ok(Self {
                should_uninitialize: true,
            });
        }
        if result == RPC_E_CHANGED_MODE {
            return Ok(Self {
                should_uninitialize: false,
            });
        }
        Err(windows_hresult_error("CoInitializeEx", result))
    }
}

#[cfg(target_os = "windows")]
impl Drop for WindowsComGuard {
    fn drop(&mut self) {
        if self.should_uninitialize {
            unsafe { CoUninitialize() };
        }
    }
}

#[cfg(target_os = "windows")]
const COINIT_APARTMENTTHREADED: u32 = 0x2;
#[cfg(target_os = "windows")]
const RPC_E_CHANGED_MODE: i32 = 0x8001_0106u32 as i32;
#[cfg(target_os = "windows")]
const SW_SHOWNORMAL: i32 = 1;

#[cfg(target_os = "windows")]
#[repr(C, packed(1))]
struct SHITEMID {
    cb: u16,
    ab_id: [u8; 1],
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct ITEMIDLIST {
    mkid: SHITEMID,
}

#[cfg(target_os = "windows")]
#[link(name = "shell32")]
unsafe extern "system" {
    fn ShellExecuteW(
        hwnd: isize,
        operation: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show_cmd: i32,
    ) -> isize;
    fn ILCreateFromPathW(path: *const u16) -> *mut ITEMIDLIST;
    fn SHOpenFolderAndSelectItems(
        pidl_folder: *const ITEMIDLIST,
        item_count: u32,
        child_items: *const *const ITEMIDLIST,
        flags: u32,
    ) -> i32;
    fn ILFree(item_id_list: *const std::ffi::c_void);
}

#[cfg(target_os = "windows")]
#[link(name = "ole32")]
unsafe extern "system" {
    fn CoInitializeEx(reserved: *mut std::ffi::c_void, coinit: u32) -> i32;
    fn CoUninitialize();
}

#[cfg(all(unix, not(target_os = "macos")))]
fn spawn_linux_file_manager_show_items(path: &Path) -> std::io::Result<()> {
    let uri = file_manager_uri(path);
    let items_arg = format!("[\"{uri}\"]");

    if Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.freedesktop.FileManager1",
            "--object-path",
            "/org/freedesktop/FileManager1",
            "--method",
            "org.freedesktop.FileManager1.ShowItems",
        ])
        .arg(items_arg)
        .arg("")
        .spawn()
        .is_ok()
    {
        return Ok(());
    }

    Command::new("dbus-send")
        .args([
            "--session",
            "--dest=org.freedesktop.FileManager1",
            "--type=method_call",
            "/org/freedesktop/FileManager1",
            "org.freedesktop.FileManager1.ShowItems",
        ])
        .arg(format!("array:string:{uri}"))
        .arg("string:")
        .spawn()
        .map(|_| ())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn file_manager_uri(path: &Path) -> String {
    let absolute = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let path_text = absolute.to_string_lossy();
    let mut uri = String::from("file://");

    for byte in path_text.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                uri.push(char::from(*byte))
            }
            _ => uri.push_str(&format!("%{:02X}", byte)),
        }
    }

    uri
}
