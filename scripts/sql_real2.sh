#!/usr/bin/env bash
# BIRD usability — v2: FASTER (compact schema, validated 1.47x + ~5x shorter LoRA seqs; --max-seq-length)
# + STRONGER HONEST scaffold (sample-N: multiple verifier-gated attempts per question, NO answer leak) to
# lift yield where the naive repair scaffold starved (+2 → 14 traces, held-out flat). PHASE=probe checks if
# sample-N lifts scaffold-cross; PHASE=full distills + evals held-out before→after. Zero salary.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit
BIRD=/tmp/yogi_bird/minidev/MINIDEV; W=/tmp/yogi_sqlreal2; mkdir -p "$W/data"; PHASE="${PHASE:-probe}"; NSAMP="${NSAMP:-6}"
"$PY" - "$W" "$STUDENT" "$PHASE" "$BIRD" "$NSAMP" <<'PY'
import sqlite3,sys,re,os,json,subprocess,random
W,STU,PHASE,BIRD,NSAMP=sys.argv[1],sys.argv[2],sys.argv[3],sys.argv[4],int(sys.argv[5])
QJSON=f"{BIRD}/mini_dev_sqlite.json"; DBDIR=f"{BIRD}/dev_databases"
dbpath=lambda db:f"{DBDIR}/{db}/{db}.sqlite"
def compact_schema(db):  # table(col, col, ...) — 1.47x faster + ~5x shorter than full DDL
    c=sqlite3.connect(dbpath(db)); ts=c.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall(); out=[]
    for (t,) in ts:
        cols=[r[1] for r in c.execute(f"PRAGMA table_info('{t}')").fetchall()]; out.append(f"{t}({', '.join(cols)})")
    c.close(); return "\n".join(out)
Q=json.load(open(QJSON))
cand=["toxicology","california_schools","debit_card_specializing","student_club","superhero","financial"]
use=[db for db in cand if len(compact_schema(db))<2500]
items=[q for q in Q if q['db_id'] in use]; random.seed(0); random.shuffle(items)
train=items[:36]; test=items[36:54]
print(f"DBs={use}  train={len(train)} held-out={len(test)}  sample-N={NSAMP}  (compact schema)",flush=True)
def run_sql(db,sql):
    if not re.match(r'(?is)^\s*select\b', sql.strip()): return ("ERR","not select")
    if re.search(r'(?i)\b(drop|delete|update|insert|attach|pragma|create|alter)\b', sql): return ("ERR","forbidden")
    try:
        c=sqlite3.connect(dbpath(db)); c.execute("PRAGMA query_only=ON"); r=c.execute(sql).fetchall(); c.close(); return ("OK",r)
    except Exception as e: return ("ERR",str(e)[:100])
norm=lambda rows: sorted([tuple(str(x) for x in r) for r in rows])
def correct(db,c,g):
    s,cr=run_sql(db,c); s2,gr=run_sql(db,g); return s=="OK" and s2=="OK" and norm(cr)==norm(gr)
def extract(out):
    t=out.split('</think>')[-1]; m=re.search(r'```(?:sql)?\s*(.*?)```',t,re.S); sql=(m.group(1) if m else t).strip()
    m2=re.search(r'(?is)(select\b.*?)(;|$)',sql); return (m2.group(1).strip() if m2 else sql)
from mlx_lm import load,generate
try:
    from mlx_lm.sample_utils import make_sampler; SAMP=make_sampler(temp=0.8); HAVE_SAMP=True
except Exception as e: SAMP=None; HAVE_SAMP=False; print(f"(sampler unavailable: {e!r} — greedy only)",flush=True)
oneshot=lambda q:f"SQLite schema:\n{compact_schema(q['db_id'])}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}\nWrite ONE SQLite SELECT query. Output only the SQL. /no_think"
def gen(model,tok,p,mx=200,sample=False):
    kw={"sampler":SAMP} if (sample and HAVE_SAMP) else {}
    return generate(model,tok,prompt=tok.apply_chat_template([{"role":"user","content":p}],add_generation_prompt=True,tokenize=False),max_tokens=mx,verbose=False,**kw)
def sampleN(model,tok,q):  # honest yield lift: greedy + (N-1) sampled, keep first verifier-correct
    s0=extract(gen(model,tok,oneshot(q))); 
    if correct(q['db_id'],s0,q['SQL']): return s0,True
    for _ in range(NSAMP-1):
        s=extract(gen(model,tok,oneshot(q),sample=True))
        if correct(q['db_id'],s,q['SQL']): return s,True
    return s0,False
goldbad=sum(1 for q in train+test if run_sql(q['db_id'],q['SQL'])[0]!="OK")
print(f"gold exec check: {goldbad}/{len(train+test)} errored",flush=True)
model,tok=load(STU,adapter_path=None)
print(f"=== PROBE v2 (sample-{NSAMP}): one-shot floor vs scaffold-cross (train n={len(train)}) ===",flush=True)
os1=0; sc=0; rows=[]
for q in train:
    g0=extract(gen(model,tok,oneshot(q))); os1+=correct(q['db_id'],g0,q['SQL'])
    sql,ok=sampleN(model,tok,q); sc+=ok
    if ok: rows.append({"prompt":oneshot(q),"completion":" "+sql})
print(f"  one-shot FLOOR {os1}/{len(train)}   sample-{NSAMP} scaffold-cross {sc}/{len(train)}  -> {len(rows)} traces (was 14 naive)",flush=True)
tb=sum(correct(q['db_id'],extract(gen(model,tok,oneshot(q))),q['SQL']) for q in test)
print(f"  HELD-OUT one-shot before: {tb}/{len(test)}",flush=True)
if PHASE!="full":
    print("PROBE v2 done. if scaffold-cross >> 14 -> PHASE=full.",flush=True); sys.exit(0)
d=f"{W}/data"; open(f"{d}/train.jsonl","w").write("\n".join(json.dumps(r) for r in rows)+"\n")
open(f"{d}/valid.jsonl","w").write("\n".join(json.dumps(r) for r in rows[:max(2,len(rows)//5)])+"\n")
del model,tok; ad=f"{W}/adapter"; os.makedirs(ad,exist_ok=True)
subprocess.run([sys.executable,"-m","mlx_lm.lora","--model",STU,"--train","--data",d,"--batch-size","2",
  "--num-layers","16","--iters","200","--learning-rate","1e-4","--max-seq-length","1024","--adapter-path",ad],capture_output=True,text=True)
m1,t1=load(STU,adapter_path=ad)
ta=sum(correct(q['db_id'],extract(gen(m1,t1,oneshot(q))),q['SQL']) for q in test)
print(f"\n=== BIRD v2 RESULT (held-out one-shot exec-accuracy) ===",flush=True)
print(f"  before {tb}/{len(test)} -> after {ta}/{len(test)}   (stronger scaffold + faster harness)",flush=True)
print(f"  USABLE ✓ iff after >> before.",flush=True)
PY
