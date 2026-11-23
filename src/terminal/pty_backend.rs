//! Cross-platform local PTY shell backend powered by `portable-pty`.

use std::env;
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
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        if let Some(cwd) = working_dir
            .or_else(|| directories::UserDirs::new().map(|u| u.home_dir().to_path_buf()))
            && cwd.is_dir()
        {
            cmd.cwd(cwd);
        }

        let mut child = pair
            .slave
            .spawn_command(cmd)
            .with_context(|| format!("启动本地 Shell `{shell}` 失败"))?;
        let child_killer = child.clone_killer();
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .context("克隆 PTY 读取端失败")?;
        let mut writer = pair.master.take_writer().context("获取 PTY 写入端失败")?;

        let (write_tx, mut write_rx) = mpsc::unbounded_channel::<PtyWriteCmd>();

        // Writer thread
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

        // Reader thread
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

fn detect_default_shell() -> String {
    #[cfg(target_os = "windows")]
    {
        env::var("COMSPEC").unwrap_or_else(|_| "powershell.exe".to_string())
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(shell) = env::var("SHELL")
            && !shell.trim().is_empty()
        {
            return shell;
        }
        if std::path::Path::new("/bin/bash").exists() {
            "/bin/bash".to_string()
        } else {
            "/bin/sh".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::{AlacrittyTerminalEngine, TerminalPalette};
    use std::time::{Duration, Instant};

    #[test]
    fn spawns_local_pty_and_renders_command_output() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let pty = LocalPtyBackend::spawn(24, 80, None, tx).expect("spawn local pty");
        pty.write(b"printf 'MICA_GPUI_OK\\n'\r".to_vec());

        let mut engine = AlacrittyTerminalEngine::new(24, 80, 500);
        let deadline = Instant::now() + Duration::from_secs(4);
        let mut matched = false;

        while Instant::now() < deadline {
            if let Ok(ev) = rx.try_recv() {
                if let LocalPtyEvent::Output(bytes) = ev {
                    let _ = engine.feed_bytes(&bytes);
                    let snap = engine.snapshot(&TerminalPalette::dark(), true);
                    let text_dump: String = snap
                        .lines
                        .iter()
                        .flat_map(|row| row.text_runs.iter().map(|r| r.text.as_str()))
                        .collect::<Vec<_>>()
                        .join(" ");
                    if text_dump.contains("MICA_GPUI_OK") {
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
