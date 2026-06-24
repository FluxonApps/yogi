#!/usr/bin/env bash
# Second-lever generalization: does DECOMPOSE (plan-then-answer) follow the same headroom law as the agent-loop?
# OneShot vs Decompose on a WEAK-base task (BIRD/SQL 37) and a STRONG-base task (MBPP code 70), n=80, same harness.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
.venv-mlx/bin/python - <<'PY'
import sys; sys.path.insert(0,"scripts")
from yogi_harness import Model, OneShot, Decompose, evaluate
from yogi_tasks import BIRDTask, MBPPTask
print("loading model once...",flush=True); m=Model()
print("\n=== DECOMPOSE generalization (plan-then-answer) vs base, n=80 ===",flush=True)
for task in [BIRDTask(), MBPPTask()]:
    res={}
    for meth in [OneShot(), Decompose()]:
        r=evaluate(task, meth, m, n=80, verbose=False); res[meth.name]=r["acc"]
        print(f"  {task.id:13s} {meth.name:9s} {r['ok']}/{r['n']} ({r['acc']}%)",flush=True)
    print(f"  -> {task.id} decompose delta {res.get('decompose',0)-res.get('one-shot',0):+d}",flush=True)
print("\n  GENERALIZES on the headroom law iff decompose helps the WEAK-base task (SQL) more than the STRONG-base task (code).",flush=True)
PY
