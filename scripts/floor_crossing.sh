#!/usr/bin/env bash
# BET B PHASE 1 (docs/research/floor-crossing-ratchet.md) — the closed floor-crossing ratchet.
# A below-floor task (8B fails even WITH help). Give it a reformulation MENU; autonomously SELECT the one
# that crosses (by free-verifier taught-pass); ratchet+distill the crossing one; then ask the crux:
# does distilling reformulation-traces INTERNALIZE the skill (direct improves) or only the scaffold
# (reformulation improves)? Task = multi-digit multiplication (free verifier = the product). One model.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit
W=/tmp/yogi_floor; mkdir -p "$W/data"
"$PY" - "$W" "$STUDENT" <<'PY'
import json,sys,re,os,subprocess,random
from mlx_lm import load,generate
W,STU=sys.argv[1],sys.argv[2]
strip=lambda t:t.split('</think>')[-1]
ints=lambda t:re.findall(r'-?\d+',strip(t).replace(',',''))
last=lambda t:(lambda xs:int(xs[-1]) if xs else None)(ints(t))
def gen(m,tk,p,mx=320):
    txt=tk.apply_chat_template([{"role":"user","content":p}],add_generation_prompt=True,tokenize=False)
    return generate(m,tk,prompt=txt,max_tokens=mx,verbose=False)
random.seed(0)
def probs(nd,k):  # k multiplication problems with nd-digit factors
    lo,hi=10**(nd-1),10**nd-1
    return [(random.randint(lo,hi),random.randint(lo,hi)) for _ in range(k)]
NT=" /no_think"
# reformulations: cold(direct), taught(direct), program(python), decompose(partial products)
R_direct=lambda a,b:f"What is {a} * {b}? Give only the integer.{NT}"
R_taught=lambda a,b:f"Compute {a} * {b}. Work carefully step by step, then give the integer.{NT}"
R_prog  =lambda a,b:f"Write a single Python expression (only the expression) that computes {a} * {b}.{NT}"
R_decomp=lambda a,b:f"Compute {a} * {b} by expanding the second factor into place-value parts (units, tens, hundreds...), multiply each, then sum. Show the parts and give the final integer.{NT}"
def run_prog(out):  # extract & eval a python expr safely (digits/operators only)
    e=strip(out); m=re.search(r'([0-9][\d\s\*\+\-\(\)]*[0-9])',e)
    if not m: return None
    expr=m.group(1)
    if not re.fullmatch(r'[\d\s\*\+\-\(\)]+',expr): return None
    try: return int(eval(expr,{"__builtins__":{}},{}))
    except Exception: return None
m0,t0=load(STU)
# 1. PROBE the floor: find digit-count where DIRECT-taught is below floor (<~30%)
print("=== probe floor (direct-taught pass by digit count) ===",flush=True)
floor_nd=None; PB=probs  # cache
for nd in [2,3,4,5]:
    P=probs(nd,8); ok=sum(last(gen(m0,t0,R_taught(a,b)))==a*b for a,b in P)
    print(f"  {nd}-digit: taught {ok}/8",flush=True)
    if ok<=2 and floor_nd is None: floor_nd=nd
floor_nd=floor_nd or 4
print(f"FLOOR at {floor_nd}-digit multiplication (direct-taught fails).",flush=True)
# 2. SELECT: at the floor, measure each reformulation's verifier-pass; pick crossing ones (>=6/8)
P=probs(floor_nd,8)
def passrate(promptf,extract):
    return sum((extract(gen(m0,t0,promptf(a,b)))==a*b) for a,b in P)
sel={}
sel["direct"]=passrate(R_taught,last)
sel["program"]=passrate(R_prog,run_prog)
sel["decompose"]=passrate(R_decomp,last)
print(f"=== reformulation pass @ {floor_nd}-digit: {sel} ===",flush=True)
winner=max(("program","decompose"),key=lambda r:sel[r])
print(f"AUTONOMOUS SELECT (by verifier pass): '{winner}' (crosses iff >> direct {sel['direct']}/8)",flush=True)
# 3. RATCHET: self-gen verified traces via the winning reformulation; distill cold(direct)->solution
genf = R_prog if winner=="program" else R_decomp
extract = run_prog if winner=="program" else last
TR=probs(floor_nd,60); rows=[]; solved=0
for a,b in TR:
    if len(rows)>=40: break
    out=gen(m0,t0,genf(a,b))
    if extract(out)==a*b:
        solved+=1
        # distill the model's OWN crossing solution under the COLD (direct) prompt → internalize
        rows.append({"prompt":R_direct(a,b),"completion":" "+strip(out).strip()})
print(f"self-gen via {winner}: {solved} verified traces",flush=True)
d=f"{W}/data"; open(f"{d}/train.jsonl","w").write("\n".join(json.dumps(r) for r in rows)+"\n")
open(f"{d}/valid.jsonl","w").write("\n".join(json.dumps(r) for r in rows[:6])+"\n")
# direct-before baseline on a held-out test set
TE=probs(floor_nd,12)
direct_before=sum(last(gen(m0,t0,R_direct(a,b)))==a*b for a,b in TE)
del m0,t0
# 4. LoRA
ad=f"{W}/adapter"; os.makedirs(ad,exist_ok=True)
subprocess.run([sys.executable,"-m","mlx_lm.lora","--model",STU,"--train","--data",d,
  "--batch-size","2","--num-layers","16","--iters","200","--learning-rate","1e-4","--adapter-path",ad],
  capture_output=True,text=True)
# 5. EVAL the crux: does direct improve (skill internalized) or only the reformulation (scaffold)?
m1,t1=load(STU,adapter_path=ad)
direct_after=sum(last(gen(m1,t1,R_direct(a,b)))==a*b for a,b in TE)
reform_after=sum((extract(gen(m1,t1,genf(a,b)))==a*b) for a,b in TE)
print(f"\n=== BET B PHASE 1 RESULT ({floor_nd}-digit mult, held-out n={len(TE)}) ===",flush=True)
print(f"  selected reformulation: {winner}  (taught/program/decompose pass = {sel})",flush=True)
print(f"  DIRECT cold:  before {direct_before}/{len(TE)}  ->  after {direct_after}/{len(TE)}   (skill internalized?)",flush=True)
print(f"  via {winner}: after {reform_after}/{len(TE)}   (scaffold internalized?)",flush=True)
print("  Floor-crossing ✓ = a reformulation crosses where direct fails; Internalization ✓ = DIRECT after >> before.",flush=True)
PY
