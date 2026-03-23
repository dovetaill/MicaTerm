//! Minimal OpenSSH-style known_hosts persistence for TOFU checks.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use russh::keys::{HashAlg, PublicKey};

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

    fn load_entries(&self) -> Result<Vec<KnownHostEntry>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&self.path)
            .with_context(|| format!("failed to read known_hosts file `{}`", self.path.display()))?;
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

fn parse_known_host_entry(line: &str) -> Result<KnownHostEntry> {
    let trimmed = line.trim();
    let Some(separator_index) = trimmed.find(char::is_whitespace) else {
        bail!("invalid known_hosts entry: missing public key");
    };

    let host_pattern = trimmed[..separator_index].to_string();
    let key = PublicKey::from_openssh(trimmed[separator_index..].trim()).with_context(|| {
        format!("failed to parse known_hosts public key for `{host_pattern}`")
    })?;

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
