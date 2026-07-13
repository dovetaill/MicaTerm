# Fix terminal ZMODEM byte loss and upload stall

## Goal

Make ZMODEM detection transparent to ordinary terminal traffic and make automatic drag-triggered uploads finish quietly and predictably. No ordinary terminal byte may be discarded merely because it temporarily resembles the beginning or end of a ZMODEM control sequence.

## Background

- Every ready SSH output batch passes through `ZmodemController::intercept_remote_bytes()` before terminal rendering (`src/app/ssh/runtime/pump.rs:406`).
- The detector retains any suffix that is a prefix of `**\x18B00` or `**\x18B01` (`src/app/ssh/runtime/zmodem.rs:198`). This includes `*`, `**`, `**\x18`, `**\x18B`, and `**\x18B0`.
- Any subsequent text input, structured key input, paste, or automatic interactive upload calls `note_local_input()`, which currently clears those tentative bytes without replaying them (`src/app/ssh/runtime/zmodem.rs:207`). This is a data-loss defect, not only a rendering defect.
- Active sender and receiver sessions retain protocol bytes that `zmodem2` has not consumed yet. When a session reaches `finished`, the controller currently drops that session and clears its outer buffer without extracting the unconsumed tail (`src/app/ssh/runtime/zmodem.rs:308`). A final frame and a restored shell prompt arriving in one SSH batch can therefore lose the prompt.
- The generic post-session drain treats leading `OO`, `rz` command forms, ZMODEM-looking headers, and several control bytes as disposable without considering transfer direction (`src/app/ssh/runtime/zmodem.rs:1222`). Ordinary output with the same prefix can be removed by this heuristic.
- Automatic interactive drag upload currently sends ` rz\r` (`src/app/ssh/runtime/pump.rs:30`), and the dedicated exec path starts `rz` without quiet mode (`src/app/ssh/runtime/pump.rs:719`).
- A local PTY capture with lrzsz 0.12.21rc confirmed that `rz` writes `rz waiting to receive.` before the ZMODEM frame, while `rz -q` writes the same protocol frame without that banner.
- The banner is not required by ZMODEM. Removing it must not be used to conceal an actual protocol-finalization delay.

## Requirements

### R1. Lossless tentative protocol detection

- Bytes that are not part of a confirmed ZMODEM session must reach the ordinary terminal path exactly once and in their original order.
- Local text input, structured key input, paste, resize, disconnect, channel failure, timeout, and automatic upload startup must not silently clear tentative remote bytes.
- False candidates containing any combination of `*`, control bytes, `B`, `0`, or adjacent ordinary text must be replayed without truncation or duplication.
- Only bytes proven to belong to an active ZMODEM exchange, plus the exact app-generated automatic `rz` command echo, may be withheld from the visible terminal stream.
- Unconsumed bytes that trail a completed protocol session must be extracted before the session object is dropped and routed back through the normal detector/terminal path. On failure or cancellation, confirmed protocol bytes remain protocol-owned; only pre-session tentative ordinary bytes are losslessly flushed.
- Post-session terminator handling must be direction-aware and consume only the exact terminator still expected for that completed protocol state.

### R2. Responsive ordinary terminal echo

- A typed `*` must become visible during ordinary shell editing without waiting for Enter.
- Repeated stars and stars mixed with other characters must render faithfully, including `*`, `**`, `***`, `*.log`, `a*b`, quoted stars, escaped stars, and pasted star sequences.
- The fix must apply to the detector contract generally rather than special-casing one literal input string.

### R3. Preserve real ZMODEM detection

- The short `**\x18B00` / `**\x18B01` marker is only a candidate. The controller must validate a complete ZHEX initialization header, including CRC, before committing bytes to a protocol session.
- Upload and download handshake detection must continue to work when `**\x18B00` or `**\x18B01` is divided at every possible chunk boundary.
- Confirmed protocol frames must not leak into terminal rendering.
- Any temporary visual treatment used while detection is tentative must be fully removed when a real protocol frame is confirmed, without damaging adjacent terminal content.

### R4. Quiet automatic `rz` startup

- Both automatic interactive fallback and dedicated exec upload paths must start `rz -q`.
- The exact automatic command echo must continue to be removed from terminal output.
- Manually typed `rz` commands remain user-controlled and are outside the automatic quieting contract.
- The automatic path must not leave `rz waiting to receive.` concatenated with the restored shell prompt.

### R5. Honest upload completion

- An upload may be reported as completed only after the final ZMODEM handshake bytes required by the remote `rz` process have been written.
- The active shell must recover after an interactive upload without an internal fixed sleep or an avoidable detector drain delay.
- The intermittent post-upload pause must be investigated independently from banner suppression. The existing 4-second startup handshake timeout remains, but this fix must not add an arbitrary active-transfer timeout that can abort a valid slow upload. Any residual finalization pause must retain timestamped protocol-state evidence.

## Acceptance Criteria

- [ ] A detector regression matrix covers every partial prefix length, false-prefix continuation, local-input interleaving, and transport flush path; all non-protocol input is emitted exactly once and byte-for-byte equal to the original stream.
- [ ] Ordinary shell input visibly preserves `*`, `**`, `***`, `*.log`, `a*b`, quoted/escaped stars, and pasted star sequences without requiring Enter to reveal the first star.
- [ ] Real upload and download prefixes are detected across every chunk split, and no protocol marker remains visible after detection.
- [ ] Automatic interactive upload sends exactly ` rz -q\r`; the dedicated exec command ends in `rz -q`; neither path adds command probes or shell recovery commands.
- [ ] Echo/noise filtering recognizes the quiet automatic command without broadening deletion to unrelated text that merely contains `rz`, `OO`, stars, or control characters.
- [ ] Final-frame tests place protocol completion, `OO`, and an arbitrary shell prompt in the same chunk and at every relevant split; the prompt and any following ordinary bytes are preserved exactly once.
- [ ] A local lrzsz interoperability check confirms that quiet mode emits the same valid initialization handshake while omitting `rz waiting to receive.`; focused protocol tests prove finalization bytes are drained before completion and same-batch shell output is released immediately.
- [ ] Existing focused ZMODEM tests, terminal interaction tests, `cargo check`, and the relevant repository quality checks pass.

## Out Of Scope

- Unrelated `sz` destination-selection behavior or transfer-modal redesign.
- External drag target hit-testing and SFTP panel behavior.
- Changing the behavior of a user who manually runs `rz` or `sz`.
- Replacing `zmodem2` or broadly restructuring the SSH runtime.
