#!/usr/bin/env bash
# Complete the reachable-headroom validation: pass@8 oracle on code (MBPP, HumanEval) -> reachable-headroom.
# Expect SMALL reachable (matching the small lever deltas +1/+2), completing the 4-task table SQL/ASCII/MBPP/HE.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
.venv-mlx/bin/python - <<'PY'
import sys; sys.path.insert(0,"scripts")
from yogi_harness import Model
from yogi_tasks import MBPPTask, HumanEvalTask
from mlx_lm import generate
from mlx_lm.sample_utils import make_sampler
m=Model(); K=8; N=40; samp=make_sampler(temp=0.8)
def g(prompt,sample=False):
    p=m.t.apply_chat_template([{"role":"user","content":prompt}],add_generation_prompt=True,tokenize=False)
    kw={"sampler":samp} if sample else {}
    o=generate(m.m,m.t,prompt=p,max_tokens=512,verbose=False,**kw)
    if len(m.t.encode(o))>=510 and not sample: o=generate(m.m,m.t,prompt=p,max_tokens=1024,verbose=False)
    return o
print("\n=== pass@8 reachable-headroom on CODE ===",flush=True)
for task in [MBPPTask(), HumanEvalTask()]:
    _,held=task.split(0); test=held[:N]
    o1=sum(task.verify(task.extract(g(task.context(ex)+"\n"+task.instruction())),ex) for ex in test)
    ok=0
    for ex in test:
        base=task.context(ex)+"\n"+task.instruction()
        ok+= any(task.verify(task.extract(g(base,True)),ex) for _ in range(K))
    print(f"  {task.id:14s} one-shot {o1}/{N} ({round(100*o1/N)}%)  pass@{K} {ok}/{N} ({round(100*ok/N)}%)  reachable-headroom {round(100*(ok-o1)/N):+d}",flush=True)
print("\n  4-task table now: SQL reachable +18..22 (lever +11) | ASCII +5 (lever +0) | + code above (levers +1/+2).",flush=True)
PY
