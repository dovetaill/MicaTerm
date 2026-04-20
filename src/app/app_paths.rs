//! Resolves the effective application root directories across override, portable, and standard roots.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppRootSource {
    EnvOverride,
    PortableMarker,
    StandardLocalData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppRootPathInputs {
    pub env_root_dir: Option<PathBuf>,
    pub executable_dir: PathBuf,
    pub standard_local_data_dir: PathBuf,
    pub portable_marker_name: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppRootPaths {
    pub root_source: AppRootSource,
    pub root_dir: PathBuf,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub crash_dir: PathBuf,
}

impl AppRootPaths {
    pub fn keychain_catalog_database_path(&self) -> PathBuf {
        self.data_dir.join("keychain.redb")
    }

    pub fn transfer_database_path(&self) -> PathBuf {
        self.data_dir.join("transfers.redb")
    }
}

pub fn resolve_app_root_paths(inputs: &AppRootPathInputs) -> Result<AppRootPaths> {
    let (root_source, root_dir) = if let Some(path) = &inputs.env_root_dir {
        (AppRootSource::EnvOverride, path.clone())
    } else if inputs
        .executable_dir
        .join(inputs.portable_marker_name)
        .exists()
    {
        (AppRootSource::PortableMarker, inputs.executable_dir.clone())
    } else {
        (
            AppRootSource::StandardLocalData,
            inputs.standard_local_data_dir.clone(),
        )
    };

    let config_dir = root_dir.join("config");
    let data_dir = root_dir.join("data");
    let logs_dir = root_dir.join("logs");
    let crash_dir = root_dir.join("crash");

    fs::create_dir_all(&config_dir)?;
    fs::create_dir_all(&data_dir)?;
    fs::create_dir_all(&logs_dir)?;
    fs::create_dir_all(&crash_dir)?;

    Ok(AppRootPaths {
        root_source,
        root_dir,
        config_dir,
        data_dir,
        logs_dir,
        crash_dir,
    })
}

pub fn standard_app_root_dir_for_app() -> Result<PathBuf> {
    let project_dirs =
        ProjectDirs::from("dev", "", "MicaTerm").context("project directories are unavailable")?;
    let standard_root = project_dirs
        .data_local_dir()
        .parent()
        .context("standard app root directory is unavailable")?;

    Ok(standard_root.to_path_buf())
}

pub fn app_root_paths_for_app() -> Result<AppRootPaths> {
    let executable_dir = std::env::current_exe()?
        .parent()
        .context("executable directory is unavailable")?
        .to_path_buf();

    resolve_app_root_paths(&AppRootPathInputs {
        env_root_dir: std::env::var_os("MICA_TERM_APP_DIR").map(PathBuf::from),
        executable_dir,
        standard_local_data_dir: standard_app_root_dir_for_app()?,
        portable_marker_name: ".mica-term-portable",
    })
}
