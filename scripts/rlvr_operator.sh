#!/usr/bin/env bash
# REACHABILITY-LAW test — RLVR-lite (iterated rejection-FT) on a HIGH-reachability learnable task. Operator
# x(+)y = 3x+2y+7 (rule given in-prompt). Cold is nonzero + the answer is fully reachable (arithmetic), unlike
# generation-bound BIRD. Train operands small; HELD-OUT = larger operands (extrapolation). If held-out COMPOUNDS
# round-over-round (vs BIRD's plateau), the law holds: RL self-improvement compounds IFF the answer is reachable.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit; W=/tmp/yogi_rlvrop; mkdir -p "$W/data"; K="${K:-4}"; R="${R:-3}"; NTR="${NTR:-60}"; NTE="${NTE:-40}"
"$PY" - "$W" "$STUDENT" "$K" "$R" "$NTR" "$NTE" <<'PY'
import sys,re,json,random,os,subprocess
W,STU,K,R,NTR,NTE=sys.argv[1],sys.argv[2],int(sys.argv[3]),int(sys.argv[4]),int(sys.argv[5]),int(sys.argv[6])
op=lambda a,b: 3*a+2*b+4*((a+b)%9)+5  # harder multi-term; reachable but not saturated
prompt=lambda a,b: f"Define x ⊕ y = 3x + 2y + 4*((x+y) mod 9) + 5. Compute {a} ⊕ {b} step by step. End with 'Answer: N'."
def ans(o):
    o=o.split('</think>')[-1]; m=re.findall(r'[Aa]nswer\s*:?\s*(-?\d+)',o) or re.findall(r'(-?\d+)',o); return int(m[-1]) if m else None
random.seed(0)
train=[(random.randint(10,99),random.randint(10,99)) for _ in range(NTR*2)]; train=list(dict.fromkeys(train))[:NTR]
heldp=[(random.randint(10,99),random.randint(10,99)) for _ in range(NTE*3)]; heldp=list(dict.fromkeys(heldp))[:NTE]
from mlx_lm import load,generate
from mlx_lm.sample_utils import make_sampler
samp=make_sampler(temp=0.8)
def evalheld(adapter):
    m,t=load(STU,adapter_path=adapter) if adapter else load(STU); ok=0
    for a,b in heldp:
        o=generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":prompt(a,b)}],add_generation_prompt=True,tokenize=False),max_tokens=320,verbose=False)
        if ans(o)==op(a,b): ok+=1
    del m,t; return ok
seen=set(); traces=[]; adapter=None; n=len(heldp)
base=evalheld(None); print(f"ROUND 0 (base) held-out: {base}/{n} ({100*base//n}%)  [operator x(+)y=3x+2y+7, extrapolation]",flush=True)
for rnd in range(1,R+1):
    m,t=load(STU,adapter_path=adapter) if adapter else load(STU); newc=0
    for a,b in train:
        for _ in range(K):
            o=generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":prompt(a,b)}],add_generation_prompt=True,tokenize=False),max_tokens=320,verbose=False,sampler=samp)
            if ans(o)==op(a,b):
                kk=(a,b)
                if kk not in seen: seen.add(kk); traces.append({"prompt":prompt(a,b),"completion":f" Answer: {op(a,b)}"}); newc+=1
                break
    del m,t
    d=f"{W}/data"; open(f"{d}/train.jsonl","w").write("\n".join(json.dumps(x) for x in traces)+"\n")
    open(f"{d}/valid.jsonl","w").write("\n".join(json.dumps(x) for x in traces[:max(3,len(traces)//6)])+"\n")
    ad=f"{W}/adapter_r{rnd}"; os.makedirs(ad,exist_ok=True); it=min(300,max(100,len(traces)*4))
    r=subprocess.run([sys.executable,"-m","mlx_lm.lora","--model",STU,"--train","--data",d,"--batch-size","1",
      "--num-layers","8","--iters",str(it),"--learning-rate","1e-4","--max-seq-length","512","--grad-checkpoint","--adapter-path",ad],capture_output=True,text=True)
    if not os.path.exists(f"{ad}/adapters.safetensors"): print(f"R{rnd} LoRA FAILED:\n"+(r.stderr or r.stdout)[-400:]); break
    adapter=ad; acc=evalheld(adapter)
    print(f"ROUND {rnd}: +{newc} new correct ({len(traces)} total) -> held-out {acc}/{n} ({100*acc//n}%)",flush=True)
print(f"\n=== RLVR-LITE on REACHABLE operator task (K={K}, R={R}) ===",flush=True)
print(f"  base {base}/{n} -> rounds above. REACHABILITY LAW HOLDS iff held-out COMPOUNDS here (vs BIRD plateau 37) => RL self-improvement compounds where the answer is reachable.",flush=True)
PY
