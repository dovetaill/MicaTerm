# External References

## ZMODEM UX patterns

- `FGasper/zmodemjs` documents the receive flow as: detect session, surface the offered file, then `accept()` and save locally after the user-facing acceptance step. Source: <https://github.com/FGasper/zmodemjs>
- ZOC Terminal documents that `sz` opens a dedicated download/progress window and writes into the configured local download folder, while `rz` can be triggered either from the shell or by dragging files into the terminal window. Sources:
  - <https://www.emtec.com/zoc/help/en/10381/zmodem-file-transfer>
  - <https://www.emtec.com/kb/en/2007/zmodem-transfer-to-from-linux>
- SecureCRT documents that remote `rz` opens a local "Select Files to Send using Zmodem" dialog before upload proceeds. Source: <https://documentation.help/SecureCRT/Uploading_a_File_w_Zmodem.htm>

## External drag/drop event limitations

- The current vendored `winit` event API used in this repo exposes `HoveredFile(PathBuf)` / `DroppedFile(PathBuf)` without coordinates. Source:
  - `vendor/winit/src/event.rs`
- The upstream `winit` discussion for this API notes that apps often do not receive normal `CursorMoved` updates during file drag, which breaks multi-target drop-zone hit testing if the app relies only on cached cursor state. Source:
  - <https://github.com/rust-windowing/winit/issues/1550>
- The vendored Windows drop handler receives a COM drag point (`POINTL`) but currently ignores it and emits only file-path events. Source:
  - `vendor/winit/src/platform_impl/windows/drop_handler.rs`

## Design conclusion

- Mature terminal products gate ZMODEM receive behind an explicit local destination or progress window.
- The current backend does not guarantee usable coordinates during external file drag, so app-side pointer fallback is required for reliable drop-target highlighting and drop routing.
