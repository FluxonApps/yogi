#!/usr/bin/env bash
# REACHABILITY-LAW SEAL on a REACHABLE-but-not-saturated, TRUNCATION-FREE task: ASCII shapes (cold ~50% in the
# cross-task sweep; output is short art, no verbose reasoning to truncate; deterministic exact-match verifier).
# RLVR-lite (iterated rejection-FT): sample K, keep exact-correct, SFT, eval held-out, R rounds. Generated
# instances (shapes x sizes), held-out = unseen instances. If held-out COMPOUNDS (vs BIRD plateau), the law
# holds: RL self-improvement compounds where the answer is reachable. Ties to F6. Memory-safe LoRA.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit; W=/tmp/yogi_rlvrascii; mkdir -p "$W/data"; K="${K:-4}"; R="${R:-3}"; NTR="${NTR:-36}"; NTE="${NTE:-18}"
"$PY" - "$W" "$STUDENT" "$K" "$R" "$NTR" "$NTE" <<'PY'
import sys,re,json,random,os,subprocess
W,STU,K,R,NTR,NTE=sys.argv[1],sys.argv[2],int(sys.argv[3]),int(sys.argv[4]),int(sys.argv[5]),int(sys.argv[6])
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
SH={
 "square": (lambda n: square(n), lambda n: f"a solid square of size {n} using '#'"),
 "rtri":   (lambda n: rtri(n),   lambda n: f"a left-aligned right triangle of height {n} using '*' (row i has i stars)"),
 "hollow": (lambda n: hollow(n), lambda n: f"a hollow square of size {n} using '#' (border '#', interior spaces)"),
 "pyr":    (lambda n: pyr(n),    lambda n: f"a centered pyramid of height {n} using '*' (row i has 2i-1 stars, left-padded with spaces)"),
 "downtri":(lambda n: downtri(n),lambda n: f"an inverted left-aligned triangle of height {n} using '*' (row i has n-i stars, first row has {n})"),
 "rect":   (lambda n: rect(n),   lambda n: f"a solid rectangle of {n} rows by {n+2} columns using '#'"),
}
def gen_inst(rng,k):
    out=[]
    while len(out)<k:
        sh=rng.choice(list(SH)); n=rng.randint(3,12); out.append((sh,n))
    return out
random.seed(0); allk=set()
train=[]; 
for s in gen_inst(random.Random(1),NTR*2):
    if s not in allk: allk.add(s); train.append(s)
    if len(train)>=NTR: break
held=[]
for s in gen_inst(random.Random(2),NTE*3):
    if s not in allk: allk.add(s); held.append(s)
    if len(held)>=NTE: break
prompt=lambda sh,n: f"Draw {SH[sh][1](n)}. Output ONLY the ASCII art inside a ```\n...\n``` block. No explanation. /no_think"
gold=lambda sh,n: SH[sh][0](n)
def ext(o):
    o=o.split('</think>')[-1]; m=re.findall(r"```(?:\w+)?\s*\n(.*?)```",o,re.S); return norm(m[-1] if m else o)
from mlx_lm import load,generate
from mlx_lm.sample_utils import make_sampler
samp=make_sampler(temp=0.8)
def evalheld(adapter):
    m,t=load(STU,adapter_path=adapter) if adapter else load(STU); ok=0
    for sh,n in held:
        o=generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":prompt(sh,n)}],add_generation_prompt=True,tokenize=False),max_tokens=300,verbose=False)
        if ext(o)==norm(gold(sh,n)): ok+=1
    del m,t; return ok
seen=set(); traces=[]; adapter=None; n=len(held)
base=evalheld(None); print(f"ROUND 0 (base) held-out: {base}/{n} ({100*base//n}%)  [ASCII shapes, reachable-not-saturated, truncation-free]",flush=True)
for rnd in range(1,R+1):
    m,t=load(STU,adapter_path=adapter) if adapter else load(STU); newc=0
    for sh,nn in train:
        for _ in range(K):
            o=generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":prompt(sh,nn)}],add_generation_prompt=True,tokenize=False),max_tokens=300,verbose=False,sampler=samp)
            if ext(o)==norm(gold(sh,nn)):
                kk=(sh,nn)
                if kk not in seen: seen.add(kk); traces.append({"prompt":prompt(sh,nn),"completion":" ```\n"+gold(sh,nn)+"\n```"}); newc+=1
                break
    del m,t
    d=f"{W}/data"; open(f"{d}/train.jsonl","w").write("\n".join(json.dumps(x) for x in traces)+"\n")
    open(f"{d}/valid.jsonl","w").write("\n".join(json.dumps(x) for x in traces[:max(3,len(traces)//6)])+"\n")
    ad=f"{W}/adapter_r{rnd}"; os.makedirs(ad,exist_ok=True); it=min(300,max(100,len(traces)*5))
    r=subprocess.run([sys.executable,"-m","mlx_lm.lora","--model",STU,"--train","--data",d,"--batch-size","1",
      "--num-layers","8","--iters",str(it),"--learning-rate","1e-4","--max-seq-length","512","--grad-checkpoint","--adapter-path",ad],capture_output=True,text=True)
    if not os.path.exists(f"{ad}/adapters.safetensors"): print(f"R{rnd} LoRA FAILED:\n"+(r.stderr or r.stdout)[-400:]); break
    adapter=ad; acc=evalheld(adapter)
    print(f"ROUND {rnd}: +{newc} new correct ({len(traces)} total) -> held-out {acc}/{n} ({100*acc//n}%)",flush=True)
print(f"\n=== RLVR-LITE on ASCII (reachable-not-saturated, K={K}, R={R}) ===",flush=True)
print(f"  base {base}/{n} -> rounds above. REACHABILITY LAW SEALED iff held-out COMPOUNDS (vs BIRD plateau 37) => RL compounds where reachable.",flush=True)
PY
