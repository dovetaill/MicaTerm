# Lossless ZMODEM stream gate implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use inline execution (recommended) or manual inline execution to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve every non-protocol terminal byte while keeping ZMODEM headers hidden and making automatic drag uploads quiet.

**Architecture:** Extend `ZmodemController` with a lossless tentative-header gate, provisional-star accounting, explicit automatic-echo ownership, and a direction-aware post-session tail. Keep `zmodem2` as the protocol and CRC authority, and return released tail bytes through the existing SSH terminal-output pipeline.

**Tech Stack:** Rust 2024, `zmodem2` 0.7.1, Tokio/russh SSH pump, existing terminal core and Rust unit tests, GNU lrzsz for local interoperability.

## Global Constraints

- Do not change transfer modal UI, drag target routing, SFTP fallback, or manual `rz` command construction.
- Do not add a second ZMODEM protocol implementation or a CRC dependency.
- Every byte not validated as protocol or exact app-generated command echo must reach `process_ready_remote_output()` exactly once and in order.
- Preserve split-header detection across every byte boundary.
- Do not add sleeps, shell probes, recovery commands, or a fixed active-transfer timeout.
- Follow TDD: add each failing regression before changing its owning behavior.

---

### Task 1: Lock the lossless tentative-detection contract

**Files:**
- Modify: `src/app/ssh/runtime/zmodem.rs:120-220`
- Test: `src/app/ssh/runtime/zmodem.rs:1329-1720`

**Interfaces:**
- Consumes: `ZmodemController::intercept_remote_bytes(&mut self, bytes: &[u8]) -> Vec<u8>`.
- Produces: regression coverage for provisional output, false-candidate replay, complete-header confidence, and non-destructive local input.

- [ ] **Step 1: Add failing ordinary-star and local-input tests**

Add table-driven tests named `ordinary_star_candidates_are_visible_without_waiting_for_enter` and `local_input_never_discards_tentative_remote_bytes`. Feed `*`, `**`, `***`, `*.log`, `a*b`, quoted/escaped stars, and split false candidates. Track bytes returned by every call plus `flush_terminal_bytes()` and assert each visible star appears once and every unseen byte is eventually returned in order.

```rust
for input in [b"*".as_slice(), b"**", b"***", b"*.log", b"a*b"] {
    let mut controller = ZmodemController::default();
    let mut output = controller.intercept_remote_bytes(input);
    controller.note_local_input();
    output.extend(controller.flush_terminal_bytes());
    assert_eq!(output, input, "ordinary bytes changed for {input:?}");
}
```

- [ ] **Step 2: Add failing split-header and false-header tests**

Generate valid ZRQINIT/ZRINIT headers from `zmodem2`, split each at every offset, and assert detection occurs only after the complete valid header. Corrupt one CRC nibble and assert the entire candidate is replayed as terminal data. Add a later-header case such as `***\x18B01...` and assert the first ordinary star remains while only overlapping protocol stars are retracted.

- [ ] **Step 3: Run the new tests and record the expected failures**

Run:

```bash
cargo test -q ordinary_star_candidates_are_visible_without_waiting_for_enter --lib
cargo test -q local_input_never_discards_tentative_remote_bytes --lib
cargo test -q split_valid_initial_headers_require_complete_crc --lib
cargo test -q invalid_initial_header_replays_every_byte --lib
```

Expected: the first two tests fail because stars are buffered/cleared; the header-confidence tests fail because the current six-byte marker immediately starts a session.

- [ ] **Step 4: Implement complete-header probing and provisional-star accounting**

Add the following private ownership types and fields in `zmodem.rs`:

```rust
const ZMODEM_ZHEX_HEADER_CORE_LEN: usize = 18;
const TERMINAL_ERASE_CELL: &[u8] = b"\x08 \x08";

enum InitialHeaderScan {
    None,
    Pending { start: usize },
    Confirmed { start: usize, direction: ZmodemTransferDirection },
}

struct TentativeTerminalBytes {
    bytes: Vec<u8>,
    visible_prefix_len: usize,
}
```

Replace the bare `sniff_buffer` ownership with helpers that maintain the invariant `bytes[..visible_prefix_len]` was already emitted. Implement `scan_initial_header()` so a short marker remains pending until 18 ZHEX bytes are available. Implement `validate_initial_header_with_zmodem2()` using a temporary sender/receiver, drain its constructor `WriteWire`, submit the 18-byte candidate, and require a complete successful consume.

On pending candidates, emit any newly encountered leading `*` bytes and record them as visible. On rejection, emit only the unseen suffix. On confirmation, emit ordinary bytes before the header and append `TERMINAL_ERASE_CELL` once for each visible byte overlapping the confirmed header.

- [ ] **Step 5: Make local input and flush non-destructive**

Keep `note_local_input()` responsible for ending stale post-session mode, but remove all tentative-buffer clearing. Change `flush_terminal_bytes()` to return only `bytes[visible_prefix_len..]`, then reset both fields. Do the same before transport-close cleanup.

- [ ] **Step 6: Run the detector tests**

Run:

```bash
cargo test -q zmodem --lib
```

Expected: all detector tests pass, including prior prefix and autostart tests.

- [ ] **Step 7: Review checkpoint**

Inspect `git diff -- src/app/ssh/runtime/zmodem.rs` and verify that no file-transfer state, modal state, or command string changed in this task.

### Task 2: Preserve active-session trailing bytes

**Files:**
- Modify: `src/app/ssh/runtime/zmodem.rs:374-1122`
- Modify: `src/app/ssh/runtime/pump.rs:335-520,928-964`
- Test: `src/app/ssh/runtime/zmodem.rs:1490-1720`

**Interfaces:**
- Consumes: `SenderTransfer::pending_wire`, `ReceiverTransfer::pending_wire`, and `drive_zmodem()`.
- Produces: `ZmodemSession::take_pending_wire() -> Vec<u8>`, `ZmodemController::take_released_terminal_bytes() -> Vec<u8>`, and terminal bytes returned after protocol driving.

- [ ] **Step 1: Add failing same-chunk tail tests**

Add `completed_sender_releases_same_chunk_prompt` and `completed_receiver_consumes_only_expected_oo_then_releases_prompt`. Build real `zmodem2` completion frames, append prompts such as `root@host:~# `, `OO-service# `, `rz-admin# `, and `* prompt`, then assert the ordinary suffix survives exactly once.

Add split cases for `\r`, `\n`/`\x8a`, each `O`, and the first prompt byte. Include a mismatch case where an expected `O` is followed by `x`; assert `Ox...` is released unchanged.

- [ ] **Step 2: Run tail tests and verify failure**

Run:

```bash
cargo test -q completed_sender_releases_same_chunk_prompt --lib
cargo test -q completed_receiver_consumes_only_expected_oo_then_releases_prompt --lib
```

Expected: fail because the finished session drops `pending_wire` or the generic drain removes prompt prefixes.

- [ ] **Step 3: Expose unconsumed wire before session drop**

Add identical ownership methods to sender and receiver and forward through `ZmodemSession`:

```rust
fn take_pending_wire(&mut self) -> Vec<u8> {
    std::mem::take(&mut self.pending_wire)
}
```

In `ZmodemController::advance()`, capture `direction` and `session.take_pending_wire()` before discarding a finished session.

- [ ] **Step 4: Replace generic drain with a direction-aware tail state**

Replace the boolean `post_session_drain` and `strip_post_session_zmodem_noise()` with an enum that consumes the exact remaining ZFIN trailer and, for a local receiver only, the peer's `OO`:

```rust
enum PostSessionTail {
    Upload { trailer_offset: usize },
    Download { trailer_offset: usize, over_and_out_offset: usize },
}
```

The consume method accepts `\r` followed by `\n` or `\x8a`. After the known trailer, upload releases all bytes; download consumes exactly two `O` bytes then releases all bytes. Any mismatch releases the buffered candidate unchanged and ends tail mode.

Store released bytes in `released_terminal_bytes` and expose `take_released_terminal_bytes()`.

- [ ] **Step 5: Return released bytes through the SSH output path**

Change `drive_zmodem()` to return `Option<Vec<u8>>`: `None` means channel write failure, and `Some(bytes)` contains `take_released_terminal_bytes()` at idle. At every caller, append returned bytes to the current `terminal_bytes` before invoking `process_ready_remote_output()`. Command-driven cancel/failure branches must process a non-empty released vector through the same helper rather than applying bytes directly to the terminal core.

- [ ] **Step 6: Run tail and existing protocol tests**

Run:

```bash
cargo test -q completed_sender_releases_same_chunk_prompt --lib
cargo test -q completed_receiver_consumes_only_expected_oo_then_releases_prompt --lib
cargo test -q zmodem --lib
```

Expected: all pass; prior final-wire-drain and cancellation tests remain green.

- [ ] **Step 7: Review checkpoint**

Search for destructive buffer clearing and the removed heuristic:

```bash
git grep -n "sniff_buffer.clear\|strip_post_session_zmodem_noise\|pending_wire.clear" -- src/app/ssh/runtime/zmodem.rs
```

Expected: no ordinary-output path clears tentative or unconsumed bytes; any remaining `pending_wire.clear()` is limited to confirmed protocol cancellation/reset state and is justified by its test.

### Task 3: Make automatic `rz` quiet and explicitly owned

**Files:**
- Modify: `src/app/ssh/runtime/pump.rs:26-30,229-256,708-727,1295-1325`
- Modify: `src/app/ssh/runtime/zmodem.rs:120-220,1200-1277,1438-1489`
- Test: `src/app/ssh/runtime/pump.rs:1295-1325`
- Test: `src/app/ssh/runtime/zmodem.rs:1438-1489`

**Interfaces:**
- Consumes: `RuntimeCommand::StartInteractiveZmodemUpload`, `INTERACTIVE_RZ_UPLOAD_COMMAND`, and exec upload command formatting.
- Produces: `ZmodemController::expect_automatic_rz_echo()` and quiet automatic commands.

- [ ] **Step 1: Add failing quiet-command and ownership tests**

Update the pump constant test to require:

```rust
assert_eq!(INTERACTIVE_RZ_UPLOAD_COMMAND, b" rz -q\r");
```

Add an exec-command helper assertion that the command ends with `&& rz -q`. Add controller tests proving an explicitly armed ` rz -q\r` echo is removed before a validated ZRINIT header, while identical manual text without arming remains visible.

- [ ] **Step 2: Run quiet-command tests and verify failure**

Run:

```bash
cargo test -q interactive_rz_fallback_uses_quiet_history_friendly_command --lib
cargo test -q automatic_quiet_rz_echo_requires_explicit_ownership --lib
```

Expected: fail because current commands use plain `rz` and echo stripping is heuristic.

- [ ] **Step 3: Implement quiet commands and explicit echo ownership**

Set `INTERACTIVE_RZ_UPLOAD_COMMAND` to `b" rz -q\r"`. Extract the exec command formatting into a private pure helper used by `run_zmodem_exec_upload_inner()` and append `rz -q`.

Before writing the interactive command, call `zmodem.expect_automatic_rz_echo()`. Gate echo stripping on this one-shot flag and recognize quiet plus legacy leading-space forms only when they immediately precede the validated upload header. Clear the flag on confirmation, mismatch, startup timeout, or transport close. Do not strip unarmed manual `rz` text.

- [ ] **Step 4: Run command and ZMODEM tests**

Run:

```bash
cargo test -q interactive_rz --lib
cargo test -q automatic_quiet_rz_echo --lib
cargo test -q zmodem --lib
```

Expected: all pass, with no command-probe or recovery-shell strings introduced.

- [ ] **Step 5: Verify local lrzsz quiet behavior**

Run PTY captures for `rz` and `rz -q` and assert only the non-quiet form contains the banner:

```bash
timeout -s KILL 1s script -qfec 'rz' /dev/null 2>&1 | od -An -tx1c
timeout -s KILL 1s script -qfec 'rz -q' /dev/null 2>&1 | od -An -tx1c
```

Expected: both contain a ZRINIT frame; only the first contains `rz waiting to receive.`.

### Task 4: Full verification and contract update

**Files:**
- Modify after verification: `.trellis/spec/backend/quality-guidelines.md`
- Verify: `src/app/ssh/runtime/zmodem.rs`
- Verify: `src/app/ssh/runtime/pump.rs`

**Interfaces:**
- Consumes: completed implementation and all PRD requirements R1-R5.
- Produces: verified runtime behavior and a project-level lossless stream-gate contract.

- [ ] **Step 1: Run focused tests**

```bash
cargo test -q zmodem --lib
cargo test -q --test ssh_terminal_interaction_spec
cargo test -q --test bootstrap_smoke
```

Expected: pass. If the test runner uses substring filtering differently, run the named test target without the filter and record the exact command.

- [ ] **Step 2: Run build and lint checks**

```bash
cargo check -q
cargo clippy --all-targets -- -D warnings
```

Expected: pass with no new warnings.

- [ ] **Step 3: Audit byte ownership and command strings**

```bash
git grep -n "rz waiting to receive\|&& rz\|INTERACTIVE_RZ_UPLOAD_COMMAND\|sniff_buffer.clear\|strip_post_session_zmodem_noise" -- src tests .trellis/spec
git diff --check
```

Expected: automatic commands use `rz -q`; no broad drain helper remains; no destructive tentative-buffer clear remains; diff has no whitespace errors.

- [ ] **Step 4: Update the backend quality contract**

Replace the old single-star wording with the general byte-ownership invariant, document full-header validation, same-chunk tail extraction, direction-aware terminator handling, explicit automatic echo ownership, and the required regression matrices. Preserve unrelated ZMODEM/drop contracts.

- [ ] **Step 5: Run Trellis quality verification**

Load `trellis-check`, run its required spec/lint/test checks, and record any unavailable platform-only runtime verification with the exact residual risk.

- [ ] **Step 6: Final review checkpoint**

Map each PRD acceptance criterion to a passing test or command output, review the final diff for unrelated changes, and only then mark the task ready for finish/archive and commit.
