#!/usr/bin/env bash
# Recover the stats numbers from SAVED adapters (the wrapper's Fetching-filter ate the PASS lines, but
# the adapters persist). Eval-only, no retrain. Same n=40 held-out + eval.py for all seeds.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit
EV=/tmp/yogi_stats_1/eval.py; TEST=/tmp/yogi_stats_1/data/test.jsonl
echo "recover stats — operator ⊕, held-out n=40 (unseen operands), saved adapters:"
echo -n "  cold (no adapter): "; EVAL_MAX=256 "$PY" "$EV" "$STUDENT" - "$TEST"
counts=""
for s in 1 2 3; do
  ad="/tmp/yogi_stats_$s/adapter"
  [ -f "$ad/adapters.safetensors" ] || { echo "  seed $s: (no adapter)"; continue; }
  d=$(EVAL_MAX=256 "$PY" "$EV" "$STUDENT" "$ad" "$TEST" | grep -oE '[0-9]+/[0-9]+' | head -1)
  echo "  seed $s distilled: $d"; counts="$counts $(printf '%s' "$d" | cut -d/ -f1)"
done
python3 -c "import statistics as st; xs=[int(x) for x in '$counts'.split()]; print(f'  DISTILLED mean={st.mean(xs):.1f}/40 = {st.mean(xs)/40:.0%}  std={st.pstdev(xs):.1f}  (n_seeds={len(xs)})') if xs else print('  no adapters')"
