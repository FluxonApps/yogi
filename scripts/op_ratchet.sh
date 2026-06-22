#!/usr/bin/env bash
# Democratization ratchet — P1 make-or-break (docs/plan/democratization-roadmap.md).
# Question: does distilling the model's OWN verified successes raise its HELD-OUT floor on a NOVEL rule?
#
# vs M3 (scripts/distill_lora.sh): M3 HAND-LABELED the data (teacher). Here the model SELF-GENERATES the
# training data — it solves the train pairs with the rule IN-CONTEXT, a FREE verifier keeps only the
# correct ones, and we distill those as cold(no-rule)->answer. So a cold-floor rise = the model taught
# ITSELF a skill it could only do with help, and internalized it into weights. One model end-to-end
# (MLX, a weak 1.5B), ZERO frontier salary. Goal spec = being-goals::op (the Rust crate is the tested
# verifier; this script mirrors op = a*b+a+b). Foreground/manual only — never in the loop or hooks.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
VENV="${VENV:-.venv-mlx}"; PY="$VENV/bin/python"
STUDENT="${STUDENT:-mlx-community/Qwen2.5-1.5B-Instruct-4bit}"
WORK="${WORK:-/tmp/yogi_op_ratchet}"; DATA="$WORK/data"; ADAPTER="$WORK/adapter"
ITERS="${ITERS:-300}"; LAYERS="${LAYERS:-16}"; LR="${LR:-1e-4}"; BATCH="${BATCH:-4}"
[ -x "$PY" ] || { echo "no venv at $VENV — python3.14 -m venv $VENV && $VENV/bin/pip install mlx mlx-lm"; exit 1; }
mkdir -p "$DATA"

# 1. SELF-GENERATE verified traces: the model solves train pairs WITH the rule in-context; the free
#    verifier (compute a*b+a+b) keeps only correct ones, written as cold(no-rule)->answer. + balanced
#    replay (M3 lesson: preserve adjacent skills so non-inferiority holds).
"$PY" - "$DATA" "$STUDENT" <<'PY'
import json, sys, re, random, os
from mlx_lm import load, generate
d, mp = sys.argv[1], sys.argv[2]
NT = " /no_think" if os.environ.get("THINK_OFF") else ""   # qwen3 etc. are thinking models — disable <think>
KIND = os.environ.get("GOAL_KIND", "numeric")              # numeric (operator ⊕) | string (cipher ⊙)
strip_think = lambda t: t.split('</think>')[-1]            # ignore any <think> block before parsing
model, tok = load(mp)
def ask(p, mx=200):
    text = tok.apply_chat_template([{"role":"user","content":p}], add_generation_prompt=True, tokenize=False)
    return generate(model, tok, prompt=text, max_tokens=mx, verbose=False)
# Unified goal interface: instances + cold/taught prompts + truth string + a free verifier `ok`.
if KIND == "string":                                       # dash-insertion cipher ⊙ (mirrors being-goals::cipher)
    tr = lambda w: '-'.join(w.lower())                     # cat -> c-a-t (easy to apply → high self-gen yield)
    nows = lambda s: ''.join(c for c in s.lower() if not c.isspace())
    train_i = ["cat","dog","sun","map","red","big","top","cup","hat","pen","log","bus","fan","net","pig",
               "rug","box","jam","kid","mud","nap","owl","rat","tub","van","web","yak","zip","arm","ear",
               "ice","oak","elf","ink","egg","ant","urn","ash"]
    test_i  = ["fox","bug","gem","hop","jet","lip","nut","pit"]
    cold   = lambda w: f'Apply the ⊙ transform to the word "{w}". Output only the resulting word.{NT}'
    taught = lambda w: f'The ⊙ transform inserts a hyphen between every pair of adjacent letters (e.g. cat -> c-a-t). {cold(w)}'
    truth_str = lambda w: tr(w)
    ok = lambda resp, w: tr(w) in nows(strip_think(resp))
elif KIND == "roman":                                      # REAL recognizable task — int → Roman numeral
    def i2r(n):
        vals=[(1000,'M'),(900,'CM'),(500,'D'),(400,'CD'),(100,'C'),(90,'XC'),(50,'L'),(40,'XL'),(10,'X'),(9,'IX'),(5,'V'),(4,'IV'),(1,'I')]
        r=''
        for v,sym in vals:
            while n>=v: r+=sym; n-=v
        return r
    pool=list(range(1,4000)); random.seed(3); random.shuffle(pool)
    train_i=sorted(pool[:80]); test_i=sorted(pool[80:92])  # disjoint held-out — generalization, not lookup
    nows=lambda s:''.join(c for c in s.upper() if not c.isspace())
    cold=lambda n:f"Convert the number {n} to a Roman numeral. Output only the Roman numeral.{NT}"
    taught=lambda n:f"Roman numerals: I=1,V=5,X=10,L=50,C=100,D=500,M=1000; subtractive IV=4,IX=9,XL=40,XC=90,CD=400,CM=900. Convert {n} to a Roman numeral. Output only the Roman numeral.{NT}"
    truth_str=lambda n:i2r(n)
    ok=lambda resp,n:i2r(n) in nows(strip_think(resp))
else:                                                      # operator ⊕ = OP_EXPR (novel arithmetic rule)
    EXPR = os.environ.get("OP_EXPR", "3*a+2*b"); RULE = os.environ.get("RULE", "3*a + 2*b")
    op = lambda a,b: eval(EXPR, {"__builtins__": {}}, {"a": a, "b": b})
    train_i = [(a,b) for a in range(1,9) for b in range(1,9)]   # disjoint from the 9-containing test
    test_i  = ([(a,b) for a in range(9,13) for b in range(1,11)]  # n=40 unseen-operand held-out
               if os.environ.get("BIG_HELDOUT") else
               [(9,3),(7,9),(9,9),(2,9),(9,6),(4,9),(9,1),(8,9)])
    cold   = lambda i: f"What is {i[0]} ⊕ {i[1]}? Show your working step by step, then give the integer.{NT}"
    taught = lambda i: f"The operator ⊕ is defined by a ⊕ b = {RULE}. {cold(i)}"
    parse = lambda t: (lambda xs: int(xs[-1]) if xs else None)(re.findall(r'-?\d+', strip_think(t)))
    truth_str = lambda i: str(op(*i))
    ok = lambda resp, i: parse(resp) == op(*i)
rows, gen_ok = [], 0
for i in train_i:                                          # self-generate WITH the rule in-context
    resp = ask(taught(i), mx=200)
    if ok(resp, i):                                        # free verifier keeps only correct traces
        gen_ok += 1
        rows.append({"prompt": cold(i), "completion": " " + resp.strip()})  # distill the model's OWN solution
# balanced replay (M3 lesson: preserve adjacent skills so non-inferiority holds).
replay = [{"prompt":"What is 3 + 5? Reply with only the number.","completion":" 8"},
          {"prompt":"What is 12 - 7? Reply with only the number.","completion":" 5"},
          {"prompt":"What is 6 times 4? Reply with only the number.","completion":" 24"},
          {"prompt":"What is the capital of Japan? One word.","completion":" Tokyo"},
          {"prompt":"What is the capital of Italy? One word.","completion":" Rome"},
          {"prompt":"What color is grass? One word.","completion":" green"},
          {"prompt":"How many days are in a week? Reply with only the number.","completion":" 7"},
          {"prompt":"What color is the sky on a clear day? One word.","completion":" blue"}]
trainset = rows + replay; random.seed(7); random.shuffle(trainset)
open(f"{d}/train.jsonl","w").write("\n".join(json.dumps(r) for r in trainset)+"\n")
open(f"{d}/valid.jsonl","w").write("\n".join(json.dumps(r) for r in (rows[:8] or replay))+"\n")
open(f"{d}/test.jsonl","w").write("\n".join(json.dumps({"prompt":cold(i),"completion":" "+truth_str(i)}) for i in test_i)+"\n")
open(f"{d}/general.jsonl","w").write("\n".join(json.dumps(x) for x in [
  {"prompt":"What is 2 + 2? Reply with only the number.","completion":" 4"},
  {"prompt":"What is the capital of France? One word.","completion":" Paris"},
  {"prompt":"How many days are in a week? Reply with only the number.","completion":" 7"}])+"\n")
print(f"SELF-GENERATED: {gen_ok}/{len(train_i)} instances solved with rule in-context -> {len(rows)} verified traces")
PY

# 2. eval helper (cold = no adapter; distilled = with adapter), substring match.
cat > "$WORK/eval.py" <<'PY'
import json, sys
from mlx_lm import load, generate
mp=sys.argv[1]; ad=sys.argv[2] if len(sys.argv)>2 and sys.argv[2]!="-" else None; data=sys.argv[3]
model,tok=load(mp,adapter_path=ad); p=t=0
for line in open(data):
    ex=json.loads(line); t+=1
    text=tok.apply_chat_template([{"role":"user","content":ex["prompt"]}],add_generation_prompt=True,tokenize=False)
    import os
    out=generate(model,tok,prompt=text,max_tokens=int(os.environ.get("EVAL_MAX","300")),verbose=False).split('</think>')[-1]
    norm=lambda s:"".join(c for c in s.lower() if not c.isspace())
    if norm(ex["completion"]) in norm(out): p+=1
print(f"PASS {p}/{t}")
PY

echo "=== COLD baseline (held-out, NO rule) — expect ~0 on a novel rule ==="
echo -n "  cold: "; "$PY" "$WORK/eval.py" "$STUDENT" - "$DATA/test.jsonl"
echo "=== LoRA on the model's OWN self-generated verified traces ($STUDENT, layers=$LAYERS iters=$ITERS) ==="
"$PY" -m mlx_lm.lora --model "$STUDENT" --train --data "$DATA" \
  --batch-size "$BATCH" --num-layers "$LAYERS" --iters "$ITERS" --learning-rate "$LR" \
  --seed "${SEED:-0}" --adapter-path "$ADAPTER" 2>&1 | tail -3
echo "=== HELD-OUT floor (cold prompt, no rule) — did the floor RISE? ==="
echo -n "  cold     : "; "$PY" "$WORK/eval.py" "$STUDENT" - "$DATA/test.jsonl"
echo -n "  distilled: "; "$PY" "$WORK/eval.py" "$STUDENT" "$ADAPTER" "$DATA/test.jsonl"
echo "=== general (forgetting / non-inferiority) ==="
echo -n "  cold     : "; "$PY" "$WORK/eval.py" "$STUDENT" - "$DATA/general.jsonl"
echo -n "  distilled: "; "$PY" "$WORK/eval.py" "$STUDENT" "$ADAPTER" "$DATA/general.jsonl"
echo "=== verdict: floor rose (P1 ✓) iff distilled held-out >> cold AND general non-inferior ==="
