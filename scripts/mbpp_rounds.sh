#!/usr/bin/env bash
# Confirm the resample-budget interpretation: MBPP reachable@8 = +12. Does retry-loop @ rounds 2/4/8 climb
# toward +12 (low per-sample prob -> needs more draws), unlike SQL which saturated at 2 (high per-sample prob)?
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
.venv-mlx/bin/python - <<'PY'
import sys; sys.path.insert(0,"scripts")
from yogi_harness import Model, OneShot, MinimalLoop, evaluate
from yogi_tasks import MBPPTask
m=Model(); task=MBPPTask()
print("\n=== MBPP ROUNDS SCALING (budget interpretation; reachable@8=+12) n=80 ===",flush=True)
b=evaluate(task, OneShot(), m, n=80, verbose=False)["acc"]; print(f"  rounds=0 (one-shot) {b}%",flush=True)
for R in (2,4,8):
    meth=MinimalLoop(); meth.rounds=R
    a=evaluate(task, meth, m, n=80, verbose=False)["acc"]
    print(f"  rounds={R:<2d} retry {a}% (delta vs base {a-b:+d})",flush=True)
print("\n  Budget interpretation holds iff MBPP retry climbs toward +12 with rounds (vs SQL saturating at 2).",flush=True)
PY
