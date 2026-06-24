#!/usr/bin/env bash
# Does the verifier-gated retry mechanism GENERALIZE to weak-base SPATIAL? OneShot vs retry@2 on generated ASCII
# shapes (deterministic render verifier, truncation-free). If retry lifts spatial like SQL, the mechanism is
# domain-general (not SQL-specific).
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
prompt=lambda sh,n: f"Draw {SH[sh][1](n)}. Output ONLY the ASCII art inside a ```\n...\n``` block. /no_think"
gold=lambda sh,n: SH[sh][0](n)
def ext(o):
    o=o.split('</think>')[-1]; m=re.findall(r"```(?:\w+)?\s*\n(.*?)```",o,re.S); return norm(m[-1] if m else o)
seen=set(); held=[]
for s in [(random.Random(2).choice(list(SH)),0)]*0: pass
rng=random.Random(2)
while len(held)<24:
    k=(rng.choice(list(SH)),rng.randint(3,12))
    if k not in seen: seen.add(k); held.append(k)
m=Model()
def gen(sh,n):
    o,cap=m.gen(prompt(sh,n),300); 
    if cap: o,_=m.gen(prompt(sh,n),600)
    return ext(o)
def oneshot():
    return sum(gen(sh,n)==norm(gold(sh,n)) for sh,n in held)
def retry2():
    ok=0
    for sh,n in held:
        g=norm(gold(sh,n)); p=gen(sh,n); r=0
        while p!=g and r<2:
            o,cap=m.gen(prompt(sh,n)+"\n\nYour previous drawing was INCORRECT. Try again, exactly.",300)
            if cap: o,_=m.gen(prompt(sh,n),600)
            p=ext(o); r+=1
        ok+= (p==g)
    return ok
n=len(held); a=oneshot(); b=retry2()
print(f"\n=== RETRY mechanism on SPATIAL (ASCII shapes, n={n}) ===",flush=True)
print(f"  one-shot {a}/{n} ({round(100*a/n)}%) -> retry@2 {b}/{n} ({round(100*b/n)}%)  delta {round(100*(b-a)/n):+d}",flush=True)
print("  Retry generalizes to spatial iff retry@2 > one-shot (verifier-gated resampling is domain-general).",flush=True)
PY
