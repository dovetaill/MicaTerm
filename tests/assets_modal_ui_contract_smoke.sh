#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
CREATE_MENU="$ROOT_DIR/ui/components/assets-create-menu.slint"
FOLDER_MODAL="$ROOT_DIR/ui/components/assets-folder-create-modal.slint"
SSH_MODAL="$ROOT_DIR/ui/components/assets-ssh-connection-modal.slint"
SYNC_MODAL="$ROOT_DIR/ui/components/sync-vault-modal.slint"
RENAME_MODAL="$ROOT_DIR/ui/components/assets-rename-modal.slint"
DELETE_MODAL="$ROOT_DIR/ui/components/assets-delete-confirm-modal.slint"
SNIPPET_MODAL="$ROOT_DIR/ui/components/assets-snippet-modal.slint"
SNIPPET_PACKAGE_MODAL="$ROOT_DIR/ui/components/assets-snippet-package-modal.slint"
MODAL_SHELL="$ROOT_DIR/ui/components/blocking-modal-shell.slint"
BOOTSTRAP="$ROOT_DIR/src/app/bootstrap.rs"
TOKENS="$ROOT_DIR/ui/theme/tokens.slint"
ASSETS_SIDEBAR="$ROOT_DIR/ui/shell/assets-sidebar.slint"
SIDEBAR="$ROOT_DIR/ui/shell/sidebar.slint"

grep -F 'in-out property <bool> asset-modal-open: false;' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <bool> sync-modal-open: false;' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> sync-modal-mode: "not-configured";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> sync-modal-title: "Sync Settings";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> sync-modal-headline: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> sync-modal-status-text: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> sync-modal-error-text: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> sync-modal-provider-label: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> sync-modal-target-label: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> sync-modal-primary-action-label: "Save and enable";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> sync-modal-secondary-action-label: "Close";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <bool> sync-modal-auto-sync-enabled: false;' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> sync-modal-primary-gist-id: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> sync-modal-primary-pat: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <bool> sync-modal-mirror-enabled: false;' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> sync-modal-mirror-gist-id: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> sync-modal-mirror-pat: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> sync-modal-master-password: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-modal-kind: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <int> asset-modal-focus-sequence: 0;' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-snippet-modal-name: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-snippet-modal-script: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-snippet-modal-package: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <[string]> asset-snippet-modal-package-options: [];' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-snippet-modal-package-selected-label: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-snippet-package-modal-name: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <bool> asset-rename-modal-open: false;' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-rename-modal-name: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-rename-modal-validation-message: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <bool> asset-rename-modal-can-confirm: false;' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <bool> asset-delete-confirm-modal-open: false;' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-delete-confirm-target-label: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <int> asset-delete-confirm-descendant-count: 0;' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-ssh-modal-proxy-type: "none";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-ssh-modal-proxy-socks5-host: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-ssh-modal-proxy-socks5-port: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-ssh-modal-proxy-socks5-username: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-ssh-modal-proxy-socks5-password: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <bool> asset-ssh-modal-proxy-socks5-password-visible: false;' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-ssh-modal-proxy-ssh-asset-id: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <[string]> asset-ssh-modal-proxy-ssh-options: [];' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-ssh-modal-proxy-ssh-selected-label: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-ssh-modal-auth-source: "manual";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <[string]> asset-ssh-modal-keychain-identity-options: [];' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-ssh-modal-keychain-identity-selected-label: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-ssh-modal-keychain-identity-username: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-ssh-modal-keychain-identity-auth-summary: "";' "$APP_WINDOW" >/dev/null
grep -F 'callback drag-requested(length, length);' "$MODAL_SHELL" >/dev/null
! grep -F 'in property <string> dialog-title: "";' "$MODAL_SHELL" >/dev/null
! grep -F 'callback close-requested();' "$MODAL_SHELL" >/dev/null
! grep -F 'header := Rectangle {' "$MODAL_SHELL" >/dev/null
! grep -F 'close-button := Rectangle {' "$MODAL_SHELL" >/dev/null
grep -F 'clicked => { }' "$APP_WINDOW" >/dev/null
grep -F 'callback close-asset-modal-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback sync-modal-close-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback sync-modal-primary-action-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback sync-modal-secondary-action-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback sync-modal-draft-changed(string, string);' "$APP_WINDOW" >/dev/null
grep -F 'callback sync-modal-toggle-changed(string, bool);' "$APP_WINDOW" >/dev/null
grep -F 'callback confirm-asset-modal-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback asset-snippet-modal-draft-changed(string, string);' "$APP_WINDOW" >/dev/null
grep -F 'callback asset-snippet-package-modal-name-changed(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback asset-rename-modal-name-changed(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback confirm-asset-rename-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback confirm-delete-asset-requested();' "$APP_WINDOW" >/dev/null
grep -F 'host-titlebar-height: titlebar.height;' "$APP_WINDOW" >/dev/null
grep -F 'if root.sync-modal-open : sync-modal-shell := BlockingModalShell {' "$APP_WINDOW" >/dev/null
grep -F 'sync-vault-modal-overlay := SyncVaultModal {' "$APP_WINDOW" >/dev/null
grep -F -A4 'sync-vault-modal-overlay := SyncVaultModal {' "$APP_WINDOW" | grep -F 'width: sync-modal-shell.content-width;' >/dev/null
grep -F -A5 'sync-vault-modal-overlay := SyncVaultModal {' "$APP_WINDOW" | grep -F 'height: sync-modal-shell.content-height;' >/dev/null
grep -F 'modal-height: 230px;' "$APP_WINDOW" >/dev/null
grep -F 'modal-height: 520px;' "$APP_WINDOW" >/dev/null
grep -F 'modal-height: 230px;' "$APP_WINDOW" >/dev/null
grep -F 'modal-height: 620px;' "$APP_WINDOW" >/dev/null
grep -F 'modal-height: 268px;' "$APP_WINDOW" >/dev/null
grep -F 'modal-height: 332px;' "$APP_WINDOW" >/dev/null
grep -F 'asset-folder-modal-overlay := AssetsFolderCreateModal {' "$APP_WINDOW" >/dev/null
grep -F 'asset-snippet-modal-overlay := AssetsSnippetModal {' "$APP_WINDOW" >/dev/null
grep -F 'asset-snippet-package-modal-overlay := AssetsSnippetPackageModal {' "$APP_WINDOW" >/dev/null
grep -F 'asset-ssh-modal-overlay := AssetsSshConnectionModal {' "$APP_WINDOW" >/dev/null
grep -F 'asset-rename-modal-overlay := AssetsRenameModal {' "$APP_WINDOW" >/dev/null
grep -F 'asset-delete-confirm-modal-overlay := AssetsDeleteConfirmModal {' "$APP_WINDOW" >/dev/null
grep -F 'ssh-host-key-modal-overlay := SshHostKeyConfirmModal {' "$APP_WINDOW" >/dev/null
grep -F -A4 'asset-folder-modal-overlay := AssetsFolderCreateModal {' "$APP_WINDOW" | grep -F 'width: asset-folder-modal-shell.content-width;' >/dev/null
grep -F -A5 'asset-folder-modal-overlay := AssetsFolderCreateModal {' "$APP_WINDOW" | grep -F 'height: asset-folder-modal-shell.content-height;' >/dev/null
grep -F -A4 'asset-snippet-modal-overlay := AssetsSnippetModal {' "$APP_WINDOW" | grep -F 'width: asset-snippet-modal-shell.content-width;' >/dev/null
grep -F -A5 'asset-snippet-modal-overlay := AssetsSnippetModal {' "$APP_WINDOW" | grep -F 'height: asset-snippet-modal-shell.content-height;' >/dev/null
grep -F -A4 'asset-snippet-package-modal-overlay := AssetsSnippetPackageModal {' "$APP_WINDOW" | grep -F 'width: asset-snippet-package-modal-shell.content-width;' >/dev/null
grep -F -A5 'asset-snippet-package-modal-overlay := AssetsSnippetPackageModal {' "$APP_WINDOW" | grep -F 'height: asset-snippet-package-modal-shell.content-height;' >/dev/null
grep -F -A4 'asset-ssh-modal-overlay := AssetsSshConnectionModal {' "$APP_WINDOW" | grep -F 'width: asset-ssh-modal-shell.content-width;' >/dev/null
grep -F -A5 'asset-ssh-modal-overlay := AssetsSshConnectionModal {' "$APP_WINDOW" | grep -F 'height: asset-ssh-modal-shell.content-height;' >/dev/null
grep -F -A4 'asset-rename-modal-overlay := AssetsRenameModal {' "$APP_WINDOW" | grep -F 'width: asset-rename-modal-shell.content-width;' >/dev/null
grep -F -A5 'asset-rename-modal-overlay := AssetsRenameModal {' "$APP_WINDOW" | grep -F 'height: asset-rename-modal-shell.content-height;' >/dev/null
grep -F -A4 'asset-delete-confirm-modal-overlay := AssetsDeleteConfirmModal {' "$APP_WINDOW" | grep -F 'width: asset-delete-confirm-modal-shell.content-width;' >/dev/null
grep -F -A5 'asset-delete-confirm-modal-overlay := AssetsDeleteConfirmModal {' "$APP_WINDOW" | grep -F 'height: asset-delete-confirm-modal-shell.content-height;' >/dev/null
grep -F -A4 'ssh-host-key-modal-overlay := SshHostKeyConfirmModal {' "$APP_WINDOW" | grep -F 'width: ssh-host-key-modal-shell.content-width;' >/dev/null
grep -F -A5 'ssh-host-key-modal-overlay := SshHostKeyConfirmModal {' "$APP_WINDOW" | grep -F 'height: ssh-host-key-modal-shell.content-height;' >/dev/null
grep -F 'x: 0px;' "$APP_WINDOW" >/dev/null
grep -F 'y: 0px;' "$APP_WINDOW" >/dev/null
grep -F 'export component AssetsSnippetModal inherits Rectangle {' "$SNIPPET_MODAL" >/dev/null
grep -F 'export component SyncVaultModal inherits Rectangle {' "$SYNC_MODAL" >/dev/null
grep -F 'import { ScrollView } from "std-widgets.slint";' "$SYNC_MODAL" >/dev/null
grep -F 'in property <string> mode: "not-configured";' "$SYNC_MODAL" >/dev/null
grep -F 'in property <string> title: "Sync Settings";' "$SYNC_MODAL" >/dev/null
grep -F 'in property <bool> auto-sync-enabled: false;' "$SYNC_MODAL" >/dev/null
grep -F 'in property <string> primary-gist-id: "";' "$SYNC_MODAL" >/dev/null
grep -F 'in property <bool> mirror-enabled: false;' "$SYNC_MODAL" >/dev/null
grep -F 'in property <string> master-password: "";' "$SYNC_MODAL" >/dev/null
grep -F 'callback close-requested();' "$SYNC_MODAL" >/dev/null
grep -F 'callback primary-action-requested();' "$SYNC_MODAL" >/dev/null
grep -F 'callback secondary-action-requested();' "$SYNC_MODAL" >/dev/null
grep -F 'callback draft-changed(string, string);' "$SYNC_MODAL" >/dev/null
grep -F 'callback toggle-changed(string, bool);' "$SYNC_MODAL" >/dev/null
grep -F 'body-scroll := ScrollView {' "$SYNC_MODAL" >/dev/null
grep -F 'viewport-width: scroll-body.width;' "$SYNC_MODAL" >/dev/null
grep -F 'viewport-height: scroll-body.height;' "$SYNC_MODAL" >/dev/null
grep -F 'height: max(body-scroll.visible-height, body-column.preferred-height + 24px);' "$SYNC_MODAL" >/dev/null
grep -F 'footer := Rectangle {' "$SYNC_MODAL" >/dev/null
grep -F 'y: parent.height - root.footer-height;' "$SYNC_MODAL" >/dev/null
grep -F 'error-banner := Rectangle {' "$SYNC_MODAL" >/dev/null
grep -F 'SyncModalToggleRow {' "$SYNC_MODAL" >/dev/null
grep -F 'SyncModalTextField {' "$SYNC_MODAL" >/dev/null
grep -F 'if root.mode == "not-configured" || root.mode == "locked"' "$SYNC_MODAL" >/dev/null
grep -F 'export component AssetsSnippetPackageModal inherits Rectangle {' "$SNIPPET_PACKAGE_MODAL" >/dev/null
grep -F 'export component AssetsFolderCreateModal inherits Rectangle {' "$FOLDER_MODAL" >/dev/null
grep -F 'export component AssetsSshConnectionModal inherits Rectangle {' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> dialog-title: "New Snippet";' "$SNIPPET_MODAL" >/dev/null
grep -F 'import { ComboBox, ScrollView } from "std-widgets.slint";' "$SNIPPET_MODAL" >/dev/null
grep -F 'in property <[string]> package-options: [];' "$SNIPPET_MODAL" >/dev/null
grep -F 'in property <string> package-selected-label: "";' "$SNIPPET_MODAL" >/dev/null
grep -F 'in property <string> dialog-title: "New Package";' "$SNIPPET_PACKAGE_MODAL" >/dev/null
grep -F 'text: "Name";' "$SNIPPET_MODAL" >/dev/null
grep -F 'text: "Script";' "$SNIPPET_MODAL" >/dev/null
grep -F 'text: "Package";' "$SNIPPET_MODAL" >/dev/null
grep -F 'text: "Package name";' "$SNIPPET_PACKAGE_MODAL" >/dev/null
grep -F 'body-scroll := ScrollView {' "$SNIPPET_MODAL" >/dev/null
grep -F 'ComboBox {' "$SNIPPET_MODAL" >/dev/null
grep -F 'model: root.package-options;' "$SNIPPET_MODAL" >/dev/null
grep -F 'current-value: root.package-selected-label;' "$SNIPPET_MODAL" >/dev/null
grep -F 'footer-content := VerticalLayout {' "$SNIPPET_MODAL" >/dev/null
! grep -F 'package-input := TextInput {' "$SNIPPET_MODAL" >/dev/null
grep -F 'callback draft-changed(string, string);' "$SNIPPET_MODAL" >/dev/null
grep -F 'callback name-changed(string);' "$SNIPPET_PACKAGE_MODAL" >/dev/null
grep -F 'in property <string> validation-message: "";' "$SNIPPET_MODAL" >/dev/null
grep -F 'in property <bool> can-confirm: false;' "$SNIPPET_MODAL" >/dev/null
grep -F 'in property <string> validation-message: "";' "$SNIPPET_PACKAGE_MODAL" >/dev/null
grep -F 'in property <bool> can-confirm: false;' "$SNIPPET_PACKAGE_MODAL" >/dev/null
grep -F 'drag-touch := TouchArea {' "$SNIPPET_MODAL" >/dev/null
grep -F 'close-button := Rectangle {' "$SNIPPET_MODAL" >/dev/null
grep -F 'drag-touch := TouchArea {' "$SNIPPET_PACKAGE_MODAL" >/dev/null
grep -F 'close-button := Rectangle {' "$SNIPPET_PACKAGE_MODAL" >/dev/null
grep -F 'in property <string> dialog-title: "New Folder";' "$FOLDER_MODAL" >/dev/null
grep -F 'header := Rectangle {' "$FOLDER_MODAL" >/dev/null
grep -F 'x: 0px;' "$FOLDER_MODAL" >/dev/null
grep -F 'y: 0px;' "$FOLDER_MODAL" >/dev/null
grep -F 'footer := Rectangle {' "$FOLDER_MODAL" >/dev/null
grep -F 'footer-content := VerticalLayout {' "$FOLDER_MODAL" >/dev/null
if grep -F 'footer-panel := Rectangle {' "$FOLDER_MODAL" >/dev/null; then
  echo "folder modal footer must not use an inner footer-panel wrapper" >&2
  exit 1
fi
if grep -F 'footer-divider := Rectangle {' "$FOLDER_MODAL" >/dev/null; then
  echo "folder modal footer must not draw a dedicated footer divider" >&2
  exit 1
fi
grep -F 'drag-touch := TouchArea {' "$FOLDER_MODAL" >/dev/null
grep -F 'close-button := Rectangle {' "$FOLDER_MODAL" >/dev/null
grep -F 'in property <string> dialog-title: "Rename Asset";' "$RENAME_MODAL" >/dev/null
grep -F 'footer := Rectangle {' "$RENAME_MODAL" >/dev/null
grep -F 'drag-touch := TouchArea {' "$RENAME_MODAL" >/dev/null
grep -F 'close-button := Rectangle {' "$RENAME_MODAL" >/dev/null
grep -F 'in property <string> dialog-title: "Delete Asset";' "$DELETE_MODAL" >/dev/null
grep -F 'footer := Rectangle {' "$DELETE_MODAL" >/dev/null
grep -F 'drag-touch := TouchArea {' "$DELETE_MODAL" >/dev/null
grep -F 'close-button := Rectangle {' "$DELETE_MODAL" >/dev/null
grep -F 'in property <string> dialog-title: "Verify Host Key";' "$ROOT_DIR/ui/components/ssh-host-key-confirm-modal.slint" >/dev/null
grep -F 'footer := Rectangle {' "$ROOT_DIR/ui/components/ssh-host-key-confirm-modal.slint" >/dev/null
grep -F 'drag-touch := TouchArea {' "$ROOT_DIR/ui/components/ssh-host-key-confirm-modal.slint" >/dev/null
grep -F 'close-button := Rectangle {' "$ROOT_DIR/ui/components/ssh-host-key-confirm-modal.slint" >/dev/null
grep -F 'in-out property <string> asset-modal-validation-message: "";' "$APP_WINDOW" >/dev/null
grep -F 'in property <string> validation-message: "";' "$FOLDER_MODAL" >/dev/null
grep -F 'in property <bool> can-confirm: false;' "$FOLDER_MODAL" >/dev/null
grep -F 'export component AssetsRenameModal inherits Rectangle {' "$RENAME_MODAL" >/dev/null
grep -F 'export component AssetsDeleteConfirmModal inherits Rectangle {' "$DELETE_MODAL" >/dev/null
grep -F 'in property <string> dialog-title: "New SSH Connection";' "$SSH_MODAL" >/dev/null
grep -F 'header := Rectangle {' "$SSH_MODAL" >/dev/null
grep -F 'text: "Basic";' "$SSH_MODAL" >/dev/null
grep -F 'text: "Authentication";' "$SSH_MODAL" >/dev/null
grep -F 'text: "Proxy";' "$SSH_MODAL" >/dev/null
if grep -F 'text: "Connection Options";' "$SSH_MODAL" >/dev/null; then
  exit 1
fi
grep -F 'text: "Notes";' "$SSH_MODAL" >/dev/null
grep -F 'footer := Rectangle {' "$SSH_MODAL" >/dev/null
grep -F 'footer-content := VerticalLayout {' "$SSH_MODAL" >/dev/null
if grep -F 'footer-panel := Rectangle {' "$SSH_MODAL" >/dev/null; then
  echo "ssh modal footer must not use an inner footer-panel wrapper" >&2
  exit 1
fi
if grep -F 'footer-divider := Rectangle {' "$SSH_MODAL" >/dev/null; then
  echo "ssh modal footer must not draw a dedicated footer divider" >&2
  exit 1
fi
grep -F 'drag-touch := TouchArea {' "$SSH_MODAL" >/dev/null
grep -F 'close-button := Rectangle {' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> validation-message: "";' "$SSH_MODAL" >/dev/null
grep -F 'in property <bool> can-confirm: false;' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> auth-source: "manual";' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> auth-method:' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> private-key-source:' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> password:' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> remark:' "$SSH_MODAL" >/dev/null
grep -F 'in property <[string]> keychain-identity-options: [];' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> keychain-identity-selected-label: "";' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> keychain-identity-username: "";' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> keychain-identity-auth-summary: "";' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> proxy-type: "none";' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> proxy-socks5-host: "";' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> proxy-socks5-port: "";' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> proxy-socks5-username: "";' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> proxy-socks5-password: "";' "$SSH_MODAL" >/dev/null
grep -F 'in property <bool> proxy-socks5-password-visible: false;' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> proxy-ssh-asset-id: "";' "$SSH_MODAL" >/dev/null
grep -F 'in property <[string]> proxy-ssh-options: [];' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> proxy-ssh-selected-label: "";' "$SSH_MODAL" >/dev/null
grep -F 'text: "Proxy Type";' "$SSH_MODAL" >/dev/null
grep -F 'label: "SOCKS5 Host";' "$SSH_MODAL" >/dev/null
grep -F 'label: "SOCKS5 Port";' "$SSH_MODAL" >/dev/null
grep -F 'label: "HTTP Host";' "$SSH_MODAL" >/dev/null
grep -F 'label: "HTTP Port";' "$SSH_MODAL" >/dev/null
grep -F 'label: "Username";' "$SSH_MODAL" >/dev/null
grep -F 'label: "Password";' "$SSH_MODAL" >/dev/null
grep -F 'text: "Upstream SSH Connection";' "$SSH_MODAL" >/dev/null
if grep -F 'label: "Proxy Method";' "$SSH_MODAL" >/dev/null; then
  exit 1
fi
if grep -F 'label: "Session Environment";' "$SSH_MODAL" >/dev/null; then
  exit 1
fi
! grep -F 'callback tab-selected(string);' "$SSH_MODAL" >/dev/null
grep -F 'callback draft-changed(string, string);' "$SSH_MODAL" >/dev/null
grep -F 'callback action-requested(string);' "$SSH_MODAL" >/dev/null
grep -F 'min-width: 0px;' "$SSH_MODAL" >/dev/null
if grep -F '"Standard"' "$SSH_MODAL" >/dev/null; then
  echo "ssh modal must not keep the legacy Standard tab label" >&2
  exit 1
fi
if grep -F '"Environment"' "$SSH_MODAL" >/dev/null; then
  echo "ssh modal must not keep the legacy Environment tab label" >&2
  exit 1
fi
if grep -F '"Advanced"' "$SSH_MODAL" >/dev/null; then
  echo "ssh modal must not keep the legacy Advanced tab label" >&2
  exit 1
fi
if grep -F 'in property <string> active-tab:' "$SSH_MODAL" >/dev/null; then
  echo "ssh modal must not expose a bridged active-tab input property" >&2
  exit 1
fi
if grep -F 'private property <string> active-tab:' "$SSH_MODAL" >/dev/null; then
  echo "ssh modal must not keep local active-tab state once the grouped form lands" >&2
  exit 1
fi
! grep -F 'in property <string> secret-retention-message:' "$SSH_MODAL" >/dev/null
! grep -F 'in property <bool> can-clear-saved-secret:' "$SSH_MODAL" >/dev/null
! grep -F 'in property <bool> clear-saved-secret-requested:' "$SSH_MODAL" >/dev/null
! grep -F 'Clear Saved Secret' "$SSH_MODAL" >/dev/null
! grep -F 'in-out property <string> asset-ssh-modal-secret-retention-message: "";' "$APP_WINDOW" >/dev/null
! grep -F 'in-out property <bool> asset-ssh-modal-can-clear-saved-secret: false;' "$APP_WINDOW" >/dev/null
! grep -F 'in-out property <bool> asset-ssh-modal-clear-saved-secret-requested: false;' "$APP_WINDOW" >/dev/null
! grep -F 'secret-retention-message: root.asset-ssh-modal-secret-retention-message;' "$APP_WINDOW" >/dev/null
! grep -F 'Primary remote' "$SYNC_MODAL" >/dev/null
! grep -F 'Mirror remote' "$SYNC_MODAL" >/dev/null
! grep -F 'primary-action := Rectangle' "$SYNC_MODAL" >/dev/null
! grep -F 'can-clear-saved-secret: root.asset-ssh-modal-can-clear-saved-secret;' "$APP_WINDOW" >/dev/null
! grep -F 'clear-saved-secret-requested: root.asset-ssh-modal-clear-saved-secret-requested;' "$APP_WINDOW" >/dev/null
grep -F 'import { ComboBox, ScrollView } from "std-widgets.slint";' "$SSH_MODAL" >/dev/null
grep -F 'ComboBox {' "$SSH_MODAL" >/dev/null
! grep -F 'label: "Connect";' "$SSH_MODAL" >/dev/null
! grep -F 'label: "Save and Connect";' "$SSH_MODAL" >/dev/null
! grep -F 'label: "Test Connection";' "$SSH_MODAL" >/dev/null
grep -F 'label: "Test";' "$SSH_MODAL" >/dev/null
grep -F 'label: "Save";' "$SSH_MODAL" >/dev/null
grep -F 'Manual' "$SSH_MODAL" >/dev/null
grep -F 'Keychain Identity' "$SSH_MODAL" >/dev/null
grep -F 'text: "Identity";' "$SSH_MODAL" >/dev/null
grep -F 'text: "Username";' "$SSH_MODAL" >/dev/null
grep -F 'text: "Authentication Summary";' "$SSH_MODAL" >/dev/null
grep -F 'root.draft-changed("auth_source"' "$SSH_MODAL" >/dev/null
grep -F 'root.draft-changed("keychain_identity_label"' "$SSH_MODAL" >/dev/null
if grep -F 'Use Existing Keychain Identity' "$SSH_MODAL" >/dev/null; then
  echo "ssh modal must replace the temporary keychain identity button with a real source picker" >&2
  exit 1
fi
grep -F 'InputType.password' "$SSH_MODAL" >/dev/null
grep -F 'password_visibility' "$SSH_MODAL" >/dev/null
grep -F 'busy' "$SSH_MODAL" >/dev/null
grep -F 'hover' "$TOKENS" >/dev/null
grep -F 'pressed' "$TOKENS" >/dev/null
grep -F 'in property <string> item-name;' "$RENAME_MODAL" >/dev/null
grep -F 'in property <string> validation-message;' "$RENAME_MODAL" >/dev/null
grep -F 'in property <bool> can-confirm;' "$RENAME_MODAL" >/dev/null
grep -F 'callback name-changed(string);' "$RENAME_MODAL" >/dev/null
grep -F 'callback confirm-requested();' "$RENAME_MODAL" >/dev/null
grep -F 'callback close-requested();' "$RENAME_MODAL" >/dev/null
grep -F 'in property <string> target-label;' "$DELETE_MODAL" >/dev/null
grep -F 'in property <int> descendant-count;' "$DELETE_MODAL" >/dev/null
grep -F 'callback confirm-requested();' "$DELETE_MODAL" >/dev/null
grep -F 'callback close-requested();' "$DELETE_MODAL" >/dev/null
! rg -n '[一-龥]' "$FOLDER_MODAL" >/dev/null
! rg -n '[一-龥]' "$SSH_MODAL" >/dev/null
! rg -n '[一-龥]' "$RENAME_MODAL" >/dev/null
! rg -n '[一-龥]' "$DELETE_MODAL" >/dev/null
grep -F 'dialog-title: root.asset-ssh-modal-dialog-title;' "$APP_WINDOW" >/dev/null
grep -F 'name: root.asset-snippet-modal-name;' "$APP_WINDOW" >/dev/null
grep -F 'script: root.asset-snippet-modal-script;' "$APP_WINDOW" >/dev/null
grep -F 'package: root.asset-snippet-modal-package;' "$APP_WINDOW" >/dev/null
grep -F 'package-options: root.asset-snippet-modal-package-options;' "$APP_WINDOW" >/dev/null
grep -F 'package-selected-label: root.asset-snippet-modal-package-selected-label;' "$APP_WINDOW" >/dev/null
grep -F 'package-name: root.asset-snippet-package-modal-name;' "$APP_WINDOW" >/dev/null
grep -F 'auth-source: root.asset-ssh-modal-auth-source;' "$APP_WINDOW" >/dev/null
grep -F 'proxy-type: root.asset-ssh-modal-proxy-type;' "$APP_WINDOW" >/dev/null
grep -F 'keychain-identity-options: root.asset-ssh-modal-keychain-identity-options;' "$APP_WINDOW" >/dev/null
grep -F 'keychain-identity-selected-label: root.asset-ssh-modal-keychain-identity-selected-label;' "$APP_WINDOW" >/dev/null
grep -F 'keychain-identity-username: root.asset-ssh-modal-keychain-identity-username;' "$APP_WINDOW" >/dev/null
grep -F 'keychain-identity-auth-summary: root.asset-ssh-modal-keychain-identity-auth-summary;' "$APP_WINDOW" >/dev/null
grep -F 'proxy-socks5-host: root.asset-ssh-modal-proxy-socks5-host;' "$APP_WINDOW" >/dev/null
grep -F 'proxy-socks5-port: root.asset-ssh-modal-proxy-socks5-port;' "$APP_WINDOW" >/dev/null
grep -F 'proxy-socks5-username: root.asset-ssh-modal-proxy-socks5-username;' "$APP_WINDOW" >/dev/null
grep -F 'proxy-socks5-password: root.asset-ssh-modal-proxy-socks5-password;' "$APP_WINDOW" >/dev/null
grep -F 'proxy-socks5-password-visible: root.asset-ssh-modal-proxy-socks5-password-visible;' "$APP_WINDOW" >/dev/null
grep -F 'proxy-ssh-asset-id: root.asset-ssh-modal-proxy-ssh-asset-id;' "$APP_WINDOW" >/dev/null
grep -F 'proxy-ssh-options: root.asset-ssh-modal-proxy-ssh-options;' "$APP_WINDOW" >/dev/null
grep -F 'proxy-ssh-selected-label: root.asset-ssh-modal-proxy-ssh-selected-label;' "$APP_WINDOW" >/dev/null
grep -F 'in property <int> focus-sequence: 0;' "$FOLDER_MODAL" >/dev/null
grep -F 'in property <int> focus-sequence: 0;' "$SNIPPET_MODAL" >/dev/null
grep -F 'in property <int> focus-sequence: 0;' "$SNIPPET_PACKAGE_MODAL" >/dev/null
grep -F 'in property <int> focus-sequence: 0;' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> active-panel: "console";' "$CREATE_MENU" >/dev/null
grep -F 'root.active-panel == "snippets"' "$CREATE_MENU" >/dev/null
grep -F 'label: "New Snippet";' "$CREATE_MENU" >/dev/null
grep -F 'label: "New Package";' "$CREATE_MENU" >/dev/null
grep -F 'callback new-snippet-selected;' "$CREATE_MENU" >/dev/null
grep -F 'callback new-snippet-package-selected;' "$CREATE_MENU" >/dev/null
grep -F 'in property <[ConsoleAssetItem]> snippet-asset-items: [];' "$ASSETS_SIDEBAR" >/dev/null
grep -F 'if root.active-panel == "snippets" && root.snippet-asset-items.length == 0' "$ASSETS_SIDEBAR" >/dev/null
grep -F 'if root.active-panel == "snippets" && root.snippet-asset-items.length > 0' "$ASSETS_SIDEBAR" >/dev/null
grep -F 'in property <[ConsoleAssetItem]> snippet-asset-items: [];' "$SIDEBAR" >/dev/null
grep -F 'snippet-asset-items: root.snippet-asset-items;' "$SIDEBAR" >/dev/null
rg -n "validation-message" "$FOLDER_MODAL" >/dev/null
rg -n "validation-message" "$SNIPPET_MODAL" >/dev/null
rg -n "validation-message" "$SNIPPET_PACKAGE_MODAL" >/dev/null
rg -n "validation-message" "$SSH_MODAL" >/dev/null
rg -n "can-confirm" "$FOLDER_MODAL" >/dev/null
rg -n "can-confirm" "$SNIPPET_MODAL" >/dev/null
rg -n "can-confirm" "$SNIPPET_PACKAGE_MODAL" >/dev/null
rg -n "can-confirm" "$SSH_MODAL" >/dev/null
grep -F 'window.set_asset_modal_focus_sequence(window.get_asset_modal_focus_sequence() + 1);' "$BOOTSTRAP" >/dev/null
