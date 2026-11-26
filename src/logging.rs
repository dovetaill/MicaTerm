//! Runtime diagnostics, Windows console attachment, and always-on crash reporting.

use std::backtrace::Backtrace;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::panic;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context as _, Result};
use chrono::Local;
use parking_lot::RwLock;
use tracing_subscriber::EnvFilter;

use crate::storage::AppPaths;

const MAX_LOG_FILE_BYTES: u64 = 5 * 1024 * 1024;
static RUNTIME_LOG: OnceLock<RuntimeLog> = OnceLock::new();

struct RuntimeLog {
    path: PathBuf,
    console: bool,
    enabled: AtomicBool,
    file: RwLock<Option<File>>,
}

impl RuntimeLog {
    fn new(path: PathBuf, console: bool) -> Self {
        Self {
            path,
            console,
            enabled: AtomicBool::new(false),
            file: RwLock::new(None),
        }
    }

    fn configure(&self, requested: bool) -> Result<()> {
        let enabled = requested || self.console;
        let mut guard = self.file.write();
        if !enabled {
            *guard = None;
            self.enabled.store(false, Ordering::Release);
            return Ok(());
        }

        if guard.is_none() {
            if let Some(parent) = self.path.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("创建运行日志目录失败：{}", parent.display())
                })?;
            }
            if let Ok(meta) = fs::metadata(&self.path)
                && meta.len() > MAX_LOG_FILE_BYTES
            {
                let prev = self.path.with_file_name("mica-term.prev.log");
                let _ = fs::remove_file(&prev);
                let _ = fs::rename(&self.path, &prev);
            }
            let mut f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
                .with_context(|| format!("打开运行日志失败：{}", self.path.display()))?;
            let now = Local::now().format("%Y-%m-%d %H:%M:%S%.3f %:z");
            let _ = writeln!(
                f,
                "\n================================================================================"
            );
            let _ = writeln!(
                f,
                "[{}] Mica Term GPUI v{} runtime log enabled | os={} arch={} pid={}",
                now,
                env!("CARGO_PKG_VERSION"),
                env::consts::OS,
                env::consts::ARCH,
                std::process::id(),
            );
            let _ = f.flush();
            *guard = Some(f);
        }
        self.enabled.store(true, Ordering::Release);
        Ok(())
    }
}

impl Write for &RuntimeLog {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Best-effort stderr output: Windows GUI subsystem processes without `--console`
        // have no attached console handle, which must not fail tracing writers.
        let _ = io::stderr().write_all(buf);
        if self.enabled.load(Ordering::Acquire)
            && let Some(mut file) = self.file.read().as_ref()
        {
            let _ = file.write_all(buf);
            let _ = file.flush();
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let _ = io::stderr().flush();
        if self.enabled.load(Ordering::Acquire)
            && let Some(mut file) = self.file.read().as_ref()
        {
            let _ = file.flush();
        }
        Ok(())
    }
}

/// Attaches or allocates a Win32 console window when `--console` / `-v` or `MICA_TERM_CONSOLE=1` is set.
pub fn maybe_attach_console_from_env() -> bool {
    let console = env::args_os().any(|a| a == "--console" || a == "-v")
        || env::var_os("MICA_TERM_CONSOLE").is_some_and(|v| v != "0" && !v.is_empty());
    #[cfg(target_os = "windows")]
    if console {
        win_diag::attach_or_alloc_console();
    }
    console
}

pub fn init_logging_and_crash_handlers(paths: &AppPaths, console: bool) -> Result<()> {
    install_panic_hook(paths.crash_log_path.clone());
    #[cfg(target_os = "windows")]
    win_diag::install_native_crash_handlers(&paths.crash_log_path);

    RUNTIME_LOG
        .set(RuntimeLog::new(paths.runtime_log_path.clone(), console))
        .map_err(|_| anyhow::anyhow!("运行日志已经初始化"))?;
    let log = RUNTIME_LOG
        .get()
        .expect("runtime logger was just initialized");
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info,mica_term_gpui=debug,gpui=warn,gpui_pre_windows=info,russh=info")
    });
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true)
        .with_writer(move || log)
        .try_init()
        .map_err(|err| anyhow::anyhow!("初始化日志输出失败：{err}"))?;
    if console && let Err(err) = set_debug_logging(false) {
        tracing::error!(error = %err, "无法启用控制台的运行日志文件");
    }
    Ok(())
}

pub fn set_debug_logging(requested: bool) -> Result<()> {
    let log = RUNTIME_LOG.get().context("运行日志尚未初始化")?;
    log.configure(requested)?;
    if requested || log.console {
        tracing::info!(
            debug_logging = requested,
            console = log.console,
            version = env!("CARGO_PKG_VERSION"),
            pid = std::process::id(),
            os = env::consts::OS,
            arch = env::consts::ARCH,
            "运行调试日志已开启"
        );
    }
    Ok(())
}

fn install_panic_hook(crash_path: PathBuf) {
    panic::set_hook(Box::new(move |panic_info| {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f %:z");
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("unnamed");
        let location = panic_info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".to_string());

        let payload = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Box<dyn Any> panic payload".to_string()
        };

        let backtrace = Backtrace::force_capture();
        let report = format!(
            "\n==================== MICA TERM RUST PANIC ====================\n\
             Time:      {timestamp}\n\
             Version:   {} ({}/{})\n\
             PID:       {}\n\
             Thread:    {thread_name} ({:?})\n\
             Location:  {location}\n\
             Message:   {payload}\n\
             Backtrace:\n{backtrace}\n\
             ==============================================================\n",
            env!("CARGO_PKG_VERSION"),
            env::consts::OS,
            env::consts::ARCH,
            std::process::id(),
            thread.id(),
        );

        let _ = io::stderr().write_all(report.as_bytes());
        let _ = io::stderr().flush();

        if let Some(parent) = crash_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let crash_result = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&crash_path)
            .and_then(|mut file| {
                file.write_all(report.as_bytes())?;
                file.sync_all()
            });
        let crash_status = match crash_result {
            Ok(()) => format!("完整堆栈已写入日志文件：\n{}", crash_path.display()),
            Err(err) => format!("写入崩溃日志失败：{}：{err}", crash_path.display()),
        };
        if let Some(log) = RUNTIME_LOG.get()
            && let Some(guard) = log.file.try_read()
            && let Some(mut file) = guard.as_ref()
        {
            let _ = file.write_all(report.as_bytes());
            let _ = file.sync_all();
        }

        show_fatal_startup_error(
            "Mica Term 发生未捕获异常 (Panic)",
            &format!(
                "程序遇到严重错误并即将退出：\n\n位置：{location}\n原因：{payload}\n\n{crash_status}"
            ),
        );
    }));
}

/// Displays a native modal error box on Windows (and stderr on other platforms) when fatal startup/panic errors occur.
pub fn show_fatal_startup_error(title: &str, message: &str) {
    let _ = writeln!(io::stderr(), "[FATAL] {title}: {message}");
    #[cfg(target_os = "windows")]
    win_diag::message_box_error(title, message);
}

/// Reads recent `crash.log` (if present) and the tail of `mica-term.log` up to `max_bytes` for UI display.
pub fn read_recent_logs(paths: &AppPaths, max_bytes: usize) -> String {
    let mut out = format!(
        "# 日志目录: {}\n# 运行日志: {}\n# 崩溃日志: {}\n\n",
        paths.logs_dir.display(),
        paths.runtime_log_path.display(),
        paths.crash_log_path.display()
    );
    let tail = |path: &Path, cap| match read_file_tail_lossy(path, cap) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
        Err(err) => format!("读取日志失败：{}：{err}\n", path.display()),
    };
    let crash_tail = tail(&paths.crash_log_path, max_bytes / 3);
    if !crash_tail.trim().is_empty() {
        out.push_str("=== 最近崩溃记录 (crash.log) ===\n");
        out.push_str(&crash_tail);
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
    }

    out.push_str("=== 最近运行日志 (mica-term.log) ===\n");
    let runtime_tail = tail(&paths.runtime_log_path, max_bytes);
    if runtime_tail.trim().is_empty() {
        out.push_str("（暂无日志内容）\n");
    } else {
        out.push_str(&runtime_tail);
    }
    out
}

fn read_file_tail_lossy(path: &Path, max_bytes: usize) -> io::Result<String> {
    let mut f = File::open(path)?;
    let len = f.metadata()?.len();
    let cap = max_bytes.max(1024) as u64;
    let mut prefix = String::new();
    if len > cap {
        f.seek(SeekFrom::Start(len - cap))?;
        prefix = format!("... (已省略前 {} KB 日志) ...\n", (len - cap) / 1024);
    }
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    if prefix.is_empty() {
        Ok(text.into_owned())
    } else {
        // Align to next newline so UTF-8 / line boundaries look clean
        let aligned = text.find('\n').map(|idx| &text[idx + 1..]).unwrap_or(&text);
        Ok(format!("{prefix}{aligned}"))
    }
}

/// Opens a directory in the system file manager (Explorer / Finder / xdg-open).
pub fn open_directory_in_file_manager(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|err| format!("无法创建目录 {}：{err}", dir.display()))?;
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer.exe")
            .arg(dir)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("无法打开资源管理器: {e}"))
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(dir)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("无法打开 Finder: {e}"))
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(dir)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("无法打开文件管理器: {e}"))
    }
}

#[cfg(target_os = "windows")]
mod win_diag {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use std::ptr;
    use std::sync::OnceLock;

    static CRASH_PATH_WIDE: OnceLock<Vec<u16>> = OnceLock::new();

    const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;
    const FILE_APPEND_DATA: u32 = 0x0004;
    const FILE_SHARE_READ_WRITE: u32 = 0x0001 | 0x0002;
    const OPEN_ALWAYS: u32 = 4;
    const FILE_ATTRIBUTE_NORMAL: u32 = 0x0080;
    const INVALID_HANDLE_VALUE: isize = -1;
    const EXCEPTION_CONTINUE_SEARCH: i32 = 0;
    const MB_OK_ICONERROR: u32 = 0x0000_0010;
    const SIGABRT: i32 = 22;

    #[repr(C)]
    struct ExceptionRecord {
        exception_code: u32,
        exception_flags: u32,
        exception_record: *mut ExceptionRecord,
        exception_address: *mut c_void,
        number_parameters: u32,
        exception_information: [usize; 15],
    }

    #[repr(C)]
    struct ExceptionPointers {
        exception_record: *mut ExceptionRecord,
        context_record: *mut c_void,
    }

    type TopLevelExceptionFilter = unsafe extern "system" fn(*const ExceptionPointers) -> i32;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn AttachConsole(dwProcessId: u32) -> i32;
        fn AllocConsole() -> i32;
        fn SetUnhandledExceptionFilter(
            lpTopLevelExceptionFilter: Option<TopLevelExceptionFilter>,
        ) -> Option<TopLevelExceptionFilter>;
        fn GetModuleHandleW(lpModuleName: *const u16) -> isize;
        fn GetCurrentThreadId() -> u32;
        fn GetCurrentProcessId() -> u32;
        fn CreateFileW(
            lpFileName: *const u16,
            dwDesiredAccess: u32,
            dwShareMode: u32,
            lpSecurityAttributes: *const c_void,
            dwCreationDisposition: u32,
            dwFlagsAndAttributes: u32,
            hTemplateFile: isize,
        ) -> isize;
        fn WriteFile(
            hFile: isize,
            lpBuffer: *const u8,
            nNumberOfBytesToWrite: u32,
            lpNumberOfBytesWritten: *mut u32,
            lpOverlapped: *mut c_void,
        ) -> i32;
        fn FlushFileBuffers(hFile: isize) -> i32;
        fn CloseHandle(hObject: isize) -> i32;
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(
            hWnd: isize,
            lpText: *const u16,
            lpCaption: *const u16,
            uType: u32,
        ) -> i32;
    }

    unsafe extern "C" {
        fn signal(sig: i32, func: usize) -> usize;
    }

    pub(super) fn attach_or_alloc_console() {
        unsafe {
            if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
                let _ = AllocConsole();
            }
        }
    }

    pub(super) fn message_box_error(title: &str, message: &str) {
        let mut title_w: Vec<u16> = title.encode_utf16().collect();
        title_w.push(0);
        let mut msg_w: Vec<u16> = message.encode_utf16().collect();
        msg_w.push(0);
        unsafe {
            let _ = MessageBoxW(0, msg_w.as_ptr(), title_w.as_ptr(), MB_OK_ICONERROR);
        }
    }

    pub(super) fn install_native_crash_handlers(crash_log_path: &Path) {
        let mut wide: Vec<u16> = crash_log_path.as_os_str().encode_wide().collect();
        wide.push(0);
        let _ = CRASH_PATH_WIDE.set(wide);

        unsafe {
            let _ = SetUnhandledExceptionFilter(Some(seh_exception_filter));
            let _ = signal(SIGABRT, sigabrt_handler as *const () as usize);
        }
    }

    fn write_raw_crash_bytes(bytes: &[u8]) {
        let Some(path_w) = CRASH_PATH_WIDE.get() else {
            return;
        };
        unsafe {
            let handle = CreateFileW(
                path_w.as_ptr(),
                FILE_APPEND_DATA,
                FILE_SHARE_READ_WRITE,
                ptr::null(),
                OPEN_ALWAYS,
                FILE_ATTRIBUTE_NORMAL,
                0,
            );
            if handle != INVALID_HANDLE_VALUE && handle != 0 {
                let mut written = 0u32;
                let _ = WriteFile(
                    handle,
                    bytes.as_ptr(),
                    bytes.len() as u32,
                    &mut written,
                    ptr::null_mut(),
                );
                let _ = FlushFileBuffers(handle);
                let _ = CloseHandle(handle);
            }
        }
    }

    struct StackBuf {
        buf: [u8; 768],
        len: usize,
    }

    impl StackBuf {
        const fn new() -> Self {
            Self {
                buf: [0u8; 768],
                len: 0,
            }
        }

        fn push_str(&mut self, s: &str) {
            let b = s.as_bytes();
            let avail = self.buf.len().saturating_sub(self.len);
            let n = b.len().min(avail);
            self.buf[self.len..self.len + n].copy_from_slice(&b[..n]);
            self.len += n;
        }

        fn push_hex(&mut self, val: usize, digits: usize) {
            const HEX: &[u8; 16] = b"0123456789ABCDEF";
            self.push_str("0x");
            for i in (0..digits).rev() {
                if self.len >= self.buf.len() {
                    break;
                }
                let nibble = (val >> (i * 4)) & 0xF;
                self.buf[self.len] = HEX[nibble];
                self.len += 1;
            }
        }

        fn push_u32(&mut self, mut val: u32) {
            let mut tmp = [0u8; 10];
            let mut idx = 10;
            if val == 0 {
                self.push_str("0");
                return;
            }
            while val > 0 && idx > 0 {
                idx -= 1;
                tmp[idx] = b'0' + (val % 10) as u8;
                val /= 10;
            }
            if let Ok(s) = core::str::from_utf8(&tmp[idx..]) {
                self.push_str(s);
            }
        }

        fn as_bytes(&self) -> &[u8] {
            &self.buf[..self.len]
        }
    }

    fn exception_code_name(code: u32) -> &'static str {
        match code {
            0xC000_0005 => "EXCEPTION_ACCESS_VIOLATION",
            0xC000_001D => "EXCEPTION_ILLEGAL_INSTRUCTION",
            0xC000_00FD => "EXCEPTION_STACK_OVERFLOW",
            0xC000_0409 => "STATUS_STACK_BUFFER_OVERRUN / FAST_FAIL",
            0xC000_0094 => "EXCEPTION_INT_DIVIDE_BY_ZERO",
            0x8000_0003 => "EXCEPTION_BREAKPOINT",
            0xE06D_7363 => "CPP_EH_EXCEPTION",
            _ => "WIN32_SEH_EXCEPTION",
        }
    }

    unsafe extern "system" fn seh_exception_filter(info: *const ExceptionPointers) -> i32 {
        let mut sb = StackBuf::new();
        sb.push_str("\n==================== MICA TERM WIN32 SEH CRASH ====================\n");
        let pid = unsafe { GetCurrentProcessId() };
        let tid = unsafe { GetCurrentThreadId() };
        let base = unsafe { GetModuleHandleW(ptr::null()) } as usize;
        sb.push_str("PID: ");
        sb.push_u32(pid);
        sb.push_str(" | TID: ");
        sb.push_u32(tid);
        sb.push_str("\n");

        if !info.is_null() {
            let rec = unsafe { (*info).exception_record };
            if !rec.is_null() {
                let code = unsafe { (*rec).exception_code };
                let addr = unsafe { (*rec).exception_address } as usize;
                let rva = addr.wrapping_sub(base);
                sb.push_str("ExceptionCode: ");
                sb.push_hex(code as usize, 8);
                sb.push_str(" (");
                sb.push_str(exception_code_name(code));
                sb.push_str(")\nExceptionAddr: ");
                sb.push_hex(addr, 16);
                sb.push_str("\nImageBase:     ");
                sb.push_hex(base, 16);
                sb.push_str("\nFaulting RVA:  ");
                sb.push_hex(rva, 8);
                sb.push_str("\n");
            }
        }
        sb.push_str("===================================================================\n");
        write_raw_crash_bytes(sb.as_bytes());
        EXCEPTION_CONTINUE_SEARCH
    }

    extern "C" fn sigabrt_handler(_sig: i32) {
        let mut sb = StackBuf::new();
        let pid = unsafe { GetCurrentProcessId() };
        let tid = unsafe { GetCurrentThreadId() };
        sb.push_str("\n==================== MICA TERM CRT SIGABRT ====================\n");
        sb.push_str("PID: ");
        sb.push_u32(pid);
        sb.push_str(" | TID: ");
        sb.push_u32(tid);
        sb.push_str("\nReason: C runtime abort() invoked\n");
        sb.push_str("===============================================================\n");
        write_raw_crash_bytes(sb.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_log_follows_saved_preference_and_console_override() {
        let dir = std::env::temp_dir().join(format!("mica-log-test-{}", uuid::Uuid::new_v4()));
        let paths = AppPaths::from_root(&dir).unwrap();
        let log = RuntimeLog::new(paths.runtime_log_path.clone(), false);
        log.configure(false).unwrap();
        (&log).write_all(b"off\n").unwrap();
        assert!(!paths.runtime_log_path.exists());

        log.configure(true).unwrap();
        (&log).write_all(b"first\n").unwrap();

        log.configure(false).unwrap();
        (&log).write_all(b"off again\n").unwrap();

        log.configure(true).unwrap();
        (&log).write_all(b"resumed\n").unwrap();
        drop(log);

        let console = RuntimeLog::new(paths.runtime_log_path.clone(), true);
        console.configure(false).unwrap();
        (&console).write_all(b"console\n").unwrap();
        let content = fs::read_to_string(&paths.runtime_log_path).unwrap();
        assert!(content.contains("first\n"));
        assert!(!content.contains("off\n"));
        assert!(!content.contains("off again\n"));
        assert!(content.contains("resumed\n"));
        assert!(content.contains("console\n"));
        drop(console);

        let invalid = RuntimeLog::new(dir.clone(), false);
        assert!(invalid.configure(true).is_err());
        assert!(!invalid.enabled.load(Ordering::Acquire));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn recent_logs_reads_crash_and_runtime_tails() {
        let tmp = std::env::temp_dir().join(format!("mica-log-test-{}", uuid::Uuid::new_v4()));
        let paths = AppPaths::from_root(&tmp).expect("create temp AppPaths");
        assert!(read_recent_logs(&paths, 4096).contains("暂无日志内容"));
        fs::create_dir(&paths.runtime_log_path).unwrap();
        assert!(read_recent_logs(&paths, 4096).contains("读取日志失败"));
        fs::remove_dir(&paths.runtime_log_path).unwrap();
        fs::write(&paths.runtime_log_path, "line1: hello\nline2: ssh ok\n").unwrap();
        fs::write(&paths.crash_log_path, "panic: simulated crash\n").unwrap();

        let text = read_recent_logs(&paths, 4096);
        assert!(text.contains("simulated crash"));
        assert!(text.contains("line2: ssh ok"));

        let _ = fs::remove_dir_all(&tmp);
    }
}
