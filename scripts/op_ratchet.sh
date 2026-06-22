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
import json, sys, re, random
from mlx_lm import load, generate
d, mp = sys.argv[1], sys.argv[2]
op   = lambda a,b: 3*a + 2*b      # easy arithmetic — isolates rule-internalization from arithmetic
# A single COLD prompt (NO rule) used for train + eval; it invites reasoning so the model learns the
# PROCEDURE, not a lookup → generalizes to unseen operands.
cold = lambda a,b: f"What is {a} ⊕ {b}? Show your working step by step, then give the integer."
# self-gen uses the rule IN-CONTEXT to produce a correct REASONING trace; we distill that trace.
taught = lambda a,b: f"The operator ⊕ is defined by a ⊕ b = 3*a + 2*b. {cold(a,b)}"
parse = lambda t: (lambda xs: int(xs[-1]) if xs else None)(re.findall(r'-?\d+', t))
model, tok = load(mp)
def ask(p, mx=24):
    text = tok.apply_chat_template([{"role":"user","content":p}], add_generation_prompt=True, tokenize=False)
    return generate(model, tok, prompt=text, max_tokens=mx, verbose=False)
train = [(a,b) for a in range(1,9) for b in range(1,9)]      # 1..8 (disjoint from the 9-containing test)
test  = [(9,3),(7,9),(9,9),(2,9),(9,6),(4,9),(9,1),(8,9)]
rows, gen_ok = [], 0
for a,b in train:                                            # self-generate the REASONING (rule in-context)
    resp = ask(taught(a,b), mx=200)
    if parse(resp) == op(a,b):                               # free verifier keeps only correct traces
        gen_ok += 1
        # distill the model's OWN reasoning under the COLD prompt → it learns to apply 3a+2b itself.
        rows.append({"prompt": cold(a,b), "completion": " " + resp.strip()})
# balanced replay (M3 lesson: preserve adjacent skills so non-inferiority holds) — broadened.
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
open(f"{d}/test.jsonl","w").write("\n".join(json.dumps({"prompt":cold(a,b),"completion":f" {op(a,b)}"}) for a,b in test)+"\n")
open(f"{d}/general.jsonl","w").write("\n".join(json.dumps(x) for x in [
  {"prompt":"What is 2 + 2? Reply with only the number.","completion":" 4"},
  {"prompt":"What is the capital of France? One word.","completion":" Paris"},
  {"prompt":"How many days are in a week? Reply with only the number.","completion":" 7"}])+"\n")
print(f"SELF-GENERATED: {gen_ok}/{len(train)} train pairs solved with rule in-context -> {len(rows)} verified traces")
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
    if ex["completion"].strip() in generate(model,tok,prompt=text,max_tokens=160,verbose=False): p+=1
print(f"PASS {p}/{t}")
PY

echo "=== COLD baseline (held-out, NO rule) — expect ~0 on a novel rule ==="
echo -n "  cold: "; "$PY" "$WORK/eval.py" "$STUDENT" - "$DATA/test.jsonl"
echo "=== LoRA on the model's OWN self-generated verified traces ($STUDENT, layers=$LAYERS iters=$ITERS) ==="
"$PY" -m mlx_lm.lora --model "$STUDENT" --train --data "$DATA" \
  --batch-size "$BATCH" --num-layers "$LAYERS" --iters "$ITERS" --learning-rate "$LR" \
  --adapter-path "$ADAPTER" 2>&1 | tail -3
echo "=== HELD-OUT floor (cold prompt, no rule) — did the floor RISE? ==="
echo -n "  cold     : "; "$PY" "$WORK/eval.py" "$STUDENT" - "$DATA/test.jsonl"
echo -n "  distilled: "; "$PY" "$WORK/eval.py" "$STUDENT" "$ADAPTER" "$DATA/test.jsonl"
echo "=== general (forgetting / non-inferiority) ==="
echo -n "  cold     : "; "$PY" "$WORK/eval.py" "$STUDENT" - "$DATA/general.jsonl"
echo -n "  distilled: "; "$PY" "$WORK/eval.py" "$STUDENT" "$ADAPTER" "$DATA/general.jsonl"
echo "=== verdict: floor rose (P1 ✓) iff distilled held-out >> cold AND general non-inferior ==="
