//! Cross-platform local PTY shell backend powered by `portable-pty`,
//! with automatic OSC 133 + OSC 7 shell integration for local Bash.

use std::env;
#[cfg(any(target_os = "windows", test))]
use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::thread;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use tokio::sync::mpsc;

pub enum LocalPtyEvent {
    Output(Vec<u8>),
    Exited(Option<u32>),
    Error(String),
}

enum PtyWriteCmd {
    Bytes(Vec<u8>),
    Shutdown,
}

pub struct LocalPtyBackend {
    master: Mutex<Box<dyn MasterPty + Send>>,
    write_tx: mpsc::UnboundedSender<PtyWriteCmd>,
    child_killer: Mutex<Box<dyn portable_pty::ChildKiller + Send + Sync>>,
}

impl LocalPtyBackend {
    pub fn spawn(
        rows: u16,
        cols: u16,
        working_dir: Option<PathBuf>,
        event_tx: mpsc::UnboundedSender<LocalPtyEvent>,
    ) -> Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: rows.max(2),
                cols: cols.max(4),
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("打开本地 PTY 失败")?;

        let shell = detect_default_shell();
        let mut cmd = CommandBuilder::new(&shell);
        #[cfg(unix)]
        {
            cmd.arg("-l");
        }
        #[cfg(target_os = "windows")]
        if shell.file_name().is_some_and(|name| {
            name.eq_ignore_ascii_case("pwsh.exe") || name.eq_ignore_ascii_case("powershell.exe")
        }) {
            cmd.arg("-NoLogo");
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", "mica-term");

        // Emit OSC 133 exit code + prompt start + OSC 7 CWD in local bash sessions
        if shell.to_string_lossy().ends_with("bash") && env::var_os("PROMPT_COMMAND").is_none() {
            cmd.env(
                "PROMPT_COMMAND",
                r#"__mica_ec=$?; printf '\033]133;D;%s\007\033]7;file://%s%s\007\033]133;A\007' "$__mica_ec" "${HOSTNAME:-localhost}" "$PWD""#,
            );
        }

        if let Some(cwd) = working_dir
            .or_else(|| directories::UserDirs::new().map(|u| u.home_dir().to_path_buf()))
            && cwd.is_dir()
        {
            cmd.cwd(cwd);
        }

        let mut child = pair
            .slave
            .spawn_command(cmd)
            .with_context(|| format!("启动本地 Shell `{}` 失败", shell.display()))?;
        let child_killer = child.clone_killer();
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .context("克隆 PTY 读取端失败")?;
        let mut writer = pair.master.take_writer().context("获取 PTY 写入端失败")?;

        let (write_tx, mut write_rx) = mpsc::unbounded_channel::<PtyWriteCmd>();

        thread::Builder::new()
            .name("mica-pty-writer".into())
            .spawn(move || {
                while let Some(cmd) = write_rx.blocking_recv() {
                    match cmd {
                        PtyWriteCmd::Bytes(bytes) => {
                            if writer.write_all(&bytes).is_err() || writer.flush().is_err() {
                                break;
                            }
                        }
                        PtyWriteCmd::Shutdown => break,
                    }
                }
            })?;

        let reader_tx = event_tx.clone();
        thread::Builder::new()
            .name("mica-pty-reader".into())
            .spawn(move || {
                let mut buf = [0u8; 16 * 1024];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            if reader_tx
                                .send(LocalPtyEvent::Output(buf[..n].to_vec()))
                                .is_err()
                            {
                                break;
                            }
                        }
                        Err(err) => {
                            let _ = reader_tx.send(LocalPtyEvent::Error(err.to_string()));
                            break;
                        }
                    }
                }
                let code = child.wait().ok().map(|status| status.exit_code());
                let _ = reader_tx.send(LocalPtyEvent::Exited(code));
            })?;

        Ok(Self {
            master: Mutex::new(pair.master),
            write_tx,
            child_killer: Mutex::new(child_killer),
        })
    }

    pub fn write(&self, bytes: Vec<u8>) {
        if !bytes.is_empty() {
            let _ = self.write_tx.send(PtyWriteCmd::Bytes(bytes));
        }
    }

    pub fn resize(&self, rows: u16, cols: u16) {
        let _ = self.master.lock().resize(PtySize {
            rows: rows.max(2),
            cols: cols.max(4),
            pixel_width: 0,
            pixel_height: 0,
        });
    }

    pub fn shutdown(&self) {
        let _ = self.write_tx.send(PtyWriteCmd::Shutdown);
        let _ = self.child_killer.lock().kill();
    }
}

impl Drop for LocalPtyBackend {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn detect_default_shell() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        windows_default_shell(
            &env::var_os("PATH").unwrap_or_default(),
            env::var_os("SystemRoot").as_deref(),
            env::var_os("COMSPEC").as_deref(),
        )
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(shell) = env::var("SHELL")
            && !shell.trim().is_empty()
        {
            return shell.into();
        }
        if std::path::Path::new("/bin/bash").exists() {
            "/bin/bash".into()
        } else {
            "/bin/sh".into()
        }
    }
}

#[cfg(any(target_os = "windows", test))]
fn windows_default_shell(
    path: &OsStr,
    system_root: Option<&OsStr>,
    comspec: Option<&OsStr>,
) -> PathBuf {
    let on_path = ["pwsh.exe", "powershell.exe"].into_iter().flat_map(|name| {
        env::split_paths(path)
            .filter(|dir| !dir.as_os_str().is_empty())
            .map(move |dir| dir.join(name))
    });
    let inbox = system_root
        .map(|root| PathBuf::from(root).join("System32/WindowsPowerShell/v1.0/powershell.exe"));
    on_path
        .chain(inbox)
        .find(|exe| exe.is_file())
        .or_else(|| comspec.filter(|shell| !shell.is_empty()).map(PathBuf::from))
        .unwrap_or_else(|| "cmd.exe".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::{AlacrittyTerminalEngine, SemanticOptions, TerminalPalette};
    use std::time::{Duration, Instant};

    #[test]
    fn windows_shell_prefers_pwsh_then_powershell_then_comspec() {
        let root = env::temp_dir().join(format!("mica-gpui-shell-{}", uuid::Uuid::new_v4()));
        let bin = root.join("bin");
        let inbox = root.join("System32/WindowsPowerShell/v1.0/powershell.exe");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(inbox.parent().unwrap()).unwrap();
        let path = env::join_paths([&root, &bin]).unwrap();
        let comspec = root.join("custom-cmd.exe");
        let detect =
            || windows_default_shell(&path, Some(root.as_os_str()), Some(comspec.as_os_str()));

        assert_eq!(
            windows_default_shell(&path, None, None),
            PathBuf::from("cmd.exe")
        );
        assert_eq!(detect(), comspec);
        for candidate in [inbox, root.join("powershell.exe"), bin.join("pwsh.exe")] {
            std::fs::write(&candidate, []).unwrap();
            assert_eq!(detect(), candidate);
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn spawns_local_pty_and_renders_command_output() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let pty = LocalPtyBackend::spawn(24, 80, None, tx).expect("spawn local pty");
        pty.write(b"echo MICA_GPUI_OK\r".to_vec());

        let mut engine = AlacrittyTerminalEngine::new(24, 80, 500);
        let deadline = Instant::now() + Duration::from_secs(4);
        let mut matched = false;

        while Instant::now() < deadline {
            if let Ok(ev) = rx.try_recv() {
                if let LocalPtyEvent::Output(bytes) = ev {
                    let _ = engine.feed_bytes(&bytes);
                    let snap = engine.snapshot(
                        &TerminalPalette::dark(),
                        true,
                        SemanticOptions::default(),
                    );
                    if snap.lines.iter().any(|row| {
                        row.text_runs
                            .iter()
                            .any(|run| run.start_col == 0 && run.text == "MICA_GPUI_OK")
                    }) {
                        matched = true;
                        break;
                    }
                }
            } else {
                std::thread::sleep(Duration::from_millis(20));
            }
        }

        pty.shutdown();
        assert!(matched, "expected PTY output MICA_GPUI_OK in terminal grid");
    }
}
