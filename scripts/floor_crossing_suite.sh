#!/usr/bin/env bash
# BET B GENERALIZED (F7-suite) — is autonomous floor-crossing + internalized TOOL-USE a GENERAL
# phenomenon, not multiplication-specific? 4 distinct below-floor tasks (8B fails one-shot; a program
# reformulation crosses; free verifier = a reference computation). Self-gen program-traces across all 4,
# ONE multi-task LoRA distilled under the PLAIN prompt, then per-task eval: does the model spontaneously
# reach for code under a plain prompt (internalized tool-use)? Positions vs RAG-internalization
# (arXiv:2510.01375: internalize the scaffold, drop the runtime dependency). Zero salary (free verifier).
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit
W=/tmp/yogi_suite; mkdir -p "$W/data"
"$PY" - "$W" "$STUDENT" <<'PY'
import json,sys,re,os,subprocess,random
from mlx_lm import load,generate
W,STU=sys.argv[1],sys.argv[2]
random.seed(0); NT=" /no_think"
strip=lambda t:t.split('</think>')[-1]
norm=lambda s:"".join(c for c in str(s).lower() if not c.isspace())
DENY=("import","open(","exec(","eval(","__","subprocess","os.","sys.","socket","shutil","Path(")
def safe_run(out):
    e=strip(out); m=re.search(r'```(?:python)?\n?(.*?)```',e,re.S); code=m.group(1) if m else e
    code=code.strip()
    if not code or any(d in code for d in DENY): return None
    try:
        r=subprocess.run([sys.executable,"-I","-c",code],capture_output=True,timeout=6,text=True)
        return r.stdout.strip() if r.returncode==0 else None
    except Exception: return None
# --- 4 below-floor tasks: gen instance, plain prompt, program prompt, free-verifier answer ---
def t_mult():
    a,b=random.randint(1000,9999),random.randint(1000,9999); return (a,b),f"What is {a} * {b}? Give only the integer.{NT}",f"Write Python that prints {a}*{b}. Output only code.{NT}",str(a*b)
def t_sort():
    L=[random.randint(100,999) for _ in range(15)]; return L,f"Sort this list ascending and give the result: {L}.{NT}",f"Write Python that prints the sorted (ascending) version of {L} as a Python list. Output only code.{NT}",str(sorted(L))
def t_count():
    import string as _s; s="".join(random.choice("abcde") for _ in range(120)); c=random.choice("abcde")
    return (s,c),f"In the string '{s}', how many times does '{c}' appear? Give only the integer.{NT}",f"Write Python that prints the count of '{c}' in '{s}'. Output only code.{NT}",str(s.count(c))
def t_base():
    n=random.randint(100000,999999); return n,f"Convert {n} to binary (no '0b' prefix). Give only the binary digits.{NT}",f"Write Python that prints bin({n})[2:]. Output only code.{NT}",bin(n)[2:]
TASKS=[("mult",t_mult),("sort",t_sort),("count",t_count),("base",t_base)]
def cold_gen(model,tok,p,mx=200):
    return generate(model,tok,prompt=tok.apply_chat_template([{"role":"user","content":p}],add_generation_prompt=True,tokenize=False),max_tokens=mx,verbose=False)
m0,t0=load(STU)
print("=== PROBE (cold): direct floor vs program crosses, per task (k=6) ===",flush=True)
sel={}
for name,gen in TASKS:
    insts=[gen() for _ in range(6)]
    d_ok=sum(norm(ans) in norm(strip(cold_gen(m0,t0,dp))) for _,dp,_,ans in insts)
    p_ok=sum((safe_run(cold_gen(m0,t0,pp))==ans) for _,_,pp,ans in insts)
    sel[name]=(d_ok,p_ok); print(f"  {name}: direct {d_ok}/6  program {p_ok}/6  -> {'BELOW-FLOOR, program crosses' if p_ok>d_ok else 'check'}",flush=True)
# --- self-gen program traces across all tasks, distilled under the PLAIN prompt ---
rows=[]; pertask_traces={}
for name,gen in TASKS:
    got=0
    for _ in range(16):
        if got>=10: break
        inst,dp,pp,ans=gen(); out=cold_gen(m0,t0,pp)
        if safe_run(out)==ans:
            got+=1; rows.append({"prompt":dp,"completion":" "+strip(out).strip()})  # plain prompt -> code
    pertask_traces[name]=got; print(f"  self-gen {name}: {got} verified program-traces",flush=True)
random.shuffle(rows); d=f"{W}/data"
open(f"{d}/train.jsonl","w").write("\n".join(json.dumps(r) for r in rows)+"\n")
open(f"{d}/valid.jsonl","w").write("\n".join(json.dumps(r) for r in rows[:6])+"\n")
# held-out eval instances per task (fixed seed split)
random.seed(123); HELD={name:[gen() for _ in range(6)] for name,gen in TASKS}
# direct-before baseline (cold)
before={name:sum(norm(ans) in norm(strip(cold_gen(m0,t0,dp))) for _,dp,_,ans in HELD[name]) for name,_ in TASKS}
del m0,t0
ad=f"{W}/adapter"; os.makedirs(ad,exist_ok=True)
print("=== ONE multi-task LoRA over pooled program-traces ===",flush=True)
subprocess.run([sys.executable,"-m","mlx_lm.lora","--model",STU,"--train","--data",d,
  "--batch-size","2","--num-layers","16","--iters","250","--learning-rate","1e-4","--adapter-path",ad],capture_output=True,text=True)
m1,t1=load(STU,adapter_path=ad)
print("\n=== F7-SUITE RESULT (held-out n=6/task) — internalized tool-use under a PLAIN prompt ===",flush=True)
for name,_ in TASKS:
    H=HELD[name]
    # distilled: plain prompt -> does it spontaneously emit code that runs correct?
    tool=sum((safe_run(cold_gen(m1,t1,dp))==ans) for _,dp,_,ans in H)
    direct_after=sum(norm(ans) in norm(strip(cold_gen(m1,t1,dp))) for _,dp,_,ans in H)
    d_ok,p_ok=sel[name]
    print(f"  {name}: floor(direct cold) {d_ok}/6 | program-crosses {p_ok}/6 | DISTILLED plain->tool-use {tool}/6 (direct-answer {direct_after}/6) | before {before[name]}/6",flush=True)
print("\nGENERAL ✓ iff across tasks: direct cold low (floor) AND distilled plain-prompt tool-use high (internalized reaching-for-code, no scaffold instruction).",flush=True)
PY
