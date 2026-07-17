# Bug Analysis: Dedicated exec ZMODEM controls targeted the wrong owner

## 1. Root Cause Category

- **Primary category**: B - Cross-Layer Contract
- **Secondary category**: D - Test Coverage Gap
- **Specific cause**: Dedicated exec uploads published modal state through the
  shared runtime event stream, but Done, close, and Cancel were sent only to the
  interactive terminal pump's different `ZmodemController`. The visible
  projection therefore had no owner capable of clearing or cancelling the exec
  transfer. The bootstrap fake hid this split by emitting
  `ZmodemStateChanged(None)` directly from Dismiss/Cancel.

## 2. Why Earlier Fixes Did Not Cover It

1. The drag-routing fix correctly selected remote cwd and dedicated exec
   ZMODEM, but transfer selection ended at successful upload completion and did
   not trace modal commands back to the controller/channel owner.
2. Existing modal tests were surface fixes: their fake runtime manufactured the
   clear event that real dedicated exec code could never produce.
3. Running Cancel shared the same wrong-owner route, but completion-focused
   manual testing exercised Done/X first and did not prove abort-wire delivery.

## 3. Prevention Mechanisms

| Priority | Mechanism | Specific action | Status |
| --- | --- | --- | --- |
| P0 | Architecture | `SessionManager` owns revision-checked terminal projection removal | Done |
| P0 | Architecture | Runtime owns generation-scoped dedicated exec Cancel routing | Done |
| P0 | Integration test | Bootstrap Dismiss fake returns without emitting `None` | Done |
| P0 | Live protocol test | russh server proves Cancelled, exact abort wire, EOF, and Close | Done |
| P1 | Race tests | Stale projection revision and task generation cannot clear newer work | Done |
| P1 | Documentation | Backend spec records command/controller ownership and error matrix | Done |
| P1 | Diagnostics | Lifecycle logs include command, owner, generation, phase, and outcome | Done |

## 4. Systematic Expansion

- **Similar issues**: Any future transfer path that publishes through a shared
  modal event but runs on a dedicated task/channel can repeat this split.
- **Design improvement**: Treat modal projection ownership and live transport
  ownership as separate explicit contracts. Terminal dismissal must not require
  a completed transport actor to remain alive.
- **Process improvement**: For cross-layer tests, verify that test doubles only
  record commands or emulate the real owner. They must not synthesize the final
  event whose production origin is the behavior under test.
- **Scope boundary**: A public transfer-id actor model remains unnecessary until
  concurrent transfer queueing is required; private revisions/generations cover
  the current single-active-exec contract.

## 5. Knowledge Capture

- [x] Updated `.trellis/spec/backend/quality-guidelines.md` with executable
  signatures, lifecycle contracts, error cases, tests, and wrong/correct code.
- [x] Updated `.trellis/spec/guides/cross-layer-thinking-guide.md` with the
  test-double lifecycle-event check.
- [x] Added regression coverage at manager, controller, bootstrap, runtime-slot,
  and live russh boundaries.
