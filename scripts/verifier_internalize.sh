#!/usr/bin/env bash
# BREADTH FRONTIER — VERIFIER-INTERNALIZATION. Distill a LOCAL verifier (question+SQL -> correct? yes/no) from
# execution-labeled train pairs, then use it to SELECT among candidates at inference WITHOUT executing. If the
# learned verifier selects ~as well as execution (oracle), the ratchet extends BEYOND execution-verifiable
# domains (the productization unlock), and it may capture the 55% oracle better than typed-SC's 50%.
# Train labels: gold=correct(+), model one-shot attempt=incorrect(-) when it execution-fails. Memory-safe LoRA.
# Eval n=40: K temp candidates, select by verifier P(yes); compare to single / execution-oracle / self-consistency.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit; BIRD=/tmp/yogi_bird/minidev/MINIDEV; W=/tmp/yogi_verif; mkdir -p "$W/data"
NTR="${NTR:-60}"; NTE="${NTE:-40}"; K="${K:-4}"
# Phase A (labels) + Phase B (LoRA) + Phase C (eval) in one python (model loaded once for gen; LoRA via subprocess)
"$PY" - "$W" "$STUDENT" "$BIRD" "$NTR" "$NTE" "$K" <<'PY'
import sqlite3,sys,re,json,random,os,subprocess,collections
W,STU,BIRD,NTR,NTE,K=sys.argv[1],sys.argv[2],sys.argv[3],int(sys.argv[4]),int(sys.argv[5]),int(sys.argv[6])
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
Q=json.load(open(f"{BIRD}/mini_dev_sqlite.json"))
cand=["toxicology","california_schools","debit_card_specializing","student_club","superhero","financial"]
use=[db for db in cand if len(schema(db))<2500]
items=[q for q in Q if q['db_id'] in use and q['difficulty'] in ('simple','moderate')]
random.seed(0); random.shuffle(items); test=items[:NTE]; train=items[80:80+NTR]
from mlx_lm import load,generate
from mlx_lm.sample_utils import make_sampler
samp=make_sampler(temp=0.7)
m,t=load(STU)
def gen(p,mx=200,s=False):
    kw={"sampler":samp} if s else {}
    return generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":p}],add_generation_prompt=True,tokenize=False),max_tokens=mx,verbose=False,**kw)
def askp(q): return f"SQLite schema:\n{schema(q['db_id'])}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}\nWrite ONE SQLite SELECT. Output only the SQL. /no_think"
def vprompt(q,sql): return f"SQLite schema:\n{schema(q['db_id'])}\nQuestion: {q['question']}\nCandidate SQL: {sql}\nIs this SQL correct for the question? Answer yes or no. /no_think"
# Phase A: build verifier training labels (gold=yes; a wrong model attempt=no)
print("Phase A: labeling...",flush=True); rows=[]
for q in train:
    gk=key(run(q['db_id'],q['SQL']))
    if gk is None: continue
    rows.append({"prompt":vprompt(q,q['SQL']),"completion":" yes"})
    att=exsql(gen(askp(q)))
    if key(run(q['db_id'],att))!=gk: rows.append({"prompt":vprompt(q,att),"completion":" no"})
random.shuffle(rows); d=f"{W}/data"
open(f"{d}/train.jsonl","w").write("\n".join(json.dumps(r) for r in rows)+"\n")
open(f"{d}/valid.jsonl","w").write("\n".join(json.dumps(r) for r in rows[:max(3,len(rows)//6)])+"\n")
pos=sum(1 for r in rows if r["completion"]==" yes"); print(f"  {len(rows)} verifier labels ({pos} yes / {len(rows)-pos} no)",flush=True)
del m,t
# Phase B: LoRA-train the verifier (memory-safe)
print("Phase B: training verifier LoRA...",flush=True); ad=f"{W}/adapter"; os.makedirs(ad,exist_ok=True)
r=subprocess.run([sys.executable,"-m","mlx_lm.lora","--model",STU,"--train","--data",d,"--batch-size","1",
  "--num-layers","8","--iters","200","--learning-rate","1e-4","--max-seq-length","768","--grad-checkpoint","--adapter-path",ad],capture_output=True,text=True)
if not os.path.exists(f"{ad}/adapters.safetensors"): print("LoRA FAILED:\n"+(r.stderr or r.stdout)[-600:]); sys.exit(1)
# Phase C: eval — K temp candidates; select by learned verifier; compare to single/oracle/self-consistency
print("Phase C: eval with learned verifier as selector...",flush=True)
base_m,base_t=load(STU)              # generator (base)
vm,vt=load(STU,adapter_path=ad)      # verifier (adapter)
def vscore(q,sql):                   # P(correct) proxy: does verifier say yes?
    o=generate(vm,vt,prompt=vt.apply_chat_template([{"role":"user","content":vprompt(q,sql)}],add_generation_prompt=True,tokenize=False),max_tokens=8,verbose=False).split('</think>')[-1].lower()
    return 1 if "yes" in o else 0
def gcand(q,s):
    kw={"sampler":samp} if s else {}
    return exsql(generate(base_m,base_t,prompt=base_t.apply_chat_template([{"role":"user","content":askp(q)}],add_generation_prompt=True,tokenize=False),max_tokens=200,verbose=False,**kw))
single=0; oracle=0; sc=0; vsel=0
for i,q in enumerate(test):
    gk=key(run(q['db_id'],q['SQL']))
    if gk is None: continue
    cands=[gcand(q, j>0) for j in range(K)]
    keys=[key(run(q['db_id'],c)) for c in cands]
    if keys[0]==gk: single+=1
    if any(x==gk for x in keys): oracle+=1
    nn=[x for x in keys if x is not None]
    if nn and collections.Counter(nn).most_common(1)[0][0]==gk: sc+=1
    # verifier selection: pick first candidate the verifier approves (else candidate 0)
    pick=0
    for j,c in enumerate(cands):
        if vscore(q,c): pick=j; break
    if keys[pick]==gk: vsel+=1
    if (i+1)%10==0: print(f"  {i+1}/{len(test)}: single {single} oracle {oracle} sc {sc} verifier-select {vsel}",flush=True)
n=len([q for q in test if key(run(q['db_id'],q['SQL'])) is not None])
print(f"\n=== VERIFIER-INTERNALIZATION (learned verifier as gold-free selector, n={n}, K={K}) ===",flush=True)
print(f"  single {single}/{n} ({100*single//n}%)  execution-ORACLE {oracle}/{n} ({100*oracle//n}%)  self-consistency {sc}/{n} ({100*sc//n}%)  LEARNED-VERIFIER-select {vsel}/{n} ({100*vsel//n}%)",flush=True)
print(f"  VERIFIER-INTERNALIZATION WORKS iff learned-verifier-select >> single and approaches execution-oracle (selects WITHOUT executing -> extends the ratchet beyond execution-verifiable domains).",flush=True)
PY
