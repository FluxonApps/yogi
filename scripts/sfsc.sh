#!/usr/bin/env bash
# SFSC — Self-Factorized Skill Curriculum (the NOVEL structural arm). Isolates the GRAIN variable: distill
# FACTORED solutions (sub-skills named → fragments → composed SQL) vs the teacher-WHOLE baseline (same
# source/count/held-out, flat 9→8). If factored generalizes where whole didn't, the learning GRAIN
# (decomposition into reusable sub-skills) is the lever — the SFSC hypothesis. Salary-capped teacher for
# well-powered grounded factored traces; memory-safe LoRA; held-out one-shot before→after.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit
BIRD=/tmp/yogi_bird/minidev/MINIDEV; W=/tmp/yogi_sfsc; mkdir -p "$W/data"; NTR="${NTR:-64}"; NTE="${NTE:-20}"; CAP="${TEACHER_CAP:-64}"
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
random.seed(0); random.shuffle(items); train=items[:NTR]; test=items[NTR:NTR+NTE]  # SAME split as sql_teacher (comparable)
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
# FACTORED prompt (the SFSC grain): name sub-skills, build fragment per sub-skill, compose.
FACT=("Solve by FACTORING into reusable sub-skills. (1) List the SUB-SKILLS needed (e.g. join, filter, "
      "group-aggregate, subquery). (2) For each, write its SQL fragment. (3) Compose them into the final "
      "query as ```sql ... ```.")
def fact_prompt(q): return (f"SQLite schema:\n{compact_schema(q['db_id'])}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}\n{FACT}")
def teacher(q):
    try: return subprocess.run(["claude","-p",fact_prompt(q)],capture_output=True,text=True,timeout=150).stdout
    except Exception: return ""
def cot(q): return fact_prompt(q)+" /no_think"  # distill/eval prompt (factored)
rows=[]; ok=0; used=0
for q in train:
    if used>=CAP: break
    used+=1; out=teacher(q); sql=exsql(out.split('</think>')[-1])
    if correct(q['db_id'],sql,q['SQL']):
        ok+=1; rows.append({"prompt":cot(q),"completion":" "+out.split('</think>')[-1].strip()})
    if used%16==0: print(f"  factored-teacher {used}/{min(CAP,len(train))} -> {ok} verified",flush=True)
print(f"SFSC factored traces: {ok} (salary={used} claude calls)",flush=True)
d=f"{W}/data"; open(f"{d}/train.jsonl","w").write("\n".join(json.dumps(r) for r in rows)+"\n")
open(f"{d}/valid.jsonl","w").write("\n".join(json.dumps(r) for r in rows[:max(3,len(rows)//6)])+"\n")
json.dump([{"db_id":q['db_id'],"q":q,"gold":q['SQL']} for q in test], open(f"{d}/test.json","w"))
print(f"SALARY_SPENT={used}",flush=True)
PY
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
FACT=("Solve by FACTORING into reusable sub-skills. (1) List the SUB-SKILLS needed (e.g. join, filter, "
      "group-aggregate, subquery). (2) For each, write its SQL fragment. (3) Compose them into the final "
      "query as ```sql ... ```.")
cot=lambda q:(f"SQLite schema:\n{compact_schema(q['db_id'])}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}\n{FACT} /no_think")
test=json.load(open(f"{W}/data/test.json"))
from mlx_lm import load,generate
def g(m,t,q): return generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":cot(q['q'])}],add_generation_prompt=True,tokenize=False),max_tokens=400,verbose=False)
m,t=load(STU)
tb=sum(correct(x['db_id'],exsql(g(m,t,x).split('</think>')[-1]),x['gold']) for x in test)
print(f"HELD-OUT factored one-shot before: {tb}/{len(test)}",flush=True)
del m,t; ad=f"{W}/adapter"; os.makedirs(ad,exist_ok=True)
r=subprocess.run([sys.executable,"-m","mlx_lm.lora","--model",STU,"--train","--data",f"{W}/data","--batch-size","1",
  "--num-layers","8","--iters","250","--learning-rate","1e-4","--max-seq-length","896","--grad-checkpoint","--adapter-path",ad],capture_output=True,text=True)
if not os.path.exists(f"{ad}/adapters.safetensors"): print("LoRA FAILED:\n"+(r.stderr or r.stdout)[-800:]); sys.exit(1)
m,t=load(STU,adapter_path=ad)
ta=sum(correct(x['db_id'],exsql(g(m,t,x).split('</think>')[-1]),x['gold']) for x in test)
print(f"\n=== SFSC RESULT (factored distillation, held-out one-shot n={len(test)}) ===",flush=True)
print(f"  before {tb}/{len(test)} -> after {ta}/{len(test)}  (vs teacher-WHOLE baseline 9→8 flat)",flush=True)
print(f"  GRAIN ✓ iff factored after >> before AND >> teacher-whole — decomposition is the lever.",flush=True)
PY
