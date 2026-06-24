#!/usr/bin/env bash
# ASCII half of the reachable-headroom validation (clean, f-strings only). Expect reachable-headroom ~0
# (matching flat retry/decompose), vs SQL's +18..+22 — validating: scaffolding ROI = pass@k oracle - one-shot.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
.venv-mlx/bin/python - <<'PY'
import sys,re,random; sys.path.insert(0,"scripts")
from yogi_harness import Model
from mlx_lm import generate
from mlx_lm.sample_utils import make_sampler
m=Model(); K=8; N=40; samp=make_sampler(temp=0.8)
def g(prompt,sample=False):
    p=m.t.apply_chat_template([{"role":"user","content":prompt}],add_generation_prompt=True,tokenize=False)
    kw={"sampler":samp} if sample else {}
    return generate(m.m,m.t,prompt=p,max_tokens=300,verbose=False,**kw)
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
SH={"square":(square,lambda n:f"a solid square of size {n} using '#'"),
 "rtri":(rtri,lambda n:f"a left-aligned right triangle of height {n} using '*' (row i has i stars)"),
 "hollow":(hollow,lambda n:f"a hollow square of size {n} using '#' (border '#', interior spaces)"),
 "pyr":(pyr,lambda n:f"a centered pyramid of height {n} using '*' (row i has 2i-1 stars, left-padded)"),
 "downtri":(downtri,lambda n:f"an inverted left-aligned triangle of height {n} using '*' (first row {n} stars)"),
 "rect":(rect,lambda n:f"a solid rectangle of {n} rows by {n+2} columns using '#'")}
def P(sh,n): return f"Draw {SH[sh][1](n)}. Output ONLY the ASCII art inside a fenced ``` code block. /no_think"
def gold(sh,n): return norm(SH[sh][0](n))
def ext(o):
    o=o.split('</think>')[-1]; mm=re.findall(r"```(?:\w+)?\s*\n?(.*?)```",o,re.S); return norm(mm[-1] if mm else o)
seen=set(); held=[]; rng=random.Random(2)
while len(held)<N:
    k=(rng.choice(list(SH)),rng.randint(3,12))
    if k not in seen: seen.add(k); held.append(k)
o1=sum(ext(g(P(sh,n)))==gold(sh,n) for sh,n in held)
ok=0
for sh,n in held:
    gd=gold(sh,n)
    ok+= any(ext(g(P(sh,n),True))==gd for _ in range(K))
print(f"\nASCII  one-shot {o1}/{N} ({round(100*o1/N)}%)  pass@{K} {ok}/{N} ({round(100*ok/N)}%)  reachable-headroom {round(100*(ok-o1)/N):+d}",flush=True)
print("  vs SQL: one-shot 32, pass@8 ~52, reachable +18..+22 (lever +11). ASCII reachable ~0 => validates: scaffolding ROI = pass@k oracle - one-shot.",flush=True)
PY
