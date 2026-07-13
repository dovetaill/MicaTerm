# Lossless ZMODEM stream gate design

Date: 2026-07-10
Author: Codex

## Scope

This design fixes the byte-loss and quiet-upload defects inside the existing SSH/ZMODEM runtime. It does not redesign transfer UI, drag target routing, SFTP fallback, or the `zmodem2` protocol engine.

## Invariants

1. Every remote byte is owned by exactly one destination: confirmed ZMODEM protocol handling or ordinary terminal output.
2. A tentative match never owns bytes permanently. If validation fails or the transport ends, all unseen tentative bytes are replayed once and in order.
3. Ordinary interactive stars are visible immediately. A star may be repainted only after it is proven to be part of a valid protocol header.
4. Dropping a completed `ZmodemSession` cannot drop its unconsumed input tail. Failed or cancelled sessions must not replay confirmed protocol bytes as terminal output.
5. Post-session filtering consumes only protocol bytes expected for the completed direction and state. It does not use broad text-prefix heuristics.
6. Automatic command echo suppression is enabled only by an app-generated interactive `rz` start.

## Evidence

- `ZmodemController::note_local_input()` currently clears tentative prefix bytes without emitting them.
- `SenderTransfer` and `ReceiverTransfer` keep unconsumed bytes in `pending_wire`, but `ZmodemController::advance()` drops a finished session without extracting that buffer.
- `strip_post_session_zmodem_noise()` removes `OO`, `rz` forms, control bytes, and ZMODEM-looking text without transfer-direction proof.
- Local lrzsz output proves that `rz -q` preserves the initialization frame and removes only `rz waiting to receive.`.
- Upstream zmodem.js forwards tentative traffic instead of losing it, and current Tabby code has an explicit safety-net flush for trailing terminal bytes after a session ends. MicaTerm will adopt the lossless ownership principle without forwarding confirmed protocol headers to the renderer.

## Rejected approaches

### Only stop clearing `sniff_buffer`

This prevents permanent loss on local input, but a single star can remain invisible until later remote output. It also leaves active-session tail loss and broad post-session deletion unchanged.

### Forward all initialization bytes to the terminal

This matches zmodem.js's low-latency sentry behavior, but leaks `**\x18B...` header text into MicaTerm's terminal surface. It regresses the existing clean transfer UI.

### Add a fixed delay before rendering candidate bytes

A timer makes correctness depend on network packet timing. A split header arriving after the delay either leaks protocol text or requires the same rollback mechanism, so the delay adds complexity without removing ambiguity.

## Chosen architecture

The existing `ZmodemController` remains the owner of protocol sessions. Its stream-facing state becomes an explicit lossless gate with three phases:

```text
Normal / tentative scan
  -> validated initialization header -> Active protocol session
  -> invalid candidate -> ordinary terminal replay

Active protocol session
  -> protocol actions and file IO
  -> completed session -> direction-aware tail drain

Direction-aware tail drain
  -> consume only the expected header trailer / OO terminator
  -> release the first ordinary byte and everything after it
  -> Normal / tentative scan
```

No new thread or async task is needed. The gate remains on the existing SSH pump path.

## Tentative detection

### Complete-header validation

The short markers `**\x18B00` and `**\x18B01` identify a candidate, not a confirmed session. The gate retains enough bytes for the 18-byte ZHEX header core and validates it with a temporary `zmodem2::Receiver` or `zmodem2::Sender` state machine after draining that temporary parser's constructor output. A successful full consume confirms frame type and CRC. An incomplete header stays tentative; a parser error makes the candidate ordinary terminal data.

This reuses `zmodem2` for protocol validation rather than duplicating CRC or frame parsing.

### Immediate star presentation

`sniff_buffer` gains a count of leading candidate bytes already presented to the terminal. Only leading ASCII stars are presented provisionally; control bytes and header payload remain hidden until validation resolves.

- Ordinary `*`, repeated stars, and pasted star runs are emitted immediately.
- When a candidate becomes ordinary text, only bytes not already presented are emitted.
- When a valid header is confirmed, only the already presented stars that overlap that header are repainted with one `\x08 \x08` sequence per cell.
- Stars before a later valid header remain visible; overlap is calculated from the confirmed header offset rather than erasing every recent star.

The invariant is that `sniff_buffer[..visible_len]` has already reached the terminal and `sniff_buffer[visible_len..]` has not.

### Local input and transport close

`note_local_input()` no longer clears tentative remote bytes. Local input does not redefine the remote byte-stream boundary. `flush_terminal_bytes()` emits only the unseen portion of a tentative buffer so provisional stars are never duplicated. Disconnect and failure paths flush unseen ordinary tentative data before clearing the gate.

## Active-session tail ownership

`SenderTransfer` and `ReceiverTransfer` expose `take_pending_wire() -> Vec<u8>`, forwarded through `ZmodemSession`. Before a finished session is dropped, the controller moves this unconsumed tail into a direction-aware tail state.

For both directions, a just-consumed ZHEX `ZFIN` can leave its CR/LF trailer unconsumed because `zmodem2` stops when it queues an event or outbound action. The tail state consumes that exact trailer, accepting LF with the high bit set for lrzsz compatibility.

- Local sender (upload): after the trailer, no inbound terminator is expected because the local sender writes `OO`. All following bytes belong to the restored shell.
- Local receiver (download): after the trailer, exactly one remote `OO` terminator is expected. Once it is consumed, all following bytes belong to the restored shell.

If expected bytes do not match, the buffered bytes are released unchanged. The old generic `strip_post_session_zmodem_noise()` heuristic is removed.

The controller stores released tail bytes until the SSH pump has finished `drive_zmodem()`. `drive_zmodem()` returns those bytes to the same `process_ready_remote_output()` path used by ordinary SSH output, so shell integration, terminal parsing, and dirty notifications remain intact.

## Automatic `rz` startup

The automatic commands become:

```text
interactive PTY:  " rz -q\r"
dedicated exec:   "<PATH setup>; cd <quoted-dir> && rz -q"
```

Before the interactive command is sent, the controller records that one automatic echo is expected. Echo stripping is permitted only while that flag is set and only for the exact quiet command or a legacy in-flight ` rz` form immediately followed by a validated upload header. Manual `rz` output is not broadly classified as disposable.

The 4-second startup handshake timeout remains. No new fixed mid-transfer timeout is added: aborting a valid slow transfer would be a regression. Upload state remains `Finalizing upload` until `zmodem2` receives the peer's final response, queues local `OO`, and all final wire bytes are written. Only then may the modal publish `Completed`.

## Failure behavior

- Invalid initialization header: replay the candidate as ordinary terminal data and continue scanning.
- Local input during tentative detection: preserve the remote candidate; never clear it.
- Transport close while tentative: flush unseen bytes once, then close.
- Completed session with same-batch prompt: strip only proven protocol tail and immediately release the prompt.
- Automatic interactive handshake timeout: release any unmatched ordinary bytes, publish the existing failed upload state, and do not inject recovery shell commands.
- Protocol error after a confirmed session: keep pending confirmed-session bytes out of the renderer and publish failure; lossless replay applies to the pre-session tentative gate and to the classified tail of a completed session.

## Test strategy

### Detector matrix

- Split valid upload and download initialization headers at every byte boundary.
- Feed false candidates at every partial length and with every possible first mismatching continuation used by the fixed regression cases.
- Interleave local text, structured key input, and paste notifications with tentative candidates.
- Cover `*`, `**`, `***`, `*.log`, `a*b`, quoted/escaped stars, and pasted runs.
- Assert ordinary output is byte-for-byte equal to input after accounting for provisional cells exactly once.

### Session-tail matrix

- Complete a local sender and receiver with final protocol bytes and prompt text in one input chunk.
- Split the ZHEX trailer, high-bit LF, `OO`, and prompt at every boundary.
- Verify prompts beginning with `O`, `OO`, `rz`, stars, and control-like bytes are not removed once the expected tail is complete or mismatches.

### Command and interoperability checks

- Lock the interactive constant to ` rz -q\r` and the exec suffix to `rz -q`.
- Verify automatic echo filtering is explicitly armed and manual lookalike text is retained.
- Use local lrzsz under a PTY to prove quiet startup still emits a valid handshake and omits the banner.
- Run focused ZMODEM tests, terminal interaction tests, `cargo check`, and the Trellis quality gate.

## Rollback boundaries

- The header-validation and provisional-presentation changes are isolated to the pre-session gate and can be reverted without changing file-transfer state.
- Tail extraction is isolated behind `take_pending_wire()` and the direction-aware tail state.
- Quiet command changes are independent constants/command formatting changes.
- If full-header probing exposes a `zmodem2` incompatibility, retain the lossless replay/tail work and fall back to the existing marker confidence only after documenting the exact interoperable frame that the probe rejects.

## External sources

- zmodem.js lossless sentry behavior: <https://github.com/FGasper/zmodemjs/blob/master/src/zsentry.js>
- Tabby trailing-terminal-byte handling: <https://github.com/Eugeny/tabby/blob/master/tabby-terminal/src/features/zmodem.ts>
- Local lrzsz option contract: `/usr/share/man/man1/rz.1.gz` and `/usr/bin/rz --help` (`-q, --quiet`).
