#!/usr/bin/env bash
# Within-task headroom test: does the agent-loop help BIRD-MODERATE (harder, lower base) MORE than BIRD-SIMPLE
# (easier, higher base)? If so, the headroom law holds at FINER grain (within a task, not just across tasks).
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
.venv-mlx/bin/python - <<'PY'
import sys; sys.path.insert(0,"scripts")
from yogi_harness import Model, OneShot, AgentLoop
from yogi_tasks import BIRDTask
m=Model(); task=BIRDTask(); _,held=task.split(0)
strata={"simple":[e for e in held if e['difficulty']=='simple'], "moderate":[e for e in held if e['difficulty']=='moderate']}
print("\n=== WITHIN-TASK headroom: BIRD by difficulty (one-shot vs agent-loop) ===",flush=True)
for diff,exs in strata.items():
    exs=exs[:40]
    if not exs: continue
    def acc(meth):
        ok=0
        for ex in exs:
            try: p=meth.solve(ex,task,m)
            except Exception: p=None
            if p is not None and task.verify(p,ex): ok+=1
        return ok
    o=acc(OneShot()); a=acc(AgentLoop())
    print(f"  BIRD-{diff:8s} (n={len(exs)}): one-shot {o}/{len(exs)} ({round(100*o/len(exs))}%) -> agent-loop {a}/{len(exs)} ({round(100*a/len(exs))}%)  delta {round(100*(a-o)/len(exs)):+d}",flush=True)
print("\n  headroom law holds WITHIN a task iff agent-loop delta is LARGER on the lower-base (moderate) stratum.",flush=True)
PY
