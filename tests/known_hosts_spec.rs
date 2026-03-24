use std::fs;
use std::path::PathBuf;

use mica_term::app::ssh::known_hosts::{KnownHostCheck, KnownHostsService};
use russh::keys::{HashAlg, PublicKey};

fn sample_key_path(label: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "mica-term-known-hosts-{}-{}.txt",
        label,
        std::process::id()
    ));
    path
}

fn sample_public_key(seed: u8) -> PublicKey {
    match seed {
        1 => PublicKey::from_openssh(
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILM+rvN+ot98qgEN796jTiQfZfG1KaT0PtFDJ/XFSqti test-1@example.com",
        )
        .expect("parse public key seed 1"),
        _ => PublicKey::from_openssh(
            "ecdsa-sha2-nistp256 AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIbmlzdHAyNTYAAABBBHwf2HMM5TRXvo2SQJjsNkiDD5KqiiNjrGVv3UUh+mMT5RHxiRtOnlqvjhQtBq0VpmpCV/PwUdhOig4vkbqAcEc= test-2@example.com",
        )
        .expect("parse public key seed 2"),
    }
}

#[test]
fn known_hosts_service_reports_unknown_host_for_first_contact() {
    let path = sample_key_path("unknown");
    let _ = fs::remove_file(&path);
    let service = KnownHostsService::new(&path);
    let key = sample_public_key(1);

    let result = service
        .check("example.com", 22, &key)
        .expect("check unknown host");

    match result {
        KnownHostCheck::Unknown { fingerprint } => {
            assert_eq!(fingerprint, key.fingerprint(HashAlg::Sha256).to_string());
        }
        other => panic!("expected unknown host result, got {other:?}"),
    }
}

#[test]
fn known_hosts_service_accepts_and_persists_tofu_entry() {
    let path = sample_key_path("accept");
    let _ = fs::remove_file(&path);
    let service = KnownHostsService::new(&path);
    let key = sample_public_key(1);

    service
        .accept_unknown("example.com", 22, &key)
        .expect("persist accepted key");

    let result = service
        .check("example.com", 22, &key)
        .expect("check trusted host");

    assert!(matches!(result, KnownHostCheck::Trusted));
}

#[test]
fn known_hosts_service_rejects_changed_host_key() {
    let path = sample_key_path("changed");
    let _ = fs::remove_file(&path);
    let service = KnownHostsService::new(&path);
    let original_key = sample_public_key(1);
    let changed_key = sample_public_key(2);

    service
        .accept_unknown("example.com", 22, &original_key)
        .expect("persist original key");

    let result = service
        .check("example.com", 22, &changed_key)
        .expect("check changed key");

    match result {
        KnownHostCheck::Changed { expected, actual } => {
            assert_eq!(
                expected,
                original_key.fingerprint(HashAlg::Sha256).to_string()
            );
            assert_eq!(actual, changed_key.fingerprint(HashAlg::Sha256).to_string());
        }
        other => panic!("expected changed host key result, got {other:?}"),
    }
}

#[test]
fn unknown_host_requires_explicit_accept_before_connect_can_continue() {
    let path = sample_key_path("ensure-trusted");
    let _ = fs::remove_file(&path);
    let service = KnownHostsService::new(&path);
    let key = sample_public_key(1);

    let initial = service.ensure_trusted("example.com", 22, &key);
    assert!(initial.is_err(), "unknown host should block connect");

    service
        .accept_unknown("example.com", 22, &key)
        .expect("accept unknown host");

    service
        .ensure_trusted("example.com", 22, &key)
        .expect("trusted host should continue after explicit accept");
}
