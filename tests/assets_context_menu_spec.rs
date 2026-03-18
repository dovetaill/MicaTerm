use mica_term::shell::context_menu::{ContextTargetKind, resolve_action_tree};

#[test]
fn folder_target_exposes_flat_create_actions() {
    let actions = resolve_action_tree(ContextTargetKind::Folder);

    let new_folder = actions
        .iter()
        .find(|action| action.id == "new-folder")
        .expect("folder target should expose new-folder");
    let new_ssh = actions
        .iter()
        .find(|action| action.id == "new-ssh-connection")
        .expect("folder target should expose new-ssh-connection");

    assert!(new_folder.children.is_empty());
    assert!(new_ssh.children.is_empty());
}
