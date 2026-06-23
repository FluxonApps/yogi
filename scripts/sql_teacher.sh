#!/usr/bin/env bash
# BIRD usability — SALARY/TEACHER-BOOTSTRAP (operator's lever; the root fix for trace-thinness). Free
# self-gen only yields ~25% correct traces (the 8B can't solve the hard ones) → too thin to generalize.
# A frontier TEACHER (claude -p) writes correct CoT+SQL for ALL train questions → many correct traces →
# distill into the 8B (SLM-SQL's SFT-on-teacher-CoT recipe). Held-out one-shot before→after = does the 8B
# become SELF-SUFFICIENT (solves held-out it couldn't, at inference, no teacher) = frontier-dependence
# DECAY = salary justified (amortizes→0). Salary-capped; memory-safe LoRA. THE ONE arm that spends salary.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit
BIRD=/tmp/yogi_bird/minidev/MINIDEV; W=/tmp/yogi_sqlteach; mkdir -p "$W/data"; NTR="${NTR:-64}"; NTE="${NTE:-20}"; CAP="${TEACHER_CAP:-64}"
# Phase A (salary): teacher generates+verifies CoT traces — python (no mlx) so it's quick & model-free.
"$PY" - "$W" "$BIRD" "$NTR" "$NTE" "$CAP" <<'PY'
import sqlite3,sys,re,os,json,subprocess,random
W,BIRD,NTR,NTE,CAP=sys.argv[1],sys.argv[2],int(sys.argv[3]),int(sys.argv[4]),int(sys.argv[5])
DBDIR=f"{BIRD}/dev_databases"; dbpath=lambda db:f"{DBDIR}/{db}/{db}.sqlite"
def compact_schema(db):
    c=sqlite3.connect(dbpath(db)); ts=c.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall(); o=[]
    for (t,) in ts: o.append(f"{t}({', '.join(r[1] for r in c.execute(f'PRAGMA table_info(\"{t}\")').fetchall())})")
    c.close(); return "\n".join(o)
Q=json.load(open(f"{BIRD}/mini_dev_sqlite.json"))
cand=["toxicology","california_schools","debit_card_specializing","student_club","superhero","financial"]
use=[db for db in cand if len(compact_schema(db))<2500]
items=[q for q in Q if q['db_id'] in use and q['difficulty'] in ('simple','moderate')]
random.seed(0); random.shuffle(items); train=items[:NTR]; test=items[NTR:NTR+NTE]
def run_sql(db,sql):
    if not re.match(r'(?is)^\s*select\b',sql.strip()) or re.search(r'(?i)\b(drop|delete|update|insert|attach|pragma|create|alter)\b',sql): return ("ERR",None)
    try:
        c=sqlite3.connect(dbpath(db)); c.execute("PRAGMA query_only=ON"); r=c.execute(sql).fetchall(); c.close(); return ("OK",r)
    except Exception: return ("ERR",None)
norm=lambda rows: sorted([tuple(str(x) for x in r) for r in (rows or [])])
def correct(db,c,g):
    s,cr=run_sql(db,c); s2,gr=run_sql(db,g); return s=="OK" and s2=="OK" and norm(cr)==norm(gr)
def exsql(t):
    m=re.findall(r'```(?:sql)?\s*(.*?)```',t,re.S); s=(m[-1] if m else t).strip()
    m2=re.findall(r'(?is)(select\b.*?)(?:;|$)',s); return (m2[-1].strip() if m2 else s)
def teacher(q):  # SALARY: frontier writes correct reasoning + SQL
    p=(f"SQLite schema:\n{compact_schema(q['db_id'])}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}\n"
       f"Write the correct SQLite SELECT. Reason briefly step by step (tables, joins, filters), then give the final query in a ```sql ...``` block.")
    try: return subprocess.run(["claude","-p",p],capture_output=True,text=True,timeout=150).stdout
    except Exception: return ""
def cot(q): return (f"SQLite schema:\n{compact_schema(q['db_id'])}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}\n"
                    f"Reason step by step then give the final query as ```sql ... ```. /no_think")
rows=[]; ok=0; used=0
for q in train:
    if used>=CAP: break
    used+=1; out=teacher(q); sql=exsql(out.split('</think>')[-1])
    if correct(q['db_id'],sql,q['SQL']):
        ok+=1; rows.append({"prompt":cot(q),"completion":" "+out.split('</think>')[-1].strip()})
    if used%16==0: print(f"  teacher {used}/{min(CAP,len(train))} -> {ok} verified",flush=True)
print(f"TEACHER (salary={used} claude calls): {ok} verified CoT traces (free self-gen got ~12)",flush=True)
d=f"{W}/data"; open(f"{d}/train.jsonl","w").write("\n".join(json.dumps(r) for r in rows)+"\n")
open(f"{d}/valid.jsonl","w").write("\n".join(json.dumps(r) for r in rows[:max(3,len(rows)//6)])+"\n")
json.dump([{"db_id":q['db_id'],"q":q,"gold":q['SQL']} for q in test], open(f"{d}/test.json","w"))
print(f"SALARY_SPENT={used}",flush=True)
PY
# Phase B (free): eval held-out before, LoRA distill teacher traces (memory-safe), eval after.
"$PY" - "$W" "$STUDENT" "$BIRD" <<'PY'
import sqlite3,sys,re,os,json,subprocess
W,STU,BIRD=sys.argv[1],sys.argv[2],sys.argv[3]
DBDIR=f"{BIRD}/dev_databases"; dbpath=lambda db:f"{DBDIR}/{db}/{db}.sqlite"
def compact_schema(db):
    c=sqlite3.connect(dbpath(db)); ts=c.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall(); o=[]
    for (t,) in ts: o.append(f"{t}({', '.join(r[1] for r in c.execute(f'PRAGMA table_info(\"{t}\")').fetchall())})")
    c.close(); return "\n".join(o)
def run_sql(db,sql):
    if not re.match(r'(?is)^\s*select\b',sql.strip()) or re.search(r'(?i)\b(drop|delete|update|insert|attach|pragma|create|alter)\b',sql): return ("ERR",None)
    try:
        c=sqlite3.connect(dbpath(db)); c.execute("PRAGMA query_only=ON"); r=c.execute(sql).fetchall(); c.close(); return ("OK",r)
    except Exception: return ("ERR",None)
norm=lambda rows: sorted([tuple(str(x) for x in r) for r in (rows or [])])
def correct(db,c,g):
    s,cr=run_sql(db,c); s2,gr=run_sql(db,g); return s=="OK" and s2=="OK" and norm(cr)==norm(gr)
def exsql(t):
    m=re.findall(r'```(?:sql)?\s*(.*?)```',t,re.S); s=(m[-1] if m else t).strip()
    m2=re.findall(r'(?is)(select\b.*?)(?:;|$)',s); return (m2[-1].strip() if m2 else s)
cot=lambda q:(f"SQLite schema:\n{compact_schema(q['db_id'])}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}\nReason step by step then give the final query as ```sql ... ```. /no_think")
test=json.load(open(f"{W}/data/test.json"))
from mlx_lm import load,generate
def g(m,t,q): return generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":cot(q['q'])}],add_generation_prompt=True,tokenize=False),max_tokens=300,verbose=False)
m,t=load(STU)
tb=sum(correct(x['db_id'],exsql(g(m,t,x).split('</think>')[-1]),x['gold']) for x in test)
print(f"HELD-OUT one-shot before (8B, no teacher): {tb}/{len(test)}",flush=True)
del m,t; ad=f"{W}/adapter"; os.makedirs(ad,exist_ok=True)
r=subprocess.run([sys.executable,"-m","mlx_lm.lora","--model",STU,"--train","--data",f"{W}/data","--batch-size","1",
  "--num-layers","8","--iters","250","--learning-rate","1e-4","--max-seq-length","768","--grad-checkpoint","--adapter-path",ad],capture_output=True,text=True)
if not os.path.exists(f"{ad}/adapters.safetensors"): print("LoRA FAILED:\n"+(r.stderr or r.stdout)[-800:]); sys.exit(1)
m,t=load(STU,adapter_path=ad)
ta=sum(correct(x['db_id'],exsql(g(m,t,x).split('</think>')[-1]),x['gold']) for x in test)
print(f"\n=== BIRD TEACHER-BOOTSTRAP RESULT (held-out one-shot, n={len(test)}) ===",flush=True)
print(f"  before {tb}/{len(test)} -> after {ta}/{len(test)}  (8B self-sufficient at inference, no teacher)",flush=True)
print(f"  SALARY JUSTIFIED ✓ iff after >> before = teacher traces internalized → frontier-dependence decays.",flush=True)
PY
