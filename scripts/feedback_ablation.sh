#!/usr/bin/env bash
# MECHANISM: does the agent-loop gain come from execution-FEEDBACK CONTENT or just from RETRYING? On BIRD (the
# task where the agent-loop helps most), compare one-shot / agent-loop-min (generic 'incorrect, retry') /
# agent-loop (rich execution feedback: error or wrong-rowcount). n=80.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
.venv-mlx/bin/python - <<'PY'
import sys; sys.path.insert(0,"scripts")
from yogi_harness import Model, OneShot, MinimalLoop, AgentLoop, evaluate
from yogi_tasks import BIRDTask
m=Model(); task=BIRDTask(); res={}
print("\n=== FEEDBACK ABLATION on BIRD (content vs retries), n=80 ===",flush=True)
for meth in [OneShot(), MinimalLoop(), AgentLoop()]:
    r=evaluate(task, meth, m, n=80, verbose=False); res[meth.name]=r["acc"]
    print(f"  {meth.name:14s} {r['ok']}/{r['n']} ({r['acc']}%)",flush=True)
gm=res.get("agent-loop-min",0)-res.get("one-shot",0); gr=res.get("agent-loop",0)-res.get("agent-loop-min",0)
print(f"\n  retries-alone gain = {gm:+d}; feedback-CONTENT gain (rich - min) = {gr:+d}.",flush=True)
print("  If feedback-content gain >> retries-alone, the agent-loop's value is the EXECUTION FEEDBACK, not just resampling.",flush=True)
PY
