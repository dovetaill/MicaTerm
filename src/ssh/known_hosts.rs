//! OpenSSH-compatible `known_hosts` store with SHA-256 fingerprint verification.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use russh::keys::{HashAlg, PublicKey};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KnownHostStatus {
    Trusted,
    Unknown {
        fingerprint: String,
        openssh_key: String,
    },
    Changed {
        expected_fingerprint: String,
        actual_fingerprint: String,
    },
}

#[derive(Debug, Clone)]
pub struct KnownHostsStore {
    path: PathBuf,
}

impl KnownHostsStore {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn verify(&self, host: &str, port: u16, key: &PublicKey) -> Result<KnownHostStatus> {
        let pattern = host_pattern(host, port);
        let actual_fp = key.fingerprint(HashAlg::Sha256).to_string();
        let openssh_key = key
            .to_openssh()
            .context("编码服务端 SSH 公钥失败")?;

        let entries = self.read_all()?;
        let Some((_, saved_key)) = entries.into_iter().find(|(h, _)| h == &pattern) else {
            return Ok(KnownHostStatus::Unknown {
                fingerprint: actual_fp,
                openssh_key,
            });
        };

        if &saved_key == key {
            Ok(KnownHostStatus::Trusted)
        } else {
            Ok(KnownHostStatus::Changed {
                expected_fingerprint: saved_key.fingerprint(HashAlg::Sha256).to_string(),
                actual_fingerprint: actual_fp,
            })
        }
    }

    pub fn trust_openssh_key(&self, host: &str, port: u16, openssh_key: &str) -> Result<()> {
        let parsed = PublicKey::from_openssh(openssh_key)
            .context("解析待信任的 SSH 公钥失败")?;
        let pattern = host_pattern(host, port);
        let mut entries = self.read_all()?;
        entries.retain(|(h, _)| h != &pattern);
        entries.push((pattern, parsed));
        self.write_all(&entries)
    }

    fn read_all(&self) -> Result<Vec<(String, PublicKey)>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(&self.path)?;
        let mut out = Vec::new();
        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some(sep) = trimmed.find(char::is_whitespace) {
                let host_part = trimmed[..sep].to_string();
                let key_part = trimmed[sep..].trim();
                if let Ok(pk) = PublicKey::from_openssh(key_part) {
                    out.push((host_part, pk));
                }
            }
        }
        Ok(out)
    }

    fn write_all(&self, entries: &[(String, PublicKey)]) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut body = String::new();
        for (pattern, pk) in entries {
            if let Ok(encoded) = pk.to_openssh() {
                body.push_str(pattern);
                body.push(' ');
                body.push_str(&encoded);
                body.push('\n');
            }
        }
        fs::write(&self.path, body)?;
        Ok(())
    }
}

fn host_pattern(host: &str, port: u16) -> String {
    format!("[{}]:{}", host.trim(), port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_hosts_tofu_and_mismatch_detection() {
        let path = std::env::temp_dir().join(format!("mica-kh-{}", uuid::Uuid::new_v4()));
        let store = KnownHostsStore::new(&path);

        let pub1 = PublicKey::from_openssh(
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILM+rvN+ot98qgEN796jTiQfZfG1KaT0PtFDJ/XFSqti test1",
        )
        .unwrap();

        let status = store.verify("example.com", 22, &pub1).unwrap();
        let openssh = match status {
            KnownHostStatus::Unknown { openssh_key, .. } => openssh_key,
            other => panic!("expected unknown, got {other:?}"),
        };

        store.trust_openssh_key("example.com", 22, &openssh).unwrap();
        assert_eq!(
            store.verify("example.com", 22, &pub1).unwrap(),
            KnownHostStatus::Trusted
        );

        let pub2 = PublicKey::from_openssh(
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIK0wmN/Cr3JXab+7u8vH/A+pb43c/325+b3n2kP7xY8a test2",
        )
        .unwrap();
        assert!(matches!(
            store.verify("example.com", 22, &pub2).unwrap(),
            KnownHostStatus::Changed { .. }
        ));

        let _ = fs::remove_file(path);
    }
}
