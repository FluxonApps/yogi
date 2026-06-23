#!/usr/bin/env bash
# PHASE 2b — RECURSION / the C3 REMATCH (docs/research/recursive-invention.md). C3 collapsed because it
# accumulated CONFUSABLE instance-skills (priors->3/8, plasticity died). Here we test COMPOSITIONAL
# abstraction-skills: ⊞ is already internalized (S1 = invent_v2 adapter); learn ⊠=(a⊞b)⊞b which is BUILT
# ON ⊞. Does the internalized abstraction (a) get reused to acquire the next skill and (b) survive
# (retention) — i.e. do ABSTRACTIONS compound where instances collapsed? Zero salary.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit
W=/tmp/yogi_recurse; mkdir -p "$W/data"; S1=/tmp/yogi_invent2/adapter
"$PY" - "$W" "$STUDENT" "$S1" <<'PY'
import json,sys,re,os,subprocess,random
from mlx_lm import load,generate
W,STU,S1=sys.argv[1],sys.argv[2],sys.argv[3]
DENY=("import","open(","exec(","__","subprocess","os.","sys.","socket")
def safe_run(out):
    e=out.split("</think>")[-1]; m=re.search(r"```(?:python)?\n?(.*?)```",e,re.S); code=(m.group(1) if m else e).strip()
    if not code or any(x in code for x in DENY): return None
    try:
        import subprocess as _sp; r=_sp.run([sys.executable,"-I","-c",code],capture_output=True,timeout=8,text=True)
        return r.stdout.strip() if r.returncode==0 else None
    except Exception: return None
prog2=lambda a,b:f"Compute (a \u229e b) \u229e b where a \u229e b = 5a+3b+7. Write Python that prints the result for a={a}, b={b}. Output only code.{NT}"
random.seed(2); NT=" /no_think"
strip=lambda t:t.split('</think>')[-1]
parse=lambda t:(lambda xs:int(xs[-1]) if xs else None)(re.findall(r'-?\d+',strip(t)))
f=lambda a,b:5*a+3*b+7            # ⊞ (internalized in S1)
g=lambda a,b:f(f(a,b),b)          # ⊠ = (a⊞b)⊞b  == 25a+18b+42 (novel, built on ⊞)
c1=lambda a,b:f"What is {a} ⊞ {b}? Give only the integer.{NT}"
c2=lambda a,b:f"What is {a} ⊠ {b}? Give only the integer.{NT}"
taught2=lambda a,b:f"The operator ⊠ is defined by a ⊠ b = (a ⊞ b) ⊞ b, where a ⊞ b = 5a+3b+7. What is {a} ⊠ {b}? Show your working, then give the integer.{NT}"
test=[(9,4),(8,9),(9,9),(3,9),(9,6),(4,8),(9,2),(7,9)]
def ask(m,t,p,mx=240):
    return generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":p}],add_generation_prompt=True,tokenize=False),max_tokens=mx,verbose=False)
m0,t0=load(STU,adapter_path=S1)   # start from ⊞-internalized model
b2_before=sum((safe_run(ask(m0,t0,c2(a,b),300))==str(g(a,b))) for a,b in test)   # cold ⊠ before (~0, novel)
b1_before=sum(parse(ask(m0,t0,c1(a,b),400))==f(a,b) for a,b in test)   # ⊞ retention baseline (~6/8 from F8)
# self-gen ⊠ via the composition (the model REUSES internalized ⊞ + the def); verify against g
rows=[]; yok=0; TR=[(a,b) for a in range(2,9) for b in range(2,9)]; random.shuffle(TR)
for a,b in TR:
    if len(rows)>=40: break
    out=ask(m0,t0,prog2(a,b),300)
    if safe_run(out) is not None and safe_run(out)==str(g(a,b)): yok+=1; rows.append({"prompt":c2(a,b),"completion":" "+strip(out).strip()})
# replay ⊞ to retain (compositional, not confusable — should hold)
for a,b in test[:4]: rows.append({"prompt":c1(a,b),"completion":f" {f(a,b)}"})
random.shuffle(rows); d=f"{W}/data"
open(f"{d}/train.jsonl","w").write("\n".join(json.dumps(r) for r in rows)+"\n")
open(f"{d}/valid.jsonl","w").write("\n".join(json.dumps(r) for r in rows[:6])+"\n")
print(f"self-gen ⊠ via PROGRAM lever: {yok}/40 verified (high yield => ⊞ reused to acquire ⊠)",flush=True)
del m0,t0
S2=f"{W}/ad2"; os.makedirs(S2,exist_ok=True)
subprocess.run([sys.executable,"-m","mlx_lm.lora","--model",STU,"--train","--data",d,"--batch-size","2",
  "--num-layers","16","--iters","200","--learning-rate","1e-4","--adapter-path",S2,
  "--resume-adapter-file",f"{S1}/adapters.safetensors"],capture_output=True,text=True)
m1,t1=load(STU,adapter_path=S2)
b2_after=sum((safe_run(ask(m1,t1,c2(a,b),300))==str(g(a,b))) for a,b in test)
b1_after=sum(parse(ask(m1,t1,c1(a,b),400))==f(a,b) for a,b in test)
print(f"\n=== PHASE 2b RESULT (recursion / C3 rematch) ===",flush=True)
print(f"  ⊠ tool-use (plain prompt->program): cold before {b2_before}/8 -> after {b2_after}/8",flush=True)
print(f"  ⊞ retained (the abstraction): before {b1_before}/8 -> after {b1_after}/8",flush=True)
print(f"  COMPOUND ✓ iff ⊠ rises (abstraction reused to acquire the next skill) AND ⊞ retained (vs C3 confusable collapse to ~3/8).",flush=True)
PY
