#!/usr/bin/env bash
# NOVEL-APPROACH test at the forgetting gap (research→invent→test, not the obvious fix).
# Q: does DISAMBIGUATION / similarity-aware replay prevent the confusable-skill forgetting that UNIFORM
#    replay missed (A_add ⊕=3a+2b collapsed 7/8→1/8 when the confusable C_mul ⊗=2a+3b was learned)?
# Reuses /tmp/yogi_seq (A+B adapter ad2 + per-skill traces + eval.py). Learns C_mul 3 ways, resuming ad2:
#   uniform : C + 10 random A + 10 random B          (baseline = the original failure)
#   heavyA  : C + ALL A + 10 B                        (similarity-aware heavy replay of the confusable skill)
#   disambig: C + ALL A + 10 B + joint-contrast (⊕ vs ⊗) examples   (the novel mechanism)
# Eval A_add (retention) + C_mul (still learned) after each. Real qwen3:8b, zero salary. Foreground only.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit; SEQ=/tmp/yogi_seq; W=/tmp/yogi_disambig
[ -f "$SEQ/ad2/adapters.safetensors" ] || { echo "need /tmp/yogi_seq/ad2 — run scripts/sequential_rounds.sh first"; exit 1; }
mkdir -p "$W"; cp "$SEQ/eval.py" "$W/eval.py"
ev(){ "$PY" "$W/eval.py" "$STUDENT" "$1" "$SEQ/data/test_$2.jsonl"; }
echo "baseline (uniform replay, from the sequential run): A_add collapsed to 1/8 when C_mul was learned."
for cond in uniform heavyA disambig; do
  "$PY" - "$SEQ/data" "$W" "$cond" <<'PY'
import json,sys,random; seq,w,cond=sys.argv[1],sys.argv[2],sys.argv[3]; random.seed(1)
ld=lambda f:[json.loads(l) for l in open(f"{seq}/train_{f}.jsonl")]
C,A,B=ld("C_mul"),ld("A_add"),ld("B_dash"); random.shuffle(A); random.shuffle(B)
if cond=="uniform": rep=A[:10]+B[:10]
elif cond=="heavyA": rep=A+B[:10]
else:
    op=lambda a,b:3*a+2*b; ot=lambda a,b:2*a+3*b
    contrast=[{"prompt":f"What is {a} ⊕ {b} and what is {a} ⊗ {b}?",
               "completion":f" {a} ⊕ {b} = {op(a,b)} (3a+2b); {a} ⊗ {b} = {ot(a,b)} (2a+3b)"}
              for a,b in [(3,4),(5,2),(7,1),(2,8),(6,6),(4,9),(8,3),(1,7),(9,5),(2,2)]]
    rep=A+B[:10]+contrast
tr=C+rep; random.shuffle(tr)
open(f"{w}/train.jsonl","w").write("\n".join(json.dumps(x) for x in tr)+"\n")
open(f"{w}/valid.jsonl","w").write("\n".join(json.dumps(x) for x in tr[:12])+"\n")
print(f"  [{cond}] train={len(tr)} (C={len(C)} + replay={len(rep)})")
PY
  "$PY" -m mlx_lm.lora --model "$STUDENT" --train --data "$W" --batch-size 2 --num-layers 16 --iters 250 \
     --learning-rate 1e-4 --resume-adapter-file "$SEQ/ad2/adapters.safetensors" --adapter-path "$W/ad_$cond" 2>&1 | tail -1
  echo -n "  [$cond] A_add retention: "; ev "$W/ad_$cond" A_add; echo -n "  [$cond] C_mul learned:   "; ev "$W/ad_$cond" C_mul
done
echo "=== NOVEL ✓ iff heavyA/disambig keep A_add high (vs uniform's 1/8 collapse) AND C_mul stays learned ==="
