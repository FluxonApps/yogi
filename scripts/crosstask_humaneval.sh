#!/usr/bin/env bash
# Confirm the headroom prediction: agent-loop on LOWER-base code (HumanEval). If it lifts here (more than the
# +1 on high-base MBPP), it confirms the lever generalizes to code WHEN fixable headroom exists.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
.venv-mlx/bin/python - <<'PY'
import sys; sys.path.insert(0,"scripts")
from yogi_harness import Model, OneShot, AgentLoop, evaluate
from yogi_tasks import HumanEvalTask
print("loading model once...",flush=True); m=Model(); task=HumanEvalTask(); res={}
print("\n=== GENERALIZATION confirm: agent-loop on LOWER-base code (HumanEval) ===",flush=True)
for meth in [OneShot(), AgentLoop()]:
    r=evaluate(task, meth, m, n=80, verbose=False); res[meth.name]=r["acc"]
    print(f"  HumanEval {meth.name:11s} {r['ok']}/{r['n']} ({r['acc']}%)",flush=True)
print(f"\n  agent-loop delta = {res.get('agent-loop',0)-res.get('one-shot',0):+d} pts (MBPP was +1 @70% base; SQL +11 @37% base). Confirms headroom-dependence.",flush=True)
PY
