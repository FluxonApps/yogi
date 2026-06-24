#!/usr/bin/env bash
# Mechanism follow-up: if the agent-loop gain is verifier-gated RETRY (resample on verified-wrong), more rounds
# should push toward the pass@k oracle (~52-55 on BIRD) then plateau. MinimalLoop @ rounds 2/4/6 on BIRD n=80.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
.venv-mlx/bin/python - <<'PY'
import sys; sys.path.insert(0,"scripts")
from yogi_harness import Model, OneShot, MinimalLoop, evaluate
from yogi_tasks import BIRDTask
m=Model(); task=BIRDTask()
print("\n=== ROUNDS SCALING (verifier-gated retry) on BIRD, n=80 ===",flush=True)
b=evaluate(task, OneShot(), m, n=80, verbose=False)["acc"]; print(f"  rounds=0 (one-shot) {b}%",flush=True)
prev=b
for R in (2,4,6):
    meth=MinimalLoop(); meth.rounds=R
    a=evaluate(task, meth, m, n=80, verbose=False)["acc"]
    print(f"  rounds={R:<2d} retry-loop {a}%  (delta vs prev {a-prev:+d})",flush=True); prev=a
print("\n  Retry-as-resampling iff acc rises with rounds toward the pass@k oracle (~52-55) then plateaus.",flush=True)
PY
