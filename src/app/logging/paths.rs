//! Resolves the effective logging directories across portable, override, and standard app roots.

use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;

use crate::app::app_paths::{AppRootPathInputs, AppRootSource, resolve_app_root_paths};

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
    pub data_dir: PathBuf,
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
    let app_paths = resolve_app_root_paths(&AppRootPathInputs {
        env_root_dir: inputs.env_log_dir.clone(),
        executable_dir: inputs.executable_dir.clone(),
        standard_local_data_dir: inputs.standard_local_data_dir.clone(),
        portable_marker_name: inputs.portable_marker_name,
    })?;

    Ok(LoggingPaths {
        root_source: map_root_source(app_paths.root_source),
        root_dir: app_paths.root_dir,
        data_dir: app_paths.data_dir,
        logs_dir: app_paths.logs_dir,
        crash_dir: app_paths.crash_dir,
    })
}

fn map_root_source(source: AppRootSource) -> LoggingRootSource {
    match source {
        AppRootSource::EnvOverride => LoggingRootSource::EnvOverride,
        AppRootSource::PortableMarker => LoggingRootSource::PortableMarker,
        AppRootSource::StandardLocalData => LoggingRootSource::StandardLocalData,
    }
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
