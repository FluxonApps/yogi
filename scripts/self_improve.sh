#!/usr/bin/env bash
# THE CAPSTONE — agent-driven self-improvement (awareness + practice + loop, nothing human).
# The agent ASSESSES its own capability frontier (being-metacog, verifier-grounded), DECIDES to practice
# the frontier (ZPD), PRACTICES + INTERNALIZES (self-gen→verify→distill ratchet on the real qwen3:8b),
# and the ratchet's cold→distilled held-out eval IS the RE-ASSESSMENT (frontier→mastered if it rose).
# The decision is driven by the agent's OWN self-knowledge, not the operator. Foreground/manual only.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

echo "════ 1. ASSESS (awareness) — verifier-grounded: what can the agent learn now? ════"
cargo build -q -p being-goals --bin metacog_assess 2>/dev/null
ASSESS="$(./target/debug/metacog_assess 2>/dev/null)"
echo "$ASSESS" | grep -E "capability map|AWARE" || echo "$ASSESS" | tail -2
FRONTIER="$(printf '%s' "$ASSESS" | grep -oE 'frontier=[0-9]+' | grep -oE '[0-9]+' | head -1)"
FRONTIER="${FRONTIER:-0}"

if [ "$FRONTIER" -eq 0 ]; then
  echo "════ DECIDE — no frontier (nothing learnable-now). Stop. ════"
  exit 0
fi
echo "════ 2. DECIDE — $FRONTIER items at the frontier (ZPD). The agent chooses to practice them. ════"

echo "════ 3. PRACTICE + INTERNALIZE — self-gen verified traces → LoRA (the ratchet, real qwen3:8b) ════"
STUDENT=mlx-community/Qwen3-8B-4bit THINK_OFF=1 BATCH=2 WORK=/tmp/yogi_selfimprove \
  bash scripts/op_ratchet.sh 2>&1 | grep -vE "Fetching|it/s\]"

echo "════ 4. RE-ASSESS — the cold→distilled held-out eval above is the re-assessment: ════"
echo "       if distilled >> cold, the Frontier items became Mastered — the floor rose, agent-driven."
