use mica_term::app::ssh::profile::{ConnectionProfile, SshAuthMethod};
use mica_term::shell::view_model::AssetSshConnectionDraft;

fn base_draft() -> AssetSshConnectionDraft {
    AssetSshConnectionDraft {
        name: "Prod Bastion".into(),
        host: "10.0.0.12".into(),
        user: "ops".into(),
        port: "2022".into(),
        remark: "Primary entry point".into(),
        ..AssetSshConnectionDraft::default()
    }
}

#[test]
fn ssh_profile_normalizes_password_mode_from_modal_draft() {
    let mut draft = base_draft();
    draft.auth_method = "password".into();
    draft.password = "super-secret".into();

    let profile = ConnectionProfile::from_draft(&draft).expect("normalize password draft");

    assert_eq!(profile.asset_id, None);
    assert_eq!(profile.name, "Prod Bastion");
    assert_eq!(profile.host, "10.0.0.12");
    assert_eq!(profile.user, "ops");
    assert_eq!(profile.port, 2022);
    assert_eq!(profile.auth_method, SshAuthMethod::Password);
    assert!(profile.credential_ref.is_some());
    assert_eq!(profile.private_key_path, None);
    assert_eq!(profile.remark, "Primary entry point");
}

#[test]
fn ssh_profile_normalizes_private_key_path_mode_from_modal_draft() {
    let mut draft = base_draft();
    draft.auth_method = "private-key".into();
    draft.private_key_source = "path".into();
    draft.private_key_path = "/tmp/id_ed25519".into();

    let profile = ConnectionProfile::from_draft(&draft).expect("normalize key path draft");

    assert_eq!(profile.auth_method, SshAuthMethod::PrivateKeyPath);
    assert_eq!(profile.credential_ref, None);
    assert_eq!(profile.private_key_path.as_deref(), Some("/tmp/id_ed25519"));
    assert_eq!(profile.port, 2022);
}

#[test]
fn ssh_profile_normalizes_private_key_content_mode_from_modal_draft() {
    let mut draft = base_draft();
    draft.auth_method = "private-key".into();
    draft.private_key_source = "content".into();
    draft.private_key_content = "-----BEGIN OPENSSH PRIVATE KEY-----".into();
    draft.passphrase = "phrase".into();

    let profile = ConnectionProfile::from_draft(&draft).expect("normalize inline key draft");

    assert_eq!(profile.auth_method, SshAuthMethod::PrivateKeyContent);
    assert!(profile.credential_ref.is_some());
    assert_eq!(profile.private_key_path, None);
    assert_eq!(profile.remark, "Primary entry point");
}
