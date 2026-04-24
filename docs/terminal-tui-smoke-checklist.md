# Terminal TUI Smoke Checklist

## Scope

Use this checklist when validating terminal regressions around:

- `贴底` after resize or output bursts
- `alt-screen` entry and exit
- `spinner` and progress line rewrites
- `link` hover and Ctrl+click behavior
- `glyph` rendering for repeated separators

## Scenarios

### codex

- Observe whether the status area stays贴底 after prompt updates.
- Confirm resize does not leave stale rows behind.

### vim

- Enter and exit `alt-screen` cleanly.
- Confirm returning to the shell restores the previous viewport.

### less

- Confirm `alt-screen` paging does not leave ghost rows on exit.
- Verify scrolling and resize stay stable.

### htop

- Watch high-frequency refresh for stale tails or smear.
- Confirm switching back to the shell restores the normal surface.

### links

- Verify TUI mouse usage does not trigger host `link` affordances.
- Confirm `alt-screen` link hover stays suppressed.

### glyphs

- Inspect `drwx-----`, `----------`, `___`, and `===`.
- Confirm repeated separators do not collapse into an oversized continuous stroke.

### progress

- Confirm `spinner` and progress updates clear shortened tails.
- Verify CR/EL style rewrites stay visually stable after resize.
