#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/dev/terminal-tui-smoke.sh <scenario>

Scenarios:
  all
  codex
  vim
  less
  htop
  links
  glyphs
  progress
EOF
}

print_header() {
  local name="$1"
  printf '\n== %s ==\n' "$name"
}

print_command_or_fixture() {
  local label="$1"
  local command_name="$2"
  local fallback="$3"

  if command -v "$command_name" >/dev/null 2>&1; then
    printf 'Detected `%s`. Recommended launch: %s\n' "$command_name" "$command_name"
  else
    printf '%s\n' "$fallback"
  fi

  printf 'Observation focus: %s\n' "$label"
}

run_codex() {
  print_header "codex"
  print_command_or_fixture \
    "贴底 status line and resize recovery" \
    "codex" \
    "Fixture: simulate a full-screen agent session and verify footer alignment after output bursts."
}

run_vim() {
  print_header "vim"
  print_command_or_fixture \
    "alt-screen enter/exit and restore" \
    "vim" \
    "Fixture: open any file in vim and verify returning to the shell restores the previous surface cleanly."
}

run_less() {
  print_header "less"
  print_command_or_fixture \
    "alt-screen paging, scroll, and resize" \
    "less" \
    "Fixture: page a long file with less and verify exit does not leave stale rows."
}

run_htop() {
  print_header "htop"
  print_command_or_fixture \
    "high-frequency refresh stability" \
    "htop" \
    "Fixture: run a high-refresh TUI such as htop or btop and watch for smear or stale tails."
}

run_links() {
  print_header "links"
  print_command_or_fixture \
    "link gating inside mouse-driven TUI apps" \
    "links" \
    "Fixture: use a mouse-driven TUI and confirm host link hover/Ctrl+click does not leak into alt-screen."
}

run_glyphs() {
  print_header "glyphs"
  cat <<'EOF'
Inspect these samples in the terminal:
  drwx-----
  ----------
  ___
  ===
  ╭────╮
  │Codex│
  ╰────╯
  ─│╭╮╰╯
  █▀▄▌▐
Observation focus: glyph spacing, box drawing continuity, block elements fill, and resize/DPI stability.
EOF
}

run_progress() {
  print_header "progress"
  cat <<'EOF'
Fixture: verify spinner / progress rewrites stay clean.
  [/] loading
  [-] loading
  [\] loading
  [x] done
Observation focus: spinner tails, CR/EL rewrites, and resize stability.
EOF
}

run_all() {
  run_codex
  run_vim
  run_less
  run_htop
  run_links
  run_glyphs
  run_progress
}

main() {
  local scenario="${1:-all}"

  case "$scenario" in
    all)
      run_all
      ;;
    codex)
      run_codex
      ;;
    vim)
      run_vim
      ;;
    less)
      run_less
      ;;
    htop)
      run_htop
      ;;
    links)
      run_links
      ;;
    glyphs)
      run_glyphs
      ;;
    progress)
      run_progress
      ;;
    -h|--help|help)
      usage
      ;;
    *)
      printf 'Unknown scenario: %s\n\n' "$scenario" >&2
      usage >&2
      return 1
      ;;
  esac
}

main "$@"
