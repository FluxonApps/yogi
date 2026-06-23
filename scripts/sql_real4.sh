#!/usr/bin/env bash
# BIRD usability v4 — REASONING-DISTILLATION AT SCALE (the literature's recipe; challenges my premature
# "heterogeneity wall"). My v3 failed BECAUSE it distilled final ANSWERS at tiny scale (23). SLM-SQL /
# CogniSQL / DIN-SQL generalize via CoT reasoning + scale + decomposition. v4: self-gen step-by-step CoT
# traces (schema-link → joins → filters → final SQL), keep execution-verified, ~5x more (100 train,
# sample-3), distill the REASONING (not the answer). Does held-out one-shot lift where v3 was flat? Zero
# salary (free execution verifier), one model at a time.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit
BIRD=/tmp/yogi_bird/minidev/MINIDEV; W=/tmp/yogi_sqlreal4; mkdir -p "$W/data"; NTR="${NTR:-100}"; NTE="${NTE:-30}"; NSAMP="${NSAMP:-3}"
"$PY" - "$W" "$STUDENT" "$BIRD" "$NTR" "$NTE" "$NSAMP" <<'PY'
import sqlite3,sys,re,os,json,subprocess,random
W,STU,BIRD,NTR,NTE,NSAMP=sys.argv[1],sys.argv[2],sys.argv[3],int(sys.argv[4]),int(sys.argv[5]),int(sys.argv[6])
QJSON=f"{BIRD}/mini_dev_sqlite.json"; DBDIR=f"{BIRD}/dev_databases"
dbpath=lambda db:f"{DBDIR}/{db}/{db}.sqlite"
def compact_schema(db):
    c=sqlite3.connect(dbpath(db)); ts=c.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall(); out=[]
    for (t,) in ts:
        cols=[r[1] for r in c.execute(f"PRAGMA table_info('{t}')").fetchall()]; out.append(f"{t}({', '.join(cols)})")
    c.close(); return "\n".join(out)
Q=json.load(open(QJSON))
cand=["toxicology","california_schools","debit_card_specializing","student_club","superhero","financial"]
use=[db for db in cand if len(compact_schema(db))<2500]
items=[q for q in Q if q['db_id'] in use and q['difficulty'] in ('simple','moderate')]
random.seed(0); random.shuffle(items); train=items[:NTR]; test=items[NTR:NTR+NTE]
print(f"v4 reasoning-distillation: {len(train)} train / {len(test)} held-out, sample-{NSAMP}, CoT completions",flush=True)
def run_sql(db,sql):
    if not re.match(r'(?is)^\s*select\b', sql.strip()): return ("ERR","not select")
    if re.search(r'(?i)\b(drop|delete|update|insert|attach|pragma|create|alter)\b', sql): return ("ERR","forbidden")
    try:
        c=sqlite3.connect(dbpath(db)); c.execute("PRAGMA query_only=ON"); r=c.execute(sql).fetchall(); c.close(); return ("OK",r)
    except Exception as e: return ("ERR",str(e)[:80])
norm=lambda rows: sorted([tuple(str(x) for x in r) for r in rows])
def correct(db,c,g):
    s,cr=run_sql(db,c); s2,gr=run_sql(db,g); return s=="OK" and s2=="OK" and norm(cr)==norm(gr)
def extract(out):  # final SQL from a CoT: prefer ```sql```, else last SELECT...
    t=out.split('</think>')[-1]; m=re.findall(r'```(?:sql)?\s*(.*?)```',t,re.S)
    sql=(m[-1] if m else t).strip(); m2=re.findall(r'(?is)(select\b.*?)(?:;|$)',sql); return (m2[-1].strip() if m2 else sql)
from mlx_lm import load,generate
try:
    from mlx_lm.sample_utils import make_sampler; SAMP=make_sampler(temp=0.7); HAVE=True
except Exception: SAMP=None; HAVE=False
# REASONING prompt (CoT) — schema-link, joins, filters, then final SQL. This is both the self-gen prompt
# and the inference prompt (no hidden rule; schema+hint are legit inputs). Distillation internalizes the
# REASONING SKILL (shared across questions) — the literature's key to generalization.
def cot(q):
    return (f"SQLite schema:\n{compact_schema(q['db_id'])}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}\n"
            f"Reason step by step: (1) tables & columns needed, (2) joins, (3) filters/grouping/aggregation. "
            f"Then give the final query as ```sql ... ```. /no_think")
def gen(model,tok,p,mx=300,sample=False):
    kw={"sampler":SAMP} if (sample and HAVE) else {}
    return generate(model,tok,prompt=tok.apply_chat_template([{"role":"user","content":p}],add_generation_prompt=True,tokenize=False),max_tokens=mx,verbose=False,**kw)
model,tok=load(STU,adapter_path=None)
# held-out BEFORE (one-shot CoT)
tb=sum(correct(q['db_id'],extract(gen(model,tok,cot(q))),q['SQL']) for q in test)
print(f"HELD-OUT one-shot before: {tb}/{len(test)}",flush=True)
# self-gen verified CoT traces (greedy + samples), distill the FULL reasoning+SQL
rows=[]; solved=0
for i,q in enumerate(train):
    got=None
    o=gen(model,tok,cot(q))
    if correct(q['db_id'],extract(o),q['SQL']): got=o
    else:
        for _ in range(NSAMP-1):
            o=gen(model,tok,cot(q),sample=True)
            if correct(q['db_id'],extract(o),q['SQL']): got=o; break
    if got is not None:
        solved+=1; rows.append({"prompt":cot(q),"completion":" "+got.split('</think>')[-1].strip()})
    if (i+1)%25==0: print(f"  self-gen {i+1}/{len(train)} -> {solved} verified CoT traces",flush=True)
print(f"SELF-GEN: {solved}/{len(train)} verified CoT traces (v3 had 23 answer-only)",flush=True)
d=f"{W}/data"; open(f"{d}/train.jsonl","w").write("\n".join(json.dumps(r) for r in rows)+"\n")
open(f"{d}/valid.jsonl","w").write("\n".join(json.dumps(r) for r in rows[:max(3,len(rows)//6)])+"\n")
del model,tok; ad=f"{W}/adapter"; os.makedirs(ad,exist_ok=True)
print("=== LoRA on verified CoT traces (max-seq-length 1536) ===",flush=True)
subprocess.run([sys.executable,"-m","mlx_lm.lora","--model",STU,"--train","--data",d,"--batch-size","2",
  "--num-layers","16","--iters","200","--learning-rate","1e-4","--max-seq-length","1536","--adapter-path",ad],capture_output=True,text=True)
m1,t1=load(STU,adapter_path=ad)
ta=sum(correct(q['db_id'],extract(gen(m1,t1,cot(q))),q['SQL']) for q in test)
print(f"\n=== BIRD v4 RESULT (reasoning-distillation at scale, held-out n={len(test)}) ===",flush=True)
print(f"  held-out one-shot before {tb}/{len(test)} -> after {ta}/{len(test)}   (v3 answer-only was flat 11/24→10/24)",flush=True)
print(f"  GENERALIZES ✓ iff after >> before — CoT+scale lifts where answer-only+tiny-scale did not.",flush=True)
PY
