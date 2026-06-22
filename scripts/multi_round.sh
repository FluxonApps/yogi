#!/usr/bin/env bash
# COMPOUNDING test (the real ratchet): does the model hold MULTIPLE internalized novel skills at once,
# and does the floor ratchet up CUMULATIVELY across a mixed curriculum — or do new skills erode old ones
# (catastrophic forgetting)? Round R trains ONE adapter on the UNION of all goals' self-generated
# verified traces so far (+ replay), then evals EVERY goal's held-out set. Real qwen3:8b, zero salary.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
VENV="${VENV:-.venv-mlx}"; PY="$VENV/bin/python"
STUDENT="${STUDENT:-mlx-community/Qwen3-8B-4bit}"
WORK="${WORK:-/tmp/yogi_multiround}"; DATA="$WORK/data"; ADAPTER="$WORK/adapter"
ITERS="${ITERS:-400}"; LAYERS="${LAYERS:-16}"; LR="${LR:-1e-4}"; BATCH="${BATCH:-2}"
mkdir -p "$DATA"

# 1. Self-generate verified traces for a MIXED curriculum (arithmetic + string), union them.
"$PY" - "$DATA" "$STUDENT" <<'PY'
import json, sys, re, random
from mlx_lm import load, generate
d, mp = sys.argv[1], sys.argv[2]
strip=lambda t:t.split('</think>')[-1]
model,tok=load(mp)
def ask(p,mx=200):
    text=tok.apply_chat_template([{"role":"user","content":p+" /no_think"}],add_generation_prompt=True,tokenize=False)
    return generate(model,tok,prompt=text,max_tokens=mx,verbose=False)
# Curriculum: three NOVEL skills, two kinds. Each: instances, cold/taught prompts, truth, verify.
def op_goal(sym, fn, rule):
    tr=[(a,b) for a in range(1,9) for b in range(1,9)]; te=[(9,3),(7,9),(9,9),(2,9),(9,6),(4,9),(9,1),(8,9)]
    cold=lambda i:f"What is {i[0]} {sym} {i[1]}? Show your working, then give the integer."
    taught=lambda i:f"The operator {sym} is defined by a {sym} b = {rule}. {cold(i)}"
    truth=lambda i:str(fn(*i)); ok=lambda r,i:(lambda xs:int(xs[-1]) if xs else None)(re.findall(r'-?\d+',strip(r)))==fn(*i)
    return ("⊕"+sym, tr, te, cold, taught, truth, ok)
def cipher_goal():
    trw=["cat","dog","sun","map","red","big","top","cup","hat","pen","log","bus","fan","net","pig","rug","box","jam","kid","mud","nap","owl","rat","tub","van","web","yak","zip","arm","ear","ice","oak","elf","ink","egg","ant","urn","ash"]
    tew=["fox","bug","gem","hop","jet","lip","nut","pit"]; tr=lambda w:'-'.join(w.lower())
    nows=lambda s:''.join(c for c in s.lower() if not c.isspace())
    cold=lambda w:f'Apply the ⊙ transform to the word "{w}". Output only the resulting word.'
    taught=lambda w:f'The ⊙ transform inserts a hyphen between adjacent letters (cat -> c-a-t). {cold(w)}'
    return ("⊙dash", trw, tew, cold, taught, (lambda w:tr(w)), (lambda r,w:tr(w) in nows(strip(r))))
goals=[op_goal("⊕", lambda a,b:3*a+2*b, "3*a + 2*b"),
       cipher_goal(),
       op_goal("⊗", lambda a,b:2*a+3*b, "2*a + 3*b")]
union=[]; replay=[{"prompt":"What is 3 + 5? Reply with only the number.","completion":" 8"},
       {"prompt":"What is the capital of Japan? One word.","completion":" Tokyo"},
       {"prompt":"What color is grass? One word.","completion":" green"}]
for name,tr,te,cold,taught,truth,ok in goals:
    g=0
    for i in tr:
        r=ask(taught(i))
        if ok(r,i): g+=1; union.append({"prompt":cold(i),"completion":" "+r.strip()})
    open(f"{d}/test_{name}.jsonl","w").write("\n".join(json.dumps({"prompt":cold(i),"completion":" "+truth(i)}) for i in te)+"\n")
    print(f"  self-gen {name}: {g}/{len(tr)}")
union=union+replay; random.seed(7); random.shuffle(union)
open(f"{d}/train.jsonl","w").write("\n".join(json.dumps(r) for r in union)+"\n")
open(f"{d}/valid.jsonl","w").write("\n".join(json.dumps(r) for r in union[:12])+"\n")
print(f"UNION train set: {len(union)} traces across 3 novel skills (2 kinds)")
PY

cat > "$WORK/eval.py" <<'PY'
import json,sys
from mlx_lm import load,generate
mp=sys.argv[1];ad=sys.argv[2] if len(sys.argv)>2 and sys.argv[2]!="-" else None;data=sys.argv[3]
model,tok=load(mp,adapter_path=ad);p=t=0
nows=lambda s:"".join(c for c in s.lower() if not c.isspace())
for line in open(data):
    ex=json.loads(line);t+=1
    txt=tok.apply_chat_template([{"role":"user","content":ex["prompt"]+" /no_think"}],add_generation_prompt=True,tokenize=False)
    out=generate(model,tok,prompt=txt,max_tokens=300,verbose=False).split('</think>')[-1]
    if nows(ex["completion"]) in nows(out):p+=1
print(f"{p}/{t}")
PY

echo "=== COLD (no adapter) — each skill held-out ==="
for f in "$DATA"/test_*.jsonl; do echo -n "  cold  $(basename $f .jsonl): "; "$PY" "$WORK/eval.py" "$STUDENT" - "$f"; done
echo "=== LoRA on the UNION of all 3 skills' self-gen traces ==="
"$PY" -m mlx_lm.lora --model "$STUDENT" --train --data "$DATA" --batch-size "$BATCH" --num-layers "$LAYERS" --iters "$ITERS" --learning-rate "$LR" --adapter-path "$ADAPTER" 2>&1 | tail -2
echo "=== DISTILLED — does ONE model hold ALL THREE skills at once? (compounding) ==="
for f in "$DATA"/test_*.jsonl; do echo -n "  distilled $(basename $f .jsonl): "; "$PY" "$WORK/eval.py" "$STUDENT" "$ADAPTER" "$f"; done
echo "=== compounding ✓ iff all three held-out sets rose (one model, multiple novel skills, no interference) ==="
