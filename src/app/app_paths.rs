//! Resolves the effective application root directories across override, portable, and standard roots.

use std::fs;
use std::path::PathBuf;

use anyhow::Result;

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
    pub data_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub crash_dir: PathBuf,
}

impl AppRootPaths {
    pub fn keychain_catalog_database_path(&self) -> PathBuf {
        self.data_dir.join("keychain.redb")
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

    let data_dir = root_dir.join("data");
    let logs_dir = root_dir.join("logs");
    let crash_dir = root_dir.join("crash");

    fs::create_dir_all(&data_dir)?;
    fs::create_dir_all(&logs_dir)?;
    fs::create_dir_all(&crash_dir)?;

    Ok(AppRootPaths {
        root_source,
        root_dir,
        data_dir,
        logs_dir,
        crash_dir,
    })
}
