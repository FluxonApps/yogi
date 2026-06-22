#!/usr/bin/env bash
# SEQUENTIAL continual-learning test (the honest hard version of compounding): the being learns three
# novel skills ONE ROUND AT A TIME (not co-trained), each round resuming from the prior adapter +
# REPLAYing a little of the earlier skills (M3's lever). After EACH round we eval ALL skills so far —
# does the cumulative floor GROW (continual compounding) or do new skills ERASE old ones (catastrophic
# forgetting)? Real qwen3:8b, free verifiers, zero salary.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
VENV="${VENV:-.venv-mlx}"; PY="$VENV/bin/python"
STUDENT="${STUDENT:-mlx-community/Qwen3-8B-4bit}"
WORK="${WORK:-/tmp/yogi_seq}"; DATA="$WORK/data"; ITERS="${ITERS:-250}"; LAYERS=16; LR=1e-4; BATCH=2
mkdir -p "$DATA"

# 1. Self-generate per-skill verified traces + held-out tests (once).
"$PY" - "$DATA" "$STUDENT" <<'PY'
import json,sys,re,random
from mlx_lm import load,generate
d,mp=sys.argv[1],sys.argv[2]; strip=lambda t:t.split('</think>')[-1]; model,tok=load(mp)
def ask(p,mx=200):
    txt=tok.apply_chat_template([{"role":"user","content":p+" /no_think"}],add_generation_prompt=True,tokenize=False)
    return generate(model,tok,prompt=txt,max_tokens=mx,verbose=False)
def opg(sym,fn,rule):
    tr=[(a,b) for a in range(1,9) for b in range(1,9)]; te=[(9,3),(7,9),(9,9),(2,9),(9,6),(4,9),(9,1),(8,9)]
    cold=lambda i:f"What is {i[0]} {sym} {i[1]}? Show your working, then give the integer."
    taught=lambda i:f"The operator {sym} is defined by a {sym} b = {rule}. {cold(i)}"
    return (tr,te,cold,lambda i:taught(i),lambda i:str(fn(*i)),lambda r,i:(lambda xs:int(xs[-1]) if xs else None)(re.findall(r'-?\d+',strip(r)))==fn(*i))
def cig():
    trw=["cat","dog","sun","map","red","big","top","cup","hat","pen","log","bus","fan","net","pig","rug","box","jam","kid","mud","nap","owl","rat","tub","van","web","yak","zip","arm","ear","ice","oak","elf","ink","egg","ant","urn","ash"]
    tew=["fox","bug","gem","hop","jet","lip","nut","pit"]; tr=lambda w:'-'.join(w.lower()); nw=lambda s:''.join(c for c in s.lower() if not c.isspace())
    cold=lambda w:f'Apply the ⊙ transform to the word "{w}". Output only the resulting word.'
    return (trw,tew,cold,lambda w:f'The ⊙ transform inserts a hyphen between adjacent letters (cat -> c-a-t). {cold(w)}',lambda w:tr(w),lambda r,w:tr(w) in nw(strip(r)))
skills={"A_add":opg("⊕",lambda a,b:3*a+2*b,"3*a + 2*b"),"B_dash":cig(),"C_mul":opg("⊗",lambda a,b:2*a+3*b,"2*a + 3*b")}
for name,(tr,te,cold,taught,truth,ok) in skills.items():
    rows=[]; g=0
    for i in tr:
        r=ask(taught(i))
        if ok(r,i): g+=1; rows.append({"prompt":cold(i),"completion":" "+r.strip()})
    open(f"{d}/train_{name}.jsonl","w").write("\n".join(json.dumps(x) for x in rows)+"\n")
    open(f"{d}/test_{name}.jsonl","w").write("\n".join(json.dumps({"prompt":cold(i),"completion":" "+truth(i)}) for i in te)+"\n")
    print(f"  self-gen {name}: {g}/{len(tr)}")
print("per-skill traces written")
PY

cat > "$WORK/eval.py" <<'PY'
import json,sys
from mlx_lm import load,generate
mp=sys.argv[1];ad=sys.argv[2] if len(sys.argv)>2 and sys.argv[2]!="-" else None;data=sys.argv[3]
model,tok=load(mp,adapter_path=ad);p=t=0;nw=lambda s:"".join(c for c in s.lower() if not c.isspace())
for line in open(data):
    ex=json.loads(line);t+=1
    txt=tok.apply_chat_template([{"role":"user","content":ex["prompt"]+" /no_think"}],add_generation_prompt=True,tokenize=False)
    o=generate(model,tok,prompt=txt,max_tokens=300,verbose=False).split('</think>')[-1]
    if nw(ex["completion"]) in nw(o):p+=1
print(f"{p}/{t}")
PY
evalall(){ local ad="$1"; for s in A_add B_dash C_mul; do echo -n "    $s: "; "$PY" "$WORK/eval.py" "$STUDENT" "$ad" "$DATA/test_$s.jsonl"; done; }
# round r: train.jsonl = this skill's traces + REPLAY (sample of earlier skills), valid = a slice.
mkround(){ "$PY" - "$DATA" "$@" <<'PY'
import json,sys,random; d=sys.argv[1]; cur=sys.argv[2]; prior=sys.argv[3:]
random.seed(1); rows=[json.loads(l) for l in open(f"{d}/train_{cur}.jsonl")]
rep=[]
for pj in prior:
    pr=[json.loads(l) for l in open(f"{d}/train_{pj}.jsonl")]; random.shuffle(pr); rep+=pr[:10]   # replay 10 each
allr=rows+rep; random.shuffle(allr)
open(f"{d}/train.jsonl","w").write("\n".join(json.dumps(x) for x in allr)+"\n")
open(f"{d}/valid.jsonl","w").write("\n".join(json.dumps(x) for x in allr[:12])+"\n")
print(f"  round({cur}) train={len(allr)} (={len(rows)} new + {len(rep)} replay)")
PY
}
lora(){ local resume="$1" out="$2"; local r=""; [ "$resume" != "-" ] && r="--resume-adapter-file $resume/adapters.safetensors"
  "$PY" -m mlx_lm.lora --model "$STUDENT" --train --data "$DATA" --batch-size $BATCH --num-layers $LAYERS --iters $ITERS --learning-rate $LR $r --adapter-path "$out" 2>&1 | tail -1; }

echo "=== COLD (all three novel) ==="; evalall -
echo "=== ROUND 1: learn A_add ==="; mkround A_add; lora - "$WORK/ad1"; echo "  eval-all after R1:"; evalall "$WORK/ad1"
echo "=== ROUND 2: learn B_dash (+replay A) ==="; mkround B_dash A_add; lora "$WORK/ad1" "$WORK/ad2"; echo "  eval-all after R2:"; evalall "$WORK/ad2"
echo "=== ROUND 3: learn C_mul (+replay A,B) ==="; mkround C_mul A_add B_dash; lora "$WORK/ad2" "$WORK/ad3"; echo "  eval-all after R3:"; evalall "$WORK/ad3"
echo "=== continual ✓ iff after R3 ALL of A_add/B_dash/C_mul are high (each learned over time, none forgotten) ==="
