#!/usr/bin/env bash
# Lever COMPOSITION on weak-base SQL: do decompose + agent-loop STACK? Compare one-shot / decompose / agent-loop
# / decompose+loop on BIRD n=80. If combined > best-single, the levers compose (toward the full-stack ceiling ~53).
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
.venv-mlx/bin/python - <<'PY'
import sys; sys.path.insert(0,"scripts")
from yogi_harness import Model, OneShot, Decompose, AgentLoop, Combined, evaluate
from yogi_tasks import BIRDTask
m=Model(); task=BIRDTask()
print("\n=== LEVER COMPOSITION on BIRD (weak base), n=80 ===",flush=True)
for meth in [OneShot(), Decompose(), AgentLoop(), Combined()]:
    r=evaluate(task, meth, m, n=80, verbose=False)
    print(f"  {meth.name:14s} {r['ok']}/{r['n']} ({r['acc']}%)",flush=True)
print("\n  Levers STACK iff decompose+loop > best single lever (toward the ~53 full-stack).",flush=True)
PY
