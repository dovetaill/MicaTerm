use std::fs;

#[test]
fn workspace_pane_source_branches_to_sftp_workspace_host() {
    let workspace_pane =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");

    assert!(
        workspace_pane.contains("import { SftpWorkspaceHost } from \"./sftp-workspace-host.slint\";"),
        "WorkspacePane should import the dedicated SFTP workspace host"
    );
    assert!(
        workspace_pane.contains("if root.workspace-session-host-mode == \"sftp\" : sftp-host := SftpWorkspaceHost {"),
        "WorkspacePane should switch to SftpWorkspaceHost when the active workspace tab is an sftp tab"
    );
    assert!(
        workspace_pane.contains("session-title: root.workspace-session-title;")
            && workspace_pane.contains("session-subtitle: root.workspace-session-subtitle;")
            && workspace_pane.contains("session-state: root.workspace-session-state;"),
        "WorkspacePane should forward the active workspace title/subtitle/state into the SFTP workspace host"
    );
}

#[test]
fn sftp_workspace_host_source_exposes_core_file_table_headers() {
    let source = fs::read_to_string("ui/shell/sftp-workspace-host.slint")
        .expect("read sftp workspace host");

    assert!(
        source.contains("export component SftpWorkspaceHost"),
        "SFTP workspace host should live in its own component"
    );
    for label in ["Name", "Type", "Size", "Modified"] {
        assert!(
            source.contains(&format!("text: \"{label}\""))
                || source.contains(&format!("label: \"{label}\"")),
            "SFTP workspace host should expose a `{label}` table header"
        );
    }
    assert!(
        source.contains("session-title")
            && source.contains("Files workspace")
            && source.contains("Open a Quick Browser"),
        "SFTP workspace host should render a lightweight empty-state shell around the workspace title"
    );
}
