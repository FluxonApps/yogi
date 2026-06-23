#!/usr/bin/env bash
# PHASE 2a — INVENTION (docs/research/recursive-invention.md). Strict escalation of F1: there we GAVE the
# rule (taught); here the being must DISCOVER a novel rule from examples alone (no teacher), verify it
# (free, on held-out examples), then internalize the SELF-DISCOVERED rule into weights. Beyond
# menu-selection (Phase 1) and beyond DreamCoder (weights, not an external library). Zero salary.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit
W=/tmp/yogi_invent; mkdir -p "$W/data"
"$PY" - "$W" "$STUDENT" <<'PY'
import json,sys,re,os,subprocess,random
from mlx_lm import load,generate
W,STU=sys.argv[1],sys.argv[2]
random.seed(1); NT=" /no_think"
strip=lambda t:t.split('</think>')[-1]
# HIDDEN novel operator the model must DISCOVER from examples (unseen coeffs; cold≈0):
A,B,C=5,3,7   # f(a,b)=5a+3b+7  — novel, discoverable by induction from examples
f=lambda a,b:A*a+B*b+C
SYM="⊞"
ex_pairs=[(a,b) for a in range(2,8) for b in range(2,8)]; random.shuffle(ex_pairs)
shown=ex_pairs[:8]; held_disc=ex_pairs[8:16]      # examples shown vs held-out for verifying a hypothesis
test_pairs=[(9,4),(8,9),(9,9),(3,9),(9,6),(4,8),(9,2),(7,9)]   # unseen-operand held-out for internalization
def ask(m,t,p,mx=220):
    return generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":p}],add_generation_prompt=True,tokenize=False),max_tokens=mx,verbose=False)
m0,t0=load(STU)
# 0. cold baseline (model has never seen ⊞): expect ~0
cold=lambda a,b:f"What is {a} {SYM} {b}? Give only the integer.{NT}"
parse=lambda t:(lambda xs:int(xs[-1]) if xs else None)(re.findall(r'-?\d+',strip(t)))
cold_before=sum(parse(ask(m0,t0,cold(a,b),120))==f(a,b) for a,b in test_pairs)
# 1. INVENT: show examples, ask the model to PROPOSE candidate formulas (open-ended, not a menu)
exs="; ".join(f"{a} {SYM} {b} = {f(a,b)}" for a,b in shown)
prop=(f"Here are examples of a mystery operator: {exs}. "
      f"Propose FIVE different candidate Python expressions in terms of a and b that could define a {SYM} b. "
      f"Output each on its own line as: a {SYM} b = <expression>.{NT}")
cands=[]
for _ in range(3):  # a few samples to widen the hypothesis set
    out=strip(ask(m0,t0,prop,320))
    for line in out.splitlines():
        mm=re.search(r'=\s*([0-9aAbB\+\-\*\(\)\s]+)$',line.strip())
        if mm:
            e=mm.group(1).strip().lower()
            if e and re.fullmatch(r'[0-9ab\+\-\*\(\)\s]+',e): cands.append(e)
cands=list(dict.fromkeys(cands))[:20]
print(f"INVENTED {len(cands)} candidate rules (sample): {cands[:6]}",flush=True)
# 2. VERIFY each candidate on held-out examples (free verifier); keep the consistent one
def consistent(e):
    try: return all(int(eval(e,{"__builtins__":{}},{"a":a,"b":b}))==f(a,b) for a,b in held_disc)
    except Exception: return False
discovered=next((e for e in cands if consistent(e)),None)
print(f"DISCOVERED RULE: {discovered!r}  (truth: {A}*a+{B}*b+{C})  -> {'CORRECT' if discovered and all(int(eval(discovered,{'__builtins__':{}},{'a':a,'b':b}))==f(a,b) for a,b in test_pairs) else 'none/incorrect'}",flush=True)
if not discovered:
    print("INVENTION FAILED at 8B scale — honest kill: the being could not induce the rule from examples.",flush=True); sys.exit(0)
# 3. self-gen: solve WITH the self-discovered rule in-context (CoT), verify, distill cold->solution
taught=lambda a,b:f"The operator {SYM} is defined by a {SYM} b = {discovered}. What is {a} {SYM} {b}? Show your working, then give the integer.{NT}"
rows=[]; gen_ok=0; TR=[(a,b) for a in range(2,9) for b in range(2,9)]; random.shuffle(TR)
for a,b in TR:
    if len(rows)>=40: break
    r=ask(m0,t0,taught(a,b),200)
    if parse(r)==f(a,b): gen_ok+=1; rows.append({"prompt":cold(a,b),"completion":" "+strip(r).strip()})
print(f"self-gen via self-discovered rule: {gen_ok} verified traces",flush=True)
d=f"{W}/data"; open(f"{d}/train.jsonl","w").write("\n".join(json.dumps(r) for r in rows)+"\n")
open(f"{d}/valid.jsonl","w").write("\n".join(json.dumps(r) for r in rows[:6])+"\n")
del m0,t0
ad=f"{W}/adapter"; os.makedirs(ad,exist_ok=True)
subprocess.run([sys.executable,"-m","mlx_lm.lora","--model",STU,"--train","--data",d,
  "--batch-size","2","--num-layers","16","--iters","200","--learning-rate","1e-4","--adapter-path",ad],capture_output=True,text=True)
m1,t1=load(STU,adapter_path=ad)
cold_after=sum(parse(ask(m1,t1,cold(a,b),300))==f(a,b) for a,b in test_pairs)
print(f"\n=== PHASE 2a RESULT (invent-the-rule-then-internalize) ===",flush=True)
print(f"  discovered rule: {discovered}  (truth 5a+3b+7)",flush=True)
print(f"  cold {SYM} (no rule, no examples): before {cold_before}/8  ->  after {cold_after}/8",flush=True)
print(f"  INVENTION ✓ iff rule discovered correctly AND cold after >> before (self-DISCOVERED rule internalized, no teacher ever gave it).",flush=True)
PY
