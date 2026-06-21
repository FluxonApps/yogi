#!/usr/bin/env bash
# Yogi — M3 WEIGHT distillation (LoRA) on Apple Silicon via MLX. Foreground/manual only (loads + trains
# a model); NEVER run in the automated loop or hooks. This is the weight-distillation arm that the spec
# deferred (D-M3-4, retrieval-first) — built here so the option is real, not gated.
#
# Pipeline: build a held-out ⊕ dataset (teacher-verified labels) → LoRA-train a small student → eval
# cold vs distilled on HELD-OUT operands (capability, not lookup) + a general set (non-inferiority).
# The being_distill::PromotionGate then decides promotion from those pass-rates.
#
# Result (2026-06-21, Qwen2.5-0.5B-Instruct-4bit student, see docs/FINDINGS.md): naive LoRA OVERFITS
# the seen pairs (train loss 0.14) but does NOT generalize ⊕ to held-out operands (0/12) and CATASTROPHICALLY
# FORGETS general ability — so the gate correctly REJECTS it. The token-space route (rule-in-prompt,
# `distill_close` bin) PROMOTES. This empirically validates retrieval-first / weight-distillation-deferred.
#
# Usage:  scripts/distill_lora.sh                # full run
#         STUDENT=mlx-community/Qwen2.5-1.5B-Instruct-4bit scripts/distill_lora.sh
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

VENV="${VENV:-.venv-mlx}"            # python3.14 -m venv .venv-mlx ; pip install mlx mlx-lm
PY="$VENV/bin/python"
STUDENT="${STUDENT:-mlx-community/Qwen2.5-0.5B-Instruct-4bit}"
WORK="${WORK:-/tmp/yogi_lora}"
DATA="$WORK/data"; ADAPTER="$WORK/adapter"
ITERS="${ITERS:-300}"; LAYERS="${LAYERS:-16}"; LR="${LR:-2e-4}"; BATCH="${BATCH:-4}"

[ -x "$PY" ] || { echo "no venv at $VENV — run: python3.14 -m venv $VENV && $VENV/bin/pip install mlx mlx-lm"; exit 1; }
mkdir -p "$DATA"

# 1. Dataset: ⊕(a,b)=a*b+a+b over digits 2..9 (64 pairs), held-out test split (teacher-verified labels).
"$PY" - "$DATA" <<'PY'
import json, random, sys
random.seed(7); d=sys.argv[1]
pairs=[(a,b) for a in range(2,10) for b in range(2,10)]; random.shuffle(pairs)
ex=lambda a,b:{"prompt":f"What is {a} ⊕ {b}? Reply with only the number.","completion":f" {a*b+a+b}"}
sp={"train":pairs[:44],"valid":pairs[44:52],"test":pairs[52:]}
for n,rows in sp.items():
    open(f"{d}/{n}.jsonl","w").write("\n".join(json.dumps(ex(a,b)) for a,b in rows)+"\n")
open(f"{d}/general.jsonl","w").write("\n".join(json.dumps(x) for x in [
  {"prompt":"What is 2 + 2? Reply with only the number.","completion":" 4"},
  {"prompt":"What is 10 minus 7? Reply with only the number.","completion":" 3"},
  {"prompt":"What is the capital of France? One word.","completion":" Paris"},
  {"prompt":"What color is the sky on a clear day? One word.","completion":" blue"},
  {"prompt":"How many days are in a week? Reply with only the number.","completion":" 7"},
])+"\n")
print("dataset:", {k:len(v) for k,v in sp.items()})
PY

# 2. Eval helper (cold = no adapter; distilled = with adapter), substring match on held-out + general.
cat > "$WORK/eval.py" <<'PY'
import json, sys
from mlx_lm import load, generate
mp=sys.argv[1]; ad=sys.argv[2] if len(sys.argv)>2 and sys.argv[2]!="-" else None; data=sys.argv[3]
model,tok=load(mp,adapter_path=ad); p=t=0
for line in open(data):
    ex=json.loads(line); t+=1
    text=tok.apply_chat_template([{"role":"user","content":ex["prompt"]}],add_generation_prompt=True,tokenize=False)
    if ex["completion"].strip() in generate(model,tok,prompt=text,max_tokens=12,verbose=False): p+=1
print(f"PASS {p}/{t}")
PY

# 3. LoRA train the student on teacher-verified ⊕ labels.
echo "=== LoRA training ($STUDENT, layers=$LAYERS iters=$ITERS lr=$LR) ==="
"$PY" -m mlx_lm.lora --model "$STUDENT" --train --data "$DATA" \
  --batch-size "$BATCH" --num-layers "$LAYERS" --iters "$ITERS" --learning-rate "$LR" \
  --adapter-path "$ADAPTER" 2>&1 | tail -3

# 4. Eval cold vs distilled — held-out ⊕ (capability) + general (non-inferiority).
echo "=== held-out ⊕ (capability on UNSEEN operands) ==="
echo -n "  cold     : "; "$PY" "$WORK/eval.py" "$STUDENT" - "$DATA/test.jsonl"
echo -n "  distilled: "; "$PY" "$WORK/eval.py" "$STUDENT" "$ADAPTER" "$DATA/test.jsonl"
echo "=== general (non-inferiority / forgetting) ==="
echo -n "  cold     : "; "$PY" "$WORK/eval.py" "$STUDENT" - "$DATA/general.jsonl"
echo -n "  distilled: "; "$PY" "$WORK/eval.py" "$STUDENT" "$ADAPTER" "$DATA/general.jsonl"
echo "(Feed these pass-rates to being_distill::PromotionGate: promote iff gap closed AND non-inferior.)"
