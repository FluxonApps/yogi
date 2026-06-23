#!/usr/bin/env bash
# PHASE 2a-v2 — cross the INDUCTION floor by reformulating induction as PROGRAM-SEARCH, then internalize.
# Phase 2a showed reasoning-induction fails at 8B. Per the thesis (action space is the ceiling): the being
# writes code that brute-forces the rule fitting the examples (free executor), discovers ⊞=5a+3b+7,
# verifies on held-out, then internalizes the SELF-DISCOVERED rule into weights. Recursive action-space
# change applied to invention itself. Zero salary.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit
W=/tmp/yogi_invent2; mkdir -p "$W/data"
"$PY" - "$W" "$STUDENT" <<'PY'
import json,sys,re,os,subprocess,random
from mlx_lm import load,generate
W,STU=sys.argv[1],sys.argv[2]
random.seed(1); NT=" /no_think"
strip=lambda t:t.split('</think>')[-1]
A,B,C=5,3,7; f=lambda a,b:A*a+B*b+C; SYM="⊞"
ex_pairs=[(a,b) for a in range(2,8) for b in range(2,8)]; random.shuffle(ex_pairs)
shown=ex_pairs[:8]; held=ex_pairs[8:16]
test_pairs=[(9,4),(8,9),(9,9),(3,9),(9,6),(4,8),(9,2),(7,9)]
def ask(m,t,p,mx=320):
    return generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":p}],add_generation_prompt=True,tokenize=False),max_tokens=mx,verbose=False)
DENY=("import","open(","exec(","__","subprocess","os.","sys.","socket")
def safe_run(out):
    e=strip(out); m=re.search(r'```(?:python)?\n?(.*?)```',e,re.S); code=(m.group(1) if m else e).strip()
    if not code or any(d in code for d in DENY): return None
    try:
        r=subprocess.run([sys.executable,"-I","-c",code],capture_output=True,timeout=8,text=True)
        return r.stdout.strip() if r.returncode==0 else None
    except Exception: return None
m0,t0=load(STU)
cold=lambda a,b:f"What is {a} {SYM} {b}? Give only the integer.{NT}"
parse=lambda t:(lambda xs:int(xs[-1]) if xs else None)(re.findall(r'-?\d+',strip(t)))
cold_before=sum(parse(ask(m0,t0,cold(a,b),120))==f(a,b) for a,b in test_pairs)
# INDUCTION via PROGRAM-SEARCH: the being writes code that fits the examples
exs=", ".join(f"({a},{b},{f(a,b)})" for a,b in shown)
sp=(f"Examples (a,b,result) of a mystery operator: {exs}. Write Python that brute-forces integer "
    f"coefficients i,j,k each in range(-15,16) such that i*a+j*b+k equals result for EVERY example, and "
    f"prints the formula as the string 'i*a+j*b+k' with the found integers substituted. Output only code.{NT}")
discovered=None
for _ in range(3):
    out=safe_run(ask(m0,t0,sp,400))
    if out:
        s=out.lower().replace(" ","")          # the program prints formatted text; extract the formula
        mm2=re.search(r'(-?\d+\*a[+-]\d+\*b[+-]\d+)',s)
        if mm2:
            e=mm2.group(1)
            try:
                if all(int(eval(e,{"__builtins__":{}},{"a":a,"b":b}))==f(a,b) for a,b in held):
                    discovered=e; break
            except Exception: pass
print(f"INDUCTION-VIA-PROGRAM discovered: {discovered!r}  (truth 5*a+3*b+7) -> {'CORRECT' if discovered else 'FAILED'}",flush=True)
if not discovered:
    print("program-search induction also failed — honest deeper kill.",flush=True); sys.exit(0)
taught=lambda a,b:f"The operator {SYM} is defined by a {SYM} b = {discovered}. What is {a} {SYM} {b}? Show your working, then give the integer.{NT}"
rows=[]; gen_ok=0; TR=[(a,b) for a in range(2,9) for b in range(2,9)]; random.shuffle(TR)
for a,b in TR:
    if len(rows)>=40: break
    r=ask(m0,t0,taught(a,b),200)
    if parse(r)==f(a,b): gen_ok+=1; rows.append({"prompt":cold(a,b),"completion":" "+strip(r).strip()})
print(f"self-gen via self-discovered rule: {gen_ok} traces",flush=True)
d=f"{W}/data"; open(f"{d}/train.jsonl","w").write("\n".join(json.dumps(r) for r in rows)+"\n")
open(f"{d}/valid.jsonl","w").write("\n".join(json.dumps(r) for r in rows[:6])+"\n")
del m0,t0
ad=f"{W}/adapter"; os.makedirs(ad,exist_ok=True)
subprocess.run([sys.executable,"-m","mlx_lm.lora","--model",STU,"--train","--data",d,"--batch-size","2",
  "--num-layers","16","--iters","200","--learning-rate","1e-4","--adapter-path",ad],capture_output=True,text=True)
m1,t1=load(STU,adapter_path=ad)
cold_after=sum(parse(ask(m1,t1,cold(a,b),300))==f(a,b) for a,b in test_pairs)
print(f"\n=== PHASE 2a-v2 RESULT (induction-by-program-search, then internalize) ===",flush=True)
print(f"  discovered: {discovered} (truth 5a+3b+7)",flush=True)
print(f"  cold {SYM}: before {cold_before}/8 -> after {cold_after}/8",flush=True)
print(f"  ✓ iff discovered correct (induction floor crossed via program) AND cold after >> before (self-discovered rule internalized).",flush=True)
PY
