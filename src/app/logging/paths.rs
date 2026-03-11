use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoggingRootSource {
    EnvOverride,
    PortableMarker,
    StandardLocalData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggingPaths {
    pub root_source: LoggingRootSource,
    pub root_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub crash_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggingPathInputs {
    pub env_log_dir: Option<PathBuf>,
    pub executable_dir: PathBuf,
    pub standard_local_data_dir: PathBuf,
    pub portable_marker_name: &'static str,
}

pub fn resolve_logging_paths(inputs: &LoggingPathInputs) -> Result<LoggingPaths> {
    let (root_source, root_dir) = if let Some(path) = &inputs.env_log_dir {
        (LoggingRootSource::EnvOverride, path.clone())
    } else if inputs
        .executable_dir
        .join(inputs.portable_marker_name)
        .exists()
    {
        (
            LoggingRootSource::PortableMarker,
            inputs.executable_dir.clone(),
        )
    } else {
        (
            LoggingRootSource::StandardLocalData,
            inputs.standard_local_data_dir.clone(),
        )
    };

    let logs_dir = root_dir.join("logs");
    let crash_dir = root_dir.join("crash");
    fs::create_dir_all(&logs_dir)?;
    fs::create_dir_all(&crash_dir)?;

    Ok(LoggingPaths {
        root_source,
        root_dir,
        logs_dir,
        crash_dir,
    })
}

pub fn resolve_logging_paths_for_app() -> Result<LoggingPaths> {
    let project_dirs = ProjectDirs::from("dev", "MicaTerm", "MicaTerm")
        .context("project directories are unavailable")?;
    let executable_dir = std::env::current_exe()?
        .parent()
        .context("executable directory is unavailable")?
        .to_path_buf();
    let env_log_dir = std::env::var_os("MICA_TERM_LOG_DIR").map(PathBuf::from);
    let standard_local_data_dir = project_dirs.data_local_dir().join("MicaTerm");

    resolve_logging_paths(&LoggingPathInputs {
        env_log_dir,
        executable_dir,
        standard_local_data_dir,
        portable_marker_name: ".mica-term-portable",
    })
}
