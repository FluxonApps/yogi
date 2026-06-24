#!/usr/bin/env bash
# Anchoring test: MBPP has +12 reachable headroom but rich-feedback agent-loop realized only +1. Does FRESH retry
# (MinimalLoop: "incorrect, try a different answer") capture more than RICH ("fix this test failure: <trace>")?
# If MinimalLoop > AgentLoop on MBPP, rich feedback ANCHORS on the broken code (confirms the mechanism). n=80.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
.venv-mlx/bin/python - <<'PY'
import sys; sys.path.insert(0,"scripts")
from yogi_harness import Model, OneShot, MinimalLoop, AgentLoop, evaluate
from yogi_tasks import MBPPTask
m=Model(); task=MBPPTask(); res={}
print("\n=== MBPP ANCHORING TEST (fresh retry vs rich 'fix-this'), n=80 ===",flush=True)
for meth in [OneShot(), MinimalLoop(), AgentLoop()]:
    r=evaluate(task, meth, m, n=80, verbose=False); res[meth.name]=r["acc"]
    print(f"  {meth.name:14s} {r['ok']}/{r['n']} ({r['acc']}%)",flush=True)
mn=res.get("agent-loop-min",0); rich=res.get("agent-loop",0); base=res.get("one-shot",0)
print(f"\n  fresh-retry gain {mn-base:+d}; rich-feedback gain {rich-base:+d}; anchoring penalty (rich-fresh) {rich-mn:+d}.",flush=True)
print("  If fresh-retry > rich, rich 'fix the broken code' feedback ANCHORS and under-realizes reachable headroom (+12) on code.",flush=True)
PY
