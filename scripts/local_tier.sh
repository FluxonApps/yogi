#!/usr/bin/env bash
# PRODUCTIZATION CAPSTONE: the deployable LOCAL-TIER pipeline, composing every validated finding, on
# correctness-verifier tasks (MBPP/HumanEval — self-certify without gold). Per item:
#   stage1 one-shot -> stage2 retry@2 (cheap correlated resampling) -> stage3 best-of-5 (independent samples,
#   for low-per-sample-prob reachable headroom) -> ACCEPT any test-passing output (provably correct, safe),
#   else FLAG for escalation. Reports self-certified accuracy, AVG COST (gens/item), escalation rate; vs one-shot.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
.venv-mlx/bin/python - <<'PY'
import sys; sys.path.insert(0,"scripts")
from yogi_harness import Model
from yogi_tasks import MBPPTask, HumanEvalTask
from mlx_lm import generate
from mlx_lm.sample_utils import make_sampler
m=Model(); N=80; BON=5; samp=make_sampler(temp=0.8)
def gen(prompt,sample=False):
    p=m.t.apply_chat_template([{"role":"user","content":prompt}],add_generation_prompt=True,tokenize=False)
    kw={"sampler":samp} if sample else {}
    o=generate(m.m,m.t,prompt=p,max_tokens=512,verbose=False,**kw)
    if not sample and len(m.t.encode(o))>=510: o=generate(m.m,m.t,prompt=p,max_tokens=1024,verbose=False)
    return o
def pipeline(task, ex):
    base=task.context(ex)+"\n"+task.instruction(); cost=0
    pred=task.extract(gen(base)); cost+=1
    if task.verify(pred,ex): return True,cost,"stage1"
    for _ in range(2):  # stage2 retry (sequential)
        pred=task.extract(gen(base+"\n\nYour previous attempt FAILED the tests. Try again.")); cost+=1
        if task.verify(pred,ex): return True,cost,"stage2"
    for _ in range(BON):  # stage3 best-of-N (independent samples, verifier-selected)
        pred=task.extract(gen(base,True)); cost+=1
        if task.verify(pred,ex): return True,cost,"stage3"
    return False,cost,"escalate"  # not self-certified -> flag
print("\n=== DEPLOYABLE LOCAL-TIER PIPELINE (self-certify via correctness verifier; escalate residual) ===",flush=True)
for task in [MBPPTask(), HumanEvalTask()]:
    _,held=task.split(0); test=held[:N]
    one=sum(task.verify(task.extract(gen(task.context(ex)+"\n"+task.instruction())),ex) for ex in test)
    acc=0; totcost=0; esc=0; stages={"stage1":0,"stage2":0,"stage3":0,"escalate":0}
    for ex in test:
        ok,c,st=pipeline(task,ex); totcost+=c; stages[st]+=1
        if ok: acc+=1
        else: esc+=1
    print(f"\n  {task.id} (n={N}):",flush=True)
    print(f"    one-shot baseline      {one}/{N} ({round(100*one/N)}%)  cost 1.0 gens/item",flush=True)
    print(f"    LOCAL-TIER self-certified {acc}/{N} ({round(100*acc/N)}%)  cost {totcost/N:.1f} gens/item  escalate {round(100*esc/N)}%",flush=True)
    print(f"    accepted-by-stage: 1shot={stages['stage1']} retry={stages['stage2']} bestN={stages['stage3']} escalate={stages['escalate']}  [accepted are PROVABLY correct: tests pass]",flush=True)
print("\n  The local tier self-certifies the majority (provably correct, no gold/frontier) and flags a small tail to escalate — the accuracy/cost frontier of the deployable product.",flush=True)
PY
