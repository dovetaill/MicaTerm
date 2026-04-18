# SFTP Download Behavior Design

## Goal

Bring the SFTP download experience in line with desktop-native expectations:
- `Open File` should use the system's default file-opening behavior.
- `Open Folder` should reveal a downloaded file in its containing folder when possible, or open the downloaded folder directly.
- `Remove` should move completed local download artifacts to Trash/Recycle Bin when they still exist, then remove the transfer-center record.
- Download-name conflicts should support `Overwrite`, `Auto Rename`, and `Cancel Download`, with a persisted default strategy for the first two.

## Product Decisions

### Download conflict actions

For download conflicts, the transfer center and conflict modal should offer:
- `Overwrite`
- `Auto Rename`
- `Cancel Download`

`Cancel Download` applies only to the current conflicting item. It is not a remembered default and does not participate in batch application.

### Persisted default strategy

Persist a download-only default conflict strategy in UI preferences:
- `ask`
- `overwrite`
- `auto_rename`

Expose this in Settings as a single-choice control:
- `Ask every time`
- `Always overwrite`
- `Always auto rename`

This replaces the earlier checkbox idea with a clearer, mutually exclusive model while preserving the same behavior.

### Batch application

The conflict modal keeps the existing “apply to this batch” affordance, but it only applies when the chosen action is:
- `Overwrite`, or
- `Auto Rename`

If the user selects `Cancel Download`, only the current conflict is cancelled and later conflicts continue to prompt individually.

## Platform Behavior

### Open File

Completed single-file downloads should use the OS shell behavior rather than a custom app chooser:
- Windows: shell-open semantics so Windows can route to the default app or the system “Open with” flow.
- macOS: `open <path>` semantics.
- Linux: `xdg-open <path>` semantics.

If the local file no longer exists, the action stays disabled; if the open attempt fails, show transfer-center feedback.

### Open Folder

For a completed downloaded file:
- Windows: open Explorer and select the file.
- macOS: reveal the file in Finder.
- Linux: first try `org.freedesktop.FileManager1.ShowItems`; if unavailable, fall back to opening the parent directory.

For a completed downloaded directory:
- Open the directory itself on every platform.

### Remove

`Remove` changes meaning for completed download tasks only:
- If the downloaded local file or folder still exists, move it to Trash/Recycle Bin and then remove the transfer-center record.
- If the local artifact no longer exists, remove only the record and show a mild feedback message.
- For non-download tasks, keep the current “remove record only” behavior.

No hard-delete fallback should run automatically after a trash failure.

## Data Model Changes

### Conflict policy model

The current generic transfer conflict policy (`Overwrite`, `Skip`) is not expressive enough for downloads. Introduce a download-aware conflict resolution path that can represent:
- `Overwrite`
- `AutoRename`
- `CancelCurrent`

Only `Overwrite` and `AutoRename` should be serializable into a persisted settings default.

### Download path handling

Auto rename should use stable desktop-style suffixes:
- `report.txt` -> `report (1).txt`
- `report` -> `report (1)`
- `.env` -> `.env (1)`
- `logs` -> `logs (1)`

If a directory root is auto-renamed, every nested download target should be rewritten under the new root before transfer execution begins.

## UI Changes

### Settings modal

Add an `SFTP Downloads` (or `Downloads`) section to the existing settings modal with the single-choice default conflict strategy.

### Conflict modal

The modal should project different actions for download conflicts versus existing remote-conflict flows:
- Download conflict: `Overwrite`, `Auto Rename`, `Cancel Download`
- Existing remote conflict behavior can remain on the current `Replace` / `Skip` path for now

The copy and button labels should speak in download terms rather than generic file-manager terminology.

### Transfer center tooltips

Clarify tooltips to reflect the new behaviors:
- `Open File`: open with the system default app
- `Open Folder`: reveal/open local location
- `Remove` on completed downloads: move the downloaded item to Trash/Recycle Bin and remove the record

## Error Handling

- If `Open File` fails, show transfer-center feedback with the platform error.
- If reveal/open-folder fails, show transfer-center feedback and keep the record.
- If trashing fails, keep the record and surface the error. Do not hard-delete.
- If a remembered default strategy is invalid or missing, fall back to `ask`.

## Test Strategy

### Unit tests

- Preference serialization/deserialization for the new download conflict default.
- Auto-rename name generation for files, directories, dotfiles, and repeated collisions.
- Root-directory auto-rename propagation to nested download targets.
- Cancel-current conflict behavior leaving later conflicts untouched.

### Flow tests

- Completed download opens file and folder actions through the new platform helpers.
- Download conflict resolution with overwrite.
- Download conflict resolution with auto rename.
- Download conflict cancellation for only the current task.
- Completed download removal moves to trash when the local artifact exists.

### UI contract tests

- Settings modal exposes the new download conflict preference.
- Conflict modal exposes the new download action labels for download conflicts.
- Transfer center row projection keeps `Open File`, `Open Folder`, and `Remove` availability aligned with local artifact state.

## Relevant Files

Likely implementation touchpoints:
- `src/app/sftp/local_open.rs`
- `src/app/sftp/local_ops.rs`
- `src/app/sftp/queue.rs`
- `src/app/sftp/session_binding.rs`
- `src/app/ui_preferences.rs`
- `src/shell/view_model.rs`
- `src/shell/view_model/projection.rs`
- `src/shell/view_model/sftp.rs`
- `src/app/bootstrap/sftp.rs`
- `src/app/bootstrap/shell_chrome.rs`
- `ui/components/settings-modal.slint`
- `ui/components/sftp-conflict-modal.slint`
- `ui/app-window.slint`

Likely test touchpoints:
- `tests/ui_preferences.rs`
- `tests/sftp_queue_spec.rs`
- `tests/sftp_transfer_flow_spec.rs`
- `tests/transfer_center_smoke.rs`
- `tests/bootstrap_smoke.rs`
- `tests/vault_settings_smoke.rs`
- `tests/top_status_bar_ui_contract_smoke.sh`

## References

- Apple “Choose an app to open a file on Mac”
- Apple “If there is no application set to open the file”
- `xdg-open(1)` user-preferred application behavior
- freedesktop `org.freedesktop.FileManager1` interface (`ShowItems`, `ShowFolders`)
- freedesktop Trash Specification
