use std::{
    env,
    ffi::OsString,
    fs,
    io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShimConfig {
    pub target_os: String,
    pub target_env: String,
    pub host_os: String,
    pub target_arch: String,
    pub out_dir: PathBuf,
    pub xwin_cache_dir: Option<PathBuf>,
    pub home_dir: Option<PathBuf>,
    pub lib_paths: Vec<PathBuf>,
}

impl ShimConfig {
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            target_os: required_env("CARGO_CFG_TARGET_OS")?,
            target_env: required_env("CARGO_CFG_TARGET_ENV")?,
            host_os: env::consts::OS.to_string(),
            target_arch: required_env("CARGO_CFG_TARGET_ARCH")?,
            out_dir: PathBuf::from(required_env("OUT_DIR")?),
            xwin_cache_dir: env::var_os("XWIN_CACHE_DIR").map(PathBuf::from),
            home_dir: env::var_os("HOME").map(PathBuf::from),
            lib_paths: env::var_os("LIB")
                .map(split_search_paths)
                .unwrap_or_default(),
        })
    }
}

pub fn maybe_prepare_advapi32_shim(config: &ShimConfig) -> io::Result<Option<PathBuf>> {
    if !should_prepare_advapi32_shim(config) {
        return Ok(None);
    }

    let Some(source_path) = find_advapi32_source(config)? else {
        return Ok(None);
    };

    let shim_dir = config.out_dir.join("xwin-link-shims");
    fs::create_dir_all(&shim_dir)?;

    let shim_path = shim_dir.join("Advapi32.lib");
    if shim_path != source_path {
        fs::copy(&source_path, &shim_path)?;
    }

    Ok(Some(shim_dir))
}

fn required_env(name: &str) -> Result<String, String> {
    env::var(name).map_err(|_| format!("missing required environment variable `{name}`"))
}

fn should_prepare_advapi32_shim(config: &ShimConfig) -> bool {
    config.target_os == "windows" && config.target_env == "msvc" && config.host_os != "windows"
}

fn find_advapi32_source(config: &ShimConfig) -> io::Result<Option<PathBuf>> {
    for search_dir in candidate_search_dirs(config) {
        if let Some(path) = find_case_insensitive_file(&search_dir, "advapi32.lib")? {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

fn candidate_search_dirs(config: &ShimConfig) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Some(cache_dir) = &config.xwin_cache_dir {
        push_unique(&mut dirs, xwin_sdk_um_dir(cache_dir, &config.target_arch));
    }

    if let Some(home_dir) = &config.home_dir {
        push_unique(
            &mut dirs,
            home_dir
                .join(".cache")
                .join("cargo-xwin")
                .join("xwin")
                .join("sdk")
                .join("lib")
                .join("um")
                .join(msvc_arch_dir(&config.target_arch)),
        );
    }

    for lib_path in &config.lib_paths {
        push_unique(&mut dirs, lib_path.clone());
    }

    dirs
}

fn xwin_sdk_um_dir(cache_dir: &Path, arch: &str) -> PathBuf {
    cache_dir
        .join("xwin")
        .join("sdk")
        .join("lib")
        .join("um")
        .join(msvc_arch_dir(arch))
}

fn msvc_arch_dir(arch: &str) -> &str {
    match arch {
        "x86_64" => "x86_64",
        "x86" | "i686" => "x86",
        "aarch64" => "arm64",
        "arm" => "arm",
        other => other,
    }
}

fn split_search_paths(paths: OsString) -> Vec<PathBuf> {
    env::split_paths(&paths).collect()
}

fn push_unique(dirs: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !dirs.iter().any(|existing| existing == &candidate) {
        dirs.push(candidate);
    }
}

fn find_case_insensitive_file(dir: &Path, file_name: &str) -> io::Result<Option<PathBuf>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };

    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        if name.to_string_lossy().eq_ignore_ascii_case(file_name) {
            return Ok(Some(entry.path()));
        }
    }

    Ok(None)
}
