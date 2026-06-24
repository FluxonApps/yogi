#!/usr/bin/env bash
# RLVR-LITE (the recommended ceiling lever) = iterated rejection-sampling fine-tuning (STaR/ReST), the feasible
# RL-from-verifiable-rewards on 16GB. Each ROUND: sample K candidates per TRAIN item with the CURRENT model
# (on-policy), KEEP the execution-CORRECT ones (free verifier = reward), accumulate, SFT a fresh LoRA from BASE
# on all accumulated correct traces, eval HELD-OUT one-shot. Iterate. Q: does ITERATED on-policy reward-gated
# self-improvement lift held-out toward the ~55% oracle, where ONE-SHOT distillation was flat? Memory-safe LoRA.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit; BIRD=/tmp/yogi_bird/minidev/MINIDEV; W=/tmp/yogi_rlvr; mkdir -p "$W/data"
NTR="${NTR:-80}"; NTE="${NTE:-40}"; K="${K:-4}"; R="${R:-3}"
"$PY" - "$W" "$STUDENT" "$BIRD" "$NTR" "$NTE" "$K" "$R" <<'PY'
import sqlite3,sys,re,json,random,os,subprocess
W,STU,BIRD,NTR,NTE,K,R=sys.argv[1],sys.argv[2],sys.argv[3],int(sys.argv[4]),int(sys.argv[5]),int(sys.argv[6]),int(sys.argv[7])
DBDIR=f"{BIRD}/dev_databases"; dbpath=lambda db:f"{DBDIR}/{db}/{db}.sqlite"
def schema(db):
    c=sqlite3.connect(dbpath(db)); ts=c.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall(); o=[]
    for (t,) in ts: o.append(f"{t}({', '.join(r[1] for r in c.execute(f'PRAGMA table_info(\"{t}\")').fetchall())})")
    c.close(); return "\n".join(o)
def run(db,sql):
    if not re.match(r'(?is)^\s*select\b',sql.strip()) or re.search(r'(?i)\b(drop|delete|update|insert|attach|pragma|create|alter)\b',sql): return None
    try:
        c=sqlite3.connect(f"file:{dbpath(db)}?mode=ro",uri=True); c.execute("PRAGMA query_only=ON"); r=c.execute(sql).fetchall(); c.close(); return r
    except Exception: return None
key=lambda rows: None if rows is None else tuple(sorted(tuple(str(x) for x in r) for r in rows))
def exsql(t):
    t=t.split('</think>')[-1]; m=re.findall(r'```(?:sql)?\s*(.*?)```',t,re.S); s=(m[-1] if m else t).strip()
    m2=re.findall(r'(?is)(select\b.*?)(?:;|$)',s); return (m2[-1].strip() if m2 else s)
cold=lambda q:(f"SQLite schema:\n{schema(q['db_id'])}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}\nWrite ONE SQLite SELECT query. Output only the SQL. /no_think")
Q=json.load(open(f"{BIRD}/mini_dev_sqlite.json"))
cand=["toxicology","california_schools","debit_card_specializing","student_club","superhero","financial"]
use=[db for db in cand if len(schema(db))<2500]
items=[q for q in Q if q['db_id'] in use and q['difficulty'] in ('simple','moderate')]
random.seed(0); random.shuffle(items); test=items[:NTE]; train=items[80:80+NTR]
from mlx_lm import load,generate
from mlx_lm.sample_utils import make_sampler
samp=make_sampler(temp=0.8)
def evalheld(adapter):
    m,t=load(STU,adapter_path=adapter) if adapter else load(STU); ok=0
    for q in test:
        g=key(run(q['db_id'],q['SQL']))
        if g is None: continue
        o=generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":cold(q)}],add_generation_prompt=True,tokenize=False),max_tokens=240,verbose=False)
        if key(run(q['db_id'],exsql(o)))==g: ok+=1
    del m,t; return ok
seen=set(); traces=[]; adapter=None
base=evalheld(None); n=len([q for q in test if key(run(q['db_id'],q['SQL'])) is not None])
print(f"ROUND 0 (base) held-out one-shot: {base}/{n} ({100*base//n}%)  [oracle ~55%, one-shot ~37%]",flush=True)
for rnd in range(1,R+1):
    # sample on-policy with current model, keep execution-correct
    m,t=load(STU,adapter_path=adapter) if adapter else load(STU); newc=0
    for q in train:
        g=key(run(q['db_id'],q['SQL']))
        if g is None: continue
        for _ in range(K):
            o=generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":cold(q)}],add_generation_prompt=True,tokenize=False),max_tokens=240,verbose=False,sampler=samp)
            sql=exsql(o)
            if key(run(q['db_id'],sql))==g:
                kk=(q['question'],sql)
                if kk not in seen: seen.add(kk); traces.append({"prompt":cold(q),"completion":" "+sql}); newc+=1
                break
    del m,t
    d=f"{W}/data"; open(f"{d}/train.jsonl","w").write("\n".join(json.dumps(x) for x in traces)+"\n")
    open(f"{d}/valid.jsonl","w").write("\n".join(json.dumps(x) for x in traces[:max(3,len(traces)//6)])+"\n")
    ad=f"{W}/adapter_r{rnd}"; os.makedirs(ad,exist_ok=True)
    it=min(400,max(120,len(traces)*3))
    r=subprocess.run([sys.executable,"-m","mlx_lm.lora","--model",STU,"--train","--data",d,"--batch-size","1",
      "--num-layers","8","--iters",str(it),"--learning-rate","1e-4","--max-seq-length","768","--grad-checkpoint","--adapter-path",ad],capture_output=True,text=True)
    if not os.path.exists(f"{ad}/adapters.safetensors"): print(f"R{rnd} LoRA FAILED:\n"+(r.stderr or r.stdout)[-400:]); break
    adapter=ad; acc=evalheld(adapter)
    print(f"ROUND {rnd}: +{newc} new correct traces ({len(traces)} total) -> held-out one-shot {acc}/{n} ({100*acc//n}%)",flush=True)
print(f"\n=== RLVR-LITE (iterated rejection-FT, BIRD, K={K}, R={R}) ===",flush=True)
print(f"  base {base}/{n} -> round-by-round above. COMPOUNDS iff held-out rises round-over-round toward ~55% oracle (RL beats flat one-shot distillation); FLAT iff iterated RL-lite cant move the generation-bound ceiling (needs full GRPO/scale).",flush=True)
PY
