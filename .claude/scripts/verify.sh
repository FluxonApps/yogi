#!/usr/bin/env bash
# Stop / SubagentStop hook: forbid stopping while the build is red.
# Emits {"decision":"block","reason":...} (exit 0) when tests/clippy fail, so Claude must fix first.
set -uo pipefail

cd "${CLAUDE_PROJECT_DIR:-$(pwd)}" || exit 0
[ -f Cargo.toml ] || exit 0   # nothing to verify yet

test_out="$(cargo test --all 2>&1)"; test_rc=$?
clippy_out="$(cargo clippy --all-targets -- -D warnings 2>&1)"; clippy_rc=$?

if [ "$test_rc" -ne 0 ] || [ "$clippy_rc" -ne 0 ]; then
  reason="$(printf 'Build is RED — fix before stopping (Yogi golden rule).\n\ncargo test rc=%s, cargo clippy rc=%s\n\n--- tail ---\n%s' \
    "$test_rc" "$clippy_rc" "$(printf '%s\n%s' "$test_out" "$clippy_out" | tail -n 40)")"
  jq -n --arg r "$reason" '{decision:"block", reason:$r}'
  exit 0
fi

echo "✓ cargo test --all + clippy (-D warnings) green"
exit 0
