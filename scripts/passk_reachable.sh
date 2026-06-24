#!/usr/bin/env bash
# Validate the refined law: measure REACHABLE headroom (pass@k oracle - one-shot) directly on the contrasting
# pair — SQL (levers helped +11) vs ASCII spatial (levers flat +0). Predicts: SQL has spread (~18), ASCII ~0.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
.venv-mlx/bin/python - <<'PY'
import sys,re,random,sqlite3; sys.path.insert(0,"scripts")
from yogi_harness import Model
from yogi_tasks import BIRDTask
from mlx_lm.sample_utils import make_sampler
m=Model(); K=8; N=40
samp=make_sampler(temp=0.8)
def gen(prompt,mt,sample=False):
    from mlx_lm import generate
    p=m.t.apply_chat_template([{"role":"user","content":prompt}],add_generation_prompt=True,tokenize=False)
    kw={"sampler":samp} if sample else {}
    return generate(m.m,m.t,prompt=p,max_tokens=mt,verbose=False,**kw)
# --- SQL ---
bt=BIRDTask(); _,held=bt.split(0); sql=held[:N]
def sql_oneshot(ex): return bt.verify(bt.extract(gen(bt.context(ex)+"\n"+bt.instruction(),320)),ex)
def sql_passk(ex):
    for _ in range(K):
        if bt.verify(bt.extract(gen(bt.context(ex)+"\n"+bt.instruction(),320,True)),ex): return True
    return False
so=sum(sql_oneshot(e) for e in sql); sk=sum(sql_passk(e) for e in sql)
print(f"SQL/BIRD  one-shot {so}/{N} ({round(100*so/N)}%)  pass@{K} {sk}/{N} ({round(100*sk/N)}%)  reachable-headroom {round(100*(sk-so)/N):+d}",flush=True)
# --- ASCII ---
def norm(s):
    L=[ln.rstrip() for ln in s.replace("\r","").split("\n")]
    while L and L[0]=="": L.pop(0)
    while L and L[-1]=="": L.pop()
    return "\n".join(L)
def square(n): return "\n".join("#"*n for _ in range(n))
def rtri(n): return "\n".join("*"*i for i in range(1,n+1))
def hollow(n): return "\n".join("#"*n if r in (0,n-1) else "#"+" "*(n-2)+"#" for r in range(n))
def pyr(n): return "\n".join(" "*(n-i)+"*"*(2*i-1) for i in range(1,n+1))
def downtri(n): return "\n".join("*"*(n-i) for i in range(n))
def rect(n): return "\n".join("#"*(n+2) for _ in range(n))
SH={"square":(square,"a solid square of size {n} using '#'"),"rtri":(rtri,"a left-aligned right triangle of height {n} using '*' (row i has i stars)"),
 "hollow":(hollow,"a hollow square of size {n} using '#' (border '#', interior spaces)"),"pyr":(pyr,"a centered pyramid of height {n} using '*' (row i has 2i-1 stars, left-padded)"),
 "downtri":(downtri,"an inverted left-aligned triangle of height {n} using '*' (first row {n} stars)"),"rect":(rect,"a solid rectangle of {n} rows by {n+2} columns using '#'")}
P=lambda sh,n: f"Draw {SH[sh][1].format(n=n)}. Output ONLY the ASCII art inside a ```\\n...\\n``` block. /no_think"
gold=lambda sh,n: SH[sh][0](n)
def ext(o):
    o=o.split('</think>')[-1]; mm=re.findall(r"```(?:\w+)?\s*\n(.*?)```",o,re.S); return norm(mm[-1] if mm else o)
seen=set(); held2=[]; rng=random.Random(2)
while len(held2)<N:
    k=(rng.choice(list(SH)),rng.randint(3,12))
    if k not in seen: seen.add(k); held2.append(k)
def a_oneshot(sh,n): return ext(gen(P(sh,n),300))==norm(gold(sh,n))
def a_passk(sh,n):
    g=norm(gold(sh,n))
    for _ in range(K):
        if ext(gen(P(sh,n),300,True))==g: return True
    return False
ao=sum(a_oneshot(sh,n) for sh,n in held2); ak=sum(a_passk(sh,n) for sh,n in held2)
print(f"ASCII     one-shot {ao}/{N} ({round(100*ao/N)}%)  pass@{K} {ak}/{N} ({round(100*ak/N)}%)  reachable-headroom {round(100*(ak-ao)/N):+d}",flush=True)
print("\nVALIDATION: reachable-headroom (pass@k - one-shot) should be LARGE for SQL (levers +11) and ~0 for ASCII (levers +0).",flush=True)
PY
