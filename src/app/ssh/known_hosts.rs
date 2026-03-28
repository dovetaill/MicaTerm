//! Minimal OpenSSH-style known_hosts persistence for TOFU checks.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use directories::ProjectDirs;
use russh::keys::{HashAlg, PublicKey};

use crate::app::vault::model::VaultKnownHostEntry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KnownHostCheck {
    Trusted,
    Unknown { fingerprint: String },
    Changed { expected: String, actual: String },
}

#[derive(Debug, Clone)]
pub struct KnownHostsService {
    path: PathBuf,
}

#[derive(Debug, Clone)]
struct KnownHostEntry {
    host_pattern: String,
    public_key: PublicKey,
}

impl KnownHostsService {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn check(&self, host: &str, port: u16, key: &PublicKey) -> Result<KnownHostCheck> {
        let host_pattern = host_pattern(host, port);
        let actual = fingerprint(key);

        let Some(existing) = self
            .load_entries()?
            .into_iter()
            .find(|entry| entry.host_pattern == host_pattern)
        else {
            return Ok(KnownHostCheck::Unknown {
                fingerprint: actual,
            });
        };

        if existing.public_key == *key {
            Ok(KnownHostCheck::Trusted)
        } else {
            Ok(KnownHostCheck::Changed {
                expected: fingerprint(&existing.public_key),
                actual,
            })
        }
    }

    pub fn accept_unknown(&self, host: &str, port: u16, key: &PublicKey) -> Result<()> {
        let host_pattern = host_pattern(host, port);
        let mut entries = self.load_entries()?;
        entries.retain(|entry| entry.host_pattern != host_pattern);
        entries.push(KnownHostEntry {
            host_pattern,
            public_key: key.clone(),
        });
        self.save_entries(&entries)
    }

    pub fn ensure_trusted(&self, host: &str, port: u16, key: &PublicKey) -> Result<()> {
        match self.check(host, port, key)? {
            KnownHostCheck::Trusted => Ok(()),
            KnownHostCheck::Unknown { fingerprint } => bail!(
                "unknown SSH host key for `{host}:{port}` ({fingerprint}); explicit accept is required before connecting"
            ),
            KnownHostCheck::Changed { expected, actual } => bail!(
                "SSH host key changed for `{host}:{port}` (expected {expected}, got {actual})"
            ),
        }
    }

    pub fn export_snapshot_entries(&self) -> Result<Vec<VaultKnownHostEntry>> {
        self.load_entries()?
            .into_iter()
            .map(|entry| {
                Ok(VaultKnownHostEntry {
                    host_pattern: entry.host_pattern,
                    public_key: entry.public_key.to_openssh().with_context(|| {
                        "failed to encode known_hosts entry for vault snapshot"
                    })?,
                })
            })
            .collect()
    }

    pub fn replace_snapshot_entries(&self, entries: &[VaultKnownHostEntry]) -> Result<()> {
        let entries = entries
            .iter()
            .map(|entry| {
                let public_key = PublicKey::from_openssh(entry.public_key.as_str()).with_context(
                    || {
                        format!(
                            "failed to parse known_hosts public key for `{}`",
                            entry.host_pattern
                        )
                    },
                )?;
                Ok(KnownHostEntry {
                    host_pattern: entry.host_pattern.clone(),
                    public_key,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        self.save_entries(entries.as_slice())
    }

    fn load_entries(&self) -> Result<Vec<KnownHostEntry>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&self.path).with_context(|| {
            format!("failed to read known_hosts file `{}`", self.path.display())
        })?;
        content
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() && !trimmed.starts_with('#')
            })
            .map(parse_known_host_entry)
            .collect()
    }

    fn save_entries(&self, entries: &[KnownHostEntry]) -> Result<()> {
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create parent directory for known_hosts file `{}`",
                    self.path.display()
                )
            })?;
        }

        let mut body = String::new();
        for entry in entries {
            let encoded_key = entry.public_key.to_openssh().with_context(|| {
                format!(
                    "failed to encode known_hosts entry for `{}`",
                    entry.host_pattern
                )
            })?;
            body.push_str(&entry.host_pattern);
            body.push(' ');
            body.push_str(&encoded_key);
            body.push('\n');
        }

        fs::write(&self.path, body)
            .with_context(|| format!("failed to write known_hosts file `{}`", self.path.display()))
    }
}

pub fn default_known_hosts_path() -> Result<PathBuf> {
    if let Some(override_path) = env::var_os("MICA_TERM_KNOWN_HOSTS_PATH") {
        return Ok(PathBuf::from(override_path));
    }

    let project_dirs = ProjectDirs::from("dev", "MicaTerm", "MicaTerm")
        .context("failed to resolve default project directories for known_hosts")?;
    let data_dir = project_dirs.data_local_dir().join("MicaTerm").join("data");
    fs::create_dir_all(&data_dir).with_context(|| {
        format!(
            "failed to create known_hosts data dir `{}`",
            data_dir.display()
        )
    })?;
    Ok(data_dir.join("known_hosts"))
}

fn parse_known_host_entry(line: &str) -> Result<KnownHostEntry> {
    let trimmed = line.trim();
    let Some(separator_index) = trimmed.find(char::is_whitespace) else {
        bail!("invalid known_hosts entry: missing public key");
    };

    let host_pattern = trimmed[..separator_index].to_string();
    let key = PublicKey::from_openssh(trimmed[separator_index..].trim())
        .with_context(|| format!("failed to parse known_hosts public key for `{host_pattern}`"))?;

    Ok(KnownHostEntry {
        host_pattern,
        public_key: key,
    })
}

fn host_pattern(host: &str, port: u16) -> String {
    format!("[{host}]:{port}")
}

fn fingerprint(key: &PublicKey) -> String {
    key.fingerprint(HashAlg::Sha256).to_string()
}
