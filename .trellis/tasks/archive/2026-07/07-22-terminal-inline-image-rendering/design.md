# Design

## Parser Integration

Keep `tattoy-wezterm-term` as the protocol engine. Enable Kitty graphics in the
session config and project `CellAttributes::images()` from visible cells. Add a
minimal dependency patch only where protocol media policy or cache budgets cannot be
configured by the public API.

`TerminalCoreAdapter::apply_remote_bytes` returns both surface changes and generated
terminal replies, or exposes an immediate drain method. The SSH pump writes those
bytes before waiting for the next local-input event.

## Snapshot Model

Add immutable resource and placement records to terminal frame/runtime snapshots:

- Resource: content hash, dimensions, RGBA storage, decoded byte size.
- Placement: resource key, cell anchor/span, UV rectangle, pixel padding, z-index,
  image ID, and placement ID.

Resources are deduplicated by content hash within a session. Placements are cheap to
clone and may refer to one resource many times. Session teardown drops the resource
store.

## Rendering

Presenters derive clipped destination rectangles from real viewport metrics and
terminal cell metrics. Images with negative z-index render below text backgrounds;
non-negative images render in protocol order around text overlays according to the
existing compositor's layer contract. Native rendering retains GPU/host resources;
bitmap fallback alpha-composites the same placement list. Damage includes old and
new placement rectangles.

## Resource Policy

Protocol parsing accepts only in-band image bytes for SSH sessions. Decode and cache
limits are checked before retention. Eviction removes least-recently-used unplaced
resources first; if the session budget still cannot be met, the new image is
rejected and a protocol-appropriate error response is generated when applicable.
