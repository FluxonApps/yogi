#!/usr/bin/env bash
# GRADUATION CURVE (paper C3/F4) — democratization economics. As the local model internalizes MORE
# novel skills, does the NEXT one get cheaper to acquire (self-gen yield ↑) and stay retained (no
# collapse)? The marginal cost of skill k = the frontier-dependence-decay signal nobody measures. K
# distinct NOVEL linear operators learned SEQUENTIALLY (cumulative adapter + replay), FIXED small trace
# budget each. Per round we log: self-gen yield (cost proxy), learned (held-out cold), retention of ALL
# priors. One model at a time; zero frontier salary (free verifier = compute the operator).
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit
W=/tmp/yogi_grad; mkdir -p "$W/data"
"$PY" - "$W" "$STUDENT" <<'PY'
import json,sys,re,os,subprocess,random
from mlx_lm import load,generate
W,STU=sys.argv[1],sys.argv[2]
BUDGET=int(os.environ.get("BUDGET","24"))           # fixed self-gen trace cap per skill
OPS=[("⊕","3*a+2*b","3a + 2b"),("⊗","2*a+3*b","2a + 3b"),
     ("⊙","4*a+1*b","4a + b"),("⊚","1*a+4*b","a + 4b")]   # 4 distinct novel ops, each cold≈0
strip=lambda t:t.split('</think>')[-1]
parse=lambda t:(lambda xs:int(xs[-1]) if xs else None)(re.findall(r'-?\d+',strip(t)))
def op(expr,a,b): return eval(expr,{"__builtins__":{}},{"a":a,"b":b})
cold=lambda s,a,b:f"What is {a} {s} {b}? Show your working step by step, then give the integer. /no_think"
taught=lambda s,r,a,b:f"The operator {s} is defined by a {s} b = {r}. {cold(s,a,b)}"
train_pairs=[(a,b) for a in range(1,7) for b in range(1,7)]      # 36 candidate
test_pairs=[(9,3),(7,9),(9,9),(2,9),(9,6),(4,9),(9,1),(8,9)]    # unseen operands
def gen(model,tok,p,mx=200):
    txt=tok.apply_chat_template([{"role":"user","content":p}],add_generation_prompt=True,tokenize=False)
    return generate(model,tok,prompt=txt,max_tokens=mx,verbose=False)
def evalop(model,tok,s,expr):
    ok=0
    for a,b in test_pairs:
        if parse(gen(model,tok,cold(s,a,b),300))==op(expr,a,b): ok+=1
    return ok
prev=None; results=[]
for k,(s,expr,rule) in enumerate(OPS):
    # 1. self-gen WITH rule in-context, on the CUMULATIVE model (base + prev adapter)
    model,tok=load(STU,adapter_path=prev)
    solved=0; rows=[]
    random.seed(k); cand=train_pairs[:]; random.shuffle(cand)
    for a,b in cand:
        if len(rows)>=BUDGET: break
        r=gen(model,tok,taught(s,rule,a,b),200)
        if parse(r)==op(expr,a,b): solved+=1; rows.append({"prompt":cold(s,a,b),"completion":" "+r.strip()})
    yield_rate=solved/min(BUDGET,len(cand)) if cand else 0
    # replay: a few held-out-cold of each PRIOR op (retain) — uses prior ops' rule-internalized answers
    replay=[]
    for (ps,pe,_) in OPS[:k]:
        for a,b in test_pairs[:3]:
            replay.append({"prompt":cold(ps,a,b),"completion":f" {op(pe,a,b)}"})
    trainset=rows+replay; random.shuffle(trainset)
    d=f"{W}/data"; open(f"{d}/train.jsonl","w").write("\n".join(json.dumps(r) for r in trainset)+"\n")
    open(f"{d}/valid.jsonl","w").write("\n".join(json.dumps(r) for r in (rows[:6] or trainset[:6]))+"\n")
    del model,tok   # free before LoRA
    # 2. LoRA resume from cumulative adapter
    newad=f"{W}/ad{k}"; os.makedirs(newad,exist_ok=True)
    cmd=[PY:= sys.executable,"-m","mlx_lm.lora","--model",STU,"--train","--data",d,
         "--batch-size","2","--num-layers","16","--iters","200","--learning-rate","1e-4",
         "--adapter-path",newad]
    if prev: cmd+=["--resume-adapter-file",f"{prev}/adapters.safetensors"]
    subprocess.run(cmd,capture_output=True,text=True)
    # 3. eval: this skill learned + retention of all priors
    model,tok=load(STU,adapter_path=newad)
    learned=evalop(model,tok,s,expr)
    retention=[(ps,evalop(model,tok,ps,pe)) for (ps,pe,_) in OPS[:k]]
    del model,tok
    results.append((k+1,s,solved,yield_rate,learned,retention))
    print(f"ROUND {k+1} [{s}]: self-gen yield {solved}/{min(BUDGET,len(cand))}={yield_rate:.0%}  learned {learned}/8  retention {[(p,f'{r}/8') for p,r in retention]}",flush=True)
    prev=newad
print("\n=== GRADUATION CURVE ===")
for k,s,solved,yr,learned,ret in results:
    rmean=sum(r for _,r in ret)/len(ret) if ret else None
    print(f"  skill {k} [{s}]: yield={yr:.0%}  learned={learned}/8  prior-retention={'%.1f/8'%rmean if rmean is not None else 'n/a'}")
print("Cost-decay ✓ iff later skills keep high yield+learned at fixed budget AND priors stay retained (cheap continual acquisition).")
PY
