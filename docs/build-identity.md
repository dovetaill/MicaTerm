# Build identity

Open the upper-left menu → **About** to see the package version,
release date, UTC build time, Git revision (with a modified-worktree marker),
and project GitHub URL. Close with Escape, the header close icon, or Close.
Use **Copy details** to copy the full identity, the copy icon to copy the
commit, or the repository link to open GitHub. The link also supports Enter
when focused. Small windows scroll the content while keeping actions visible.

The version and repository come from `Cargo.toml`. Builds without
`MICA_TERM_RELEASE_DATE` display **Unreleased**. Release packaging
should supply the actual release timestamp, for example:

```sh
MICA_TERM_RELEASE_DATE='2026-09-18 12:00:00 UTC' ./build-desktop.sh
```

Set this timestamp to the intended publication time; it is not inferred from
Git commit time. `SOURCE_DATE_EPOCH` optionally fixes the embedded build time
for reproducible builds. Otherwise the build script uses the current UTC time.
Source archives without Git metadata display `unknown` for the revision.
Optional Git paths are watched only when present, so an absent `packed-refs`
does not invalidate every subsequent build.

## Validation

`cargo test --locked --test about_modal_smoke` covers actual keyboard and
pointer events, clipboard contents, repository opening, and focus restoration.
With the default software renderer enabled, visual review snapshots for dark,
light, compact, and 2x scale layouts can be generated under
`target/about-snapshots`:

```sh
cargo test --locked --test about_modal_smoke about_render_snapshots -- --ignored
```

## Vim capability reply diagnosis

Commit `d2ec067c8846fa27d7119e1a8cdd88e53139680d` selects secondary device
attributes `ESC [ > 0 ; 100 ; 0 c` in `SessionTerminalConfig`, avoiding Vim's
automatic xterm XTGETTCAP probe chain. Explicit capability queries remain
supported. `tests/terminal_session_spec.rs` covers split device queries,
alternate-screen exit, arrow input, and explicit capability responses.

A source checkout containing this fix does not prove an installed executable
contains it. Compare the About revision and build time with the package used.
If the problem persists with a confirmed fixed build, capture the remote Vim
version/configuration and whether tmux or screen is between Vim and MicaTerm;
explicit probes and remote configuration require separate investigation.
