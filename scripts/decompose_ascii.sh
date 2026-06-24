#!/usr/bin/env bash
# Complement to the retry boundary: on SYSTEMATIC-error spatial (ASCII, where retry was flat), does a
# GENERATION-CHANGING lever (decompose: plan the rows, then draw) help? one-shot vs decompose, same instances.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
.venv-mlx/bin/python - <<'PY'
import sys,re,random; sys.path.insert(0,"scripts")
from yogi_harness import Model
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
basep=lambda sh,n: f"Draw {SH[sh][1](n)}. Output ONLY the ASCII art inside a ```\n...\n``` block. /no_think"
gold=lambda sh,n: SH[sh][0](n)
def ext(o):
    o=o.split('</think>')[-1]; m=re.findall(r"```(?:\w+)?\s*\n(.*?)```",o,re.S); return norm(m[-1] if m else o)
seen=set(); held=[]; rng=random.Random(2)
while len(held)<24:
    k=(rng.choice(list(SH)),rng.randint(3,12))
    if k not in seen: seen.add(k); held.append(k)
m=Model()
def g(p):
    o,cap=m.gen(p,300)
    if cap: o,_=m.gen(p,600)
    return ext(o)
def oneshot(): return sum(g(basep(sh,n))==norm(gold(sh,n)) for sh,n in held)
def decompose():
    ok=0
    for sh,n in held:
        plan,_=m.gen(f"You will draw {SH[sh][1](n)}. First state, for each row 1..{n} (or as specified), exactly which characters/spaces it contains. Be precise. Do NOT draw yet. /no_think",300)
        plan=plan.split('</think>')[-1].strip()[:700]
        p=basep(sh,n).replace("Draw","Using this row-by-row plan, draw")+f"\n\nPlan:\n{plan}"
        ok+= (g(p)==norm(gold(sh,n)))
    return ok
n=len(held); a=oneshot(); d=decompose()
print(f"\n=== GENERATION-CHANGING lever on SPATIAL (ASCII, n={n}) — where retry was FLAT ===",flush=True)
print(f"  one-shot {a}/{n} ({round(100*a/n)}%) -> decompose(row-plan) {d}/{n} ({round(100*d/n)}%)  delta {round(100*(d-a)/n):+d}",flush=True)
print("  Generation-changing helps systematic errors iff decompose > one-shot (retry was +0 here).",flush=True)
PY
