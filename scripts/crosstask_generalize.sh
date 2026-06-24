#!/usr/bin/env bash
# GENERALIZATION sweep: does the agent-loop lever (solve->execute->observe->fix) that lifted SQL 37->48 ALSO
# lift a DIFFERENT domain (code/MBPP)? OneShot vs AgentLoop on MBPP at n=80, via the SAME harness. The delta
# (and whether it matches BIRD's one-shot->agent-loop delta) IS the generalization result.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
.venv-mlx/bin/python - <<'PY'
import sys; sys.path.insert(0,"scripts")
from yogi_harness import Model, OneShot, AgentLoop, evaluate
from yogi_tasks import MBPPTask
print("loading model once...",flush=True); m=Model()
task=MBPPTask()
print("\n=== GENERALIZATION: agent-loop lever on CODE (MBPP), via the same harness ===",flush=True)
res={}
for meth in [OneShot(), AgentLoop()]:
    r=evaluate(task, meth, m, n=80, verbose=False); res[meth.name]=r["acc"]
    print(f"  MBPP {meth.name:11s} {r['ok']}/{r['n']} ({r['acc']}%)",flush=True)
d=res.get("agent-loop",0)-res.get("one-shot",0)
print(f"\n  agent-loop delta on CODE = {d:+d} pts. BIRD/SQL delta was +11 (37->48). GENERALIZES iff the agent-loop lifts code too (the lever is domain-independent, not SQL-specific).",flush=True)
PY
