#!/usr/bin/env bash
# PostToolUse hook (Edit|Write of .rs): fast, non-blocking cargo check for immediate feedback.
set -uo pipefail

cd "${CLAUDE_PROJECT_DIR:-$(pwd)}" || exit 0
[ -f Cargo.toml ] || exit 0

if ! err="$(cargo check --all --quiet 2>&1)"; then
  printf '⚠ cargo check failed:\n%s\n' "$(printf '%s' "$err" | tail -n 20)"
fi
exit 0
