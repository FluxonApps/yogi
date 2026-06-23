#!/usr/bin/env bash
# TEXT-TO-SQL USABILITY PROOF on REAL BIRD mini-dev (the popular/recommended realistic benchmark).
# Free verifier = execute candidate vs gold on the REAL .sqlite (result-set compare). Small-schema DBs
# only (schema must fit the 8B context — avoid the Spider-2.0 starvation trap). PHASE=probe verifies the
# floor + scaffold-cross before any LoRA; PHASE=full runs the ratchet. Zero salary; one model at a time.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit
BIRD=/tmp/yogi_bird/minidev/MINIDEV; W=/tmp/yogi_sqlreal; mkdir -p "$W/data"; PHASE="${PHASE:-probe}"
"$PY" - "$W" "$STUDENT" "$PHASE" "$BIRD" <<'PY'
import sqlite3,sys,re,os,json,subprocess,random
W,STU,PHASE,BIRD=sys.argv[1:5]
QJSON=f"{BIRD}/mini_dev_sqlite.json"; DBDIR=f"{BIRD}/dev_databases"
dbpath=lambda db:f"{DBDIR}/{db}/{db}.sqlite"
def ddl(db):
    c=sqlite3.connect(dbpath(db)); rows=c.execute("SELECT sql FROM sqlite_master WHERE type='table' AND sql IS NOT NULL").fetchall(); c.close()
    return "\n".join(r[0] for r in rows)
Q=json.load(open(QJSON))
# small-schema DBs (DDL fits context). Pick by measuring DDL length.
cand=["toxicology","california_schools","debit_card_specializing","student_club","superhero","financial"]
MAXDDL=3500
use=[db for db in cand if len(ddl(db))<MAXDDL]
items=[q for q in Q if q['db_id'] in use]
random.seed(0); random.shuffle(items)
train=items[:36]; test=items[36:54]
print(f"DBs used (schema fits): {use}",flush=True)
print(f"train={len(train)} held-out={len(test)} (difficulties: "+str({d:sum(1 for q in test if q['difficulty']==d) for d in ('simple','moderate','challenging')})+")",flush=True)
def run_sql(db,sql):
    if not re.match(r'(?is)^\s*select\b', sql.strip()): return ("ERR","not a SELECT")
    if re.search(r'(?i)\b(drop|delete|update|insert|attach|pragma|create|alter)\b', sql): return ("ERR","forbidden")
    try:
        c=sqlite3.connect(dbpath(db)); c.execute("PRAGMA query_only=ON"); rows=c.execute(sql).fetchall(); c.close(); return ("OK",rows)
    except Exception as e: return ("ERR",str(e)[:120])
norm=lambda rows: sorted([tuple(str(x) for x in r) for r in rows])
def correct(db,cand,gold):
    s,c=run_sql(db,cand); s2,g=run_sql(db,gold); return s=="OK" and s2=="OK" and norm(c)==norm(g)
def extract(out):
    t=out.split('</think>')[-1]; m=re.search(r'```(?:sql)?\s*(.*?)```',t,re.S); sql=(m.group(1) if m else t).strip()
    m2=re.search(r'(?is)(select\b.*?)(;|$)',sql); return (m2.group(1).strip() if m2 else sql)
from mlx_lm import load,generate
def gen(model,tok,p,mx=256):
    return generate(model,tok,prompt=tok.apply_chat_template([{"role":"user","content":p}],add_generation_prompt=True,tokenize=False),max_tokens=mx,verbose=False)
def oneshot(q):
    return (f"SQLite schema:\n{ddl(q['db_id'])}\n\n-- Hint: {q['evidence']}\nQuestion: {q['question']}\n"
            f"Write ONE SQLite SELECT query that answers it. Output only the SQL. /no_think")
def scaffold(model,tok,q):
    sql=extract(gen(model,tok,oneshot(q)))
    for _ in range(2):
        if correct(q['db_id'],sql,q['SQL']): return sql,True
        st,info=run_sql(q['db_id'],sql)
        fb=(f"The query errored: {info}." if st=="ERR" else f"The query ran but returned {len(info)} rows and is not correct.")
        sql=extract(gen(model,tok,f"SQLite schema:\n{ddl(q['db_id'])}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}\nYour query: {sql}\n{fb} Write a corrected single SELECT. Output only SQL. /no_think"))
    return sql,correct(q['db_id'],sql,q['SQL'])
# diagnose-before-kill: verify gold executes + a sample candidate parses+executes
goldbad=sum(1 for q in train+test if run_sql(q['db_id'],q['SQL'])[0]!="OK")
print(f"gold-SQL exec check: {goldbad}/{len(train+test)} golds errored (should be ~0)",flush=True)
model,tok=load(STU,adapter_path=None)
print(f"=== PROBE (8B on REAL BIRD, train n={len(train)}): one-shot floor vs scaffold-cross ===",flush=True)
os1=0; sc=0; rows=[]
for q in train:
    s1=extract(gen(model,tok,oneshot(q))); o1=correct(q['db_id'],s1,q['SQL']); os1+=o1
    sql,ok=scaffold(model,tok,q); sc+=ok
    if ok: rows.append({"prompt":oneshot(q),"completion":" "+sql})
print(f"  one-shot (FLOOR): {os1}/{len(train)}   scaffold-cross: {sc}/{len(train)}  -> {len(rows)} traces",flush=True)
tb=sum(correct(q['db_id'],extract(gen(model,tok,oneshot(q))),q['SQL']) for q in test)
print(f"  HELD-OUT one-shot before: {tb}/{len(test)}",flush=True)
if PHASE!="full":
    print("PROBE done. floor low + scaffold higher -> PHASE=full.",flush=True); sys.exit(0)
d=f"{W}/data"; open(f"{d}/train.jsonl","w").write("\n".join(json.dumps(r) for r in rows)+"\n")
open(f"{d}/valid.jsonl","w").write("\n".join(json.dumps(r) for r in rows[:max(2,len(rows)//5)])+"\n")
del model,tok; adapter=f"{W}/adapter"; os.makedirs(adapter,exist_ok=True)
subprocess.run([sys.executable,"-m","mlx_lm.lora","--model",STU,"--train","--data",d,"--batch-size","2",
  "--num-layers","16","--iters","200","--learning-rate","1e-4","--adapter-path",adapter],capture_output=True,text=True)
m1,t1=load(STU,adapter_path=adapter)
ta=sum(correct(q['db_id'],extract(gen(m1,t1,oneshot(q))),q['SQL']) for q in test)
print(f"\n=== BIRD USABILITY PROOF (held-out one-shot execution accuracy) ===",flush=True)
print(f"  one-shot before {tb}/{len(test)} -> after {ta}/{len(test)}",flush=True)
print(f"  USABLE ✓ iff after >> before (weak 8B -> usable on REAL BIRD SQL, locally, zero salary).",flush=True)
PY
