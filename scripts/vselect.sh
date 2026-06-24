#!/usr/bin/env bash
# Reproducible verified-selection moat demo: for each task, baseline=OneShot, then verified_select keeps a
# candidate lever only if held-out acc beats baseline (the moat). Live run (~40min); the decision matches the
# measured n=80 table in FINDINGS (BIRD adopts agent-loop+decompose; code prunes both).
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
.venv-mlx/bin/python - <<'PY'
import sys; sys.path.insert(0,"scripts")
from yogi_harness import Model, OneShot, AgentLoop, Decompose, evaluate, verified_select
from yogi_tasks import BIRDTask, MBPPTask
m=Model()
for task in [BIRDTask(), MBPPTask()]:
    base=evaluate(task, OneShot(), m, n=80, verbose=False)["acc"]
    print(f"\n[{task.id}] baseline(one-shot)={base}")
    kept=verified_select(task, m, [AgentLoop(), Decompose()], base, val_n=80)
    print(f"  -> adopted: {[c.name for c,_ in kept] or 'none'}")
PY
