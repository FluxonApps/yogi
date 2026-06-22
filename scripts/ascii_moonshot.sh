#!/usr/bin/env bash
# ASCII MOONSHOT (paper F6): a model that can't draw ASCII directly learns to — by CHANGING THE ACTION
# SPACE to shape-program composition + TEACHER-BOOTSTRAPPING the program-emission skill it lacks cold.
# Grounded: Program-aided Distillation (arXiv:2305.13888) + executable-code-actions. Teacher (Claude,
# metered salary) writes DSL programs → deterministic renderer+validity filter → distill qwen3-8b to
# EMIT programs → does the distilled model emit VALID programs (that render composed art) where the cold
# model emits empty/garbage? That is crossing the bootstrap floor via action-space change.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
VENV=.venv-mlx; PY=$VENV/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit
W=/tmp/yogi_moonshot; DATA=$W/data; ADAPTER=$W/adapter; mkdir -p "$DATA"
TEACHER_CAP="${TEACHER_CAP:-24}"   # hard cap on claude -p teacher calls (bounds salary)

# 1. TEACHER CORPUS: Claude writes shape-DSL programs; render + validity-filter; keep valid subject→program.
"$PY" - "$DATA" "$TEACHER_CAP" <<'PY'
import json,sys,subprocess,re
d=sys.argv[1]; cap=int(sys.argv[2])
W,H=18,11
def run_program(prog):
    g=[[' ']*W for _ in range(H)]
    def put(x,y,c):
        if 0<=x<W and 0<=y<H: g[y][x]=c
    def pi(s):
        try: return int(s)
        except: return 0
    macros={}; cur=None; top=[]
    for ln in prog.splitlines():
        t=ln.split()
        if not t: continue
        if t[0]=="def": cur=(t[1] if len(t)>1 else "_",[])
        elif t[0]=="end" and cur is not None: macros[cur[0]]=cur[1]; cur=None
        elif cur is not None: cur[1].append(ln)
        else: top.append(ln)
    def ex(t,ox,oy):
        try:
            if t[0]=="put" and len(t)>=4: put(ox+pi(t[1]),oy+pi(t[2]),t[3][0])
            elif t[0]=="hline" and len(t)>=5:
                for i in range(max(0,pi(t[3]))): put(ox+pi(t[1])+i,oy+pi(t[2]),t[4][0])
            elif t[0]=="vline" and len(t)>=5:
                for i in range(max(0,pi(t[3]))): put(ox+pi(t[1]),oy+pi(t[2])+i,t[4][0])
            elif t[0]=="rect" and len(t)>=6:
                x,y,ww,hh,c=pi(t[1]),pi(t[2]),pi(t[3]),pi(t[4]),t[5][0]
                for i in range(max(0,ww)): put(x+i,y,c); put(x+i,y+hh-1,c)
                for i in range(max(0,hh)): put(x,y+i,c); put(x+ww-1,y+i,c)
        except: pass
    for ln in top:
        t=ln.split()
        if t and t[0]=="call" and len(t)>=4 and t[1] in macros:
            for op in macros[t[1]]: ex(op.split(),pi(t[2]),pi(t[3]))
        elif t: ex(t,0,0)
    rows=[''.join(r).rstrip() for r in g]
    while rows and not rows[-1]: rows.pop()
    return '\n'.join(rows)
def valid(art):
    lines=[l for l in art.split('\n') if l.strip()]
    chars=set(c for l in lines for c in l if not c.isspace())
    return len(lines)>=3 and len(chars)>=2
def dsl_prompt(s):
    return (f"Draw a {s} on an {W}x{H} grid using ONLY these commands, one per line: "
            f"rect X Y W H C (rectangle outline), hline X Y LEN C, vline X Y LEN C, put X Y C. "
            f"Output ONLY the command lines, no commentary.")
def claude(s):
    p=dsl_prompt(s)+" Use coordinates inside the grid. Make it recognizable."
    try:
        o=subprocess.run(["claude","-p",p],capture_output=True,text=True,timeout=120).stdout
    except Exception: return ""
    o=re.sub(r'(?s)```[a-z]*\n?|```','',o)  # strip fences
    return "\n".join(l for l in o.splitlines() if l.split()[:1] and l.split()[0] in ("rect","hline","vline","put","def","end","call"))
train_subj=["house","tree","cat","dog","fish","boat","car","star","heart","flower","sun","key","cup",
            "robot","face","mountain","arrow","bird","snake","ladder","table","box","flag","clock"][:cap]
test_subj=["rocket","umbrella","bridge","lamp","kite","drum"]
rows=[]; kept=0
for s in train_subj:
    prog=claude(s); art=run_program(prog)
    if prog and valid(art):
        kept+=1; rows.append({"prompt":dsl_prompt(s),"completion":" "+prog.strip()})
        print(f"  teacher {s}: VALID ({len(art.splitlines())} lines)")
    else:
        print(f"  teacher {s}: rejected")
import random; random.seed(7); random.shuffle(rows)
open(f"{d}/train.jsonl","w").write("\n".join(json.dumps(r) for r in rows)+"\n")
open(f"{d}/valid.jsonl","w").write("\n".join(json.dumps(r) for r in rows[:6])+"\n")
open(f"{d}/test_subjects.txt","w").write("\n".join(test_subj)+"\n")
print(f"TEACHER CORPUS: {kept}/{len(train_subj)} valid programs (salary={len(train_subj)} claude calls)")
PY

# 2. DISTILL qwen3-8b to EMIT programs (teacher-bootstrap the emission skill).
echo "=== LoRA: teacher-bootstrap program emission ==="
"$PY" -m mlx_lm.lora --model "$STUDENT" --train --data "$DATA" --batch-size 2 --num-layers 16 \
  --iters 300 --learning-rate 1e-4 --adapter-path "$ADAPTER" 2>&1 | tail -1

# 3. EVAL: cold vs distilled — VALID-PROGRAM emission rate on HELD-OUT subjects (the floor-crossing).
cat > "$W/eval.py" <<'PY'
import json,sys
from mlx_lm import load,generate
W,H=18,11
mp=sys.argv[1]; ad=sys.argv[2] if len(sys.argv)>2 and sys.argv[2]!="-" else None
exec(open(sys.argv[3]).read())  # run_program + valid (shared)
model,tok=load(mp,adapter_path=ad)
def dsl_prompt(s):
    return (f"Draw a {s} on an {W}x{H} grid using ONLY these commands, one per line: "
            f"rect X Y W H C (rectangle outline), hline X Y LEN C, vline X Y LEN C, put X Y C. "
            f"Output ONLY the command lines, no commentary. /no_think")
subj=open(sys.argv[4]).read().split()
ok=0
for s in subj:
    txt=tok.apply_chat_template([{"role":"user","content":dsl_prompt(s)}],add_generation_prompt=True,tokenize=False)
    out=generate(model,tok,prompt=txt,max_tokens=300,verbose=False).split('</think>')[-1]
    prog="\n".join(l for l in out.splitlines() if l.split()[:1] and l.split()[0] in ("rect","hline","vline","put","def","end","call"))
    art=run_program(prog) if prog else ""
    v=bool(prog) and valid(art)
    ok+=v; print(f"  {s}: {'VALID' if v else 'empty/invalid'} ({len(prog.splitlines())} cmds)")
print(f"VALID-PROGRAM RATE: {ok}/{len(subj)}")
PY
# share run_program+valid with eval via a helper file
"$PY" - "$W" <<'PY'
import sys; W=sys.argv[1]
src='''W,H=18,11
def run_program(prog):
    g=[[" "]*W for _ in range(H)]
    def put(x,y,c):
        if 0<=x<W and 0<=y<H: g[y][x]=c
    def pi(s):
        try: return int(s)
        except: return 0
    macros={}; cur=None; top=[]
    for ln in prog.splitlines():
        t=ln.split()
        if not t: continue
        if t[0]=="def": cur=(t[1] if len(t)>1 else "_",[])
        elif t[0]=="end" and cur is not None: macros[cur[0]]=cur[1]; cur=None
        elif cur is not None: cur[1].append(ln)
        else: top.append(ln)
    def ex(t,ox,oy):
        try:
            if t[0]=="put" and len(t)>=4: put(ox+pi(t[1]),oy+pi(t[2]),t[3][0])
            elif t[0]=="hline" and len(t)>=5:
                for i in range(max(0,pi(t[3]))): put(ox+pi(t[1])+i,oy+pi(t[2]),t[4][0])
            elif t[0]=="vline" and len(t)>=5:
                for i in range(max(0,pi(t[3]))): put(ox+pi(t[1]),oy+pi(t[2])+i,t[4][0])
            elif t[0]=="rect" and len(t)>=6:
                x,y,ww,hh,c=pi(t[1]),pi(t[2]),pi(t[3]),pi(t[4]),t[5][0]
                for i in range(max(0,ww)): put(x+i,y,c); put(x+i,y+hh-1,c)
                for i in range(max(0,hh)): put(x,y+i,c); put(x+ww-1,y+i,c)
        except: pass
    for ln in top:
        t=ln.split()
        if t and t[0]=="call" and len(t)>=4 and t[1] in macros:
            for op in macros[t[1]]: ex(op.split(),pi(t[2]),pi(t[3]))
        elif t: ex(t,0,0)
    rows=["".join(r).rstrip() for r in g]
    while rows and not rows[-1]: rows.pop()
    return "\\n".join(rows)
def valid(art):
    lines=[l for l in art.split("\\n") if l.strip()]
    chars=set(c for l in lines for c in l if not c.isspace())
    return len(lines)>=3 and len(chars)>=2
'''
open(f"{W}/render.py","w").write(src)
PY
echo "=== COLD (no adapter) — valid-program emission on held-out ==="; "$PY" "$W/eval.py" "$STUDENT" - "$W/render.py" "$DATA/test_subjects.txt"
echo "=== DISTILLED — did it CROSS THE FLOOR? ==="; "$PY" "$W/eval.py" "$STUDENT" "$ADAPTER" "$W/render.py" "$DATA/test_subjects.txt"
echo "=== moonshot ✓ iff distilled emits VALID programs (composed art) where cold emits empty/garbage ==="
