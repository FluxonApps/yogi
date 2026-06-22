#!/usr/bin/env bash
# STATS (F1 credibility): operator ⊕=3a+2b ratchet at 3 seeds × n=40 unseen-operand held-out → mean±std.
# Turns the headline "0→8/8 (n=8, 1 seed)" into a defensible number. Real qwen3:8b, free verifier.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
echo "skill=⊕(3a+2b)  held-out n=40 (unseen operands 9-12)  seeds=1,2,3"
counts=""
for s in 1 2 3; do
  o=$(STUDENT=mlx-community/Qwen3-8B-4bit GOAL_KIND=numeric BIG_HELDOUT=1 EVAL_MAX=64 SEED=$s THINK_OFF=1 BATCH=2 \
      WORK="/tmp/yogi_stats_$s" bash scripts/op_ratchet.sh 2>&1 | grep -vE "Fetching|it/s\]")
  cold=$(printf '%s' "$o" | grep -E '^  cold' | grep -oE '[0-9]+/[0-9]+' | head -1)
  dist=$(printf '%s' "$o" | grep 'distilled' | grep -oE '[0-9]+/[0-9]+' | head -1)
  echo "seed $s:  cold $cold   distilled $dist"
  counts="$counts $(printf '%s' "$dist" | cut -d/ -f1)"
done
echo "distilled correct (/80) across seeds:$counts"
python3 -c "import statistics as st; xs=[int(x) for x in '$counts'.split()]; print(f'DISTILLED mean={st.mean(xs):.1f}/80 = {st.mean(xs)/80:.0%}  std={st.pstdev(xs):.1f}  (n_seeds={len(xs)})') if xs else print('no data')"
