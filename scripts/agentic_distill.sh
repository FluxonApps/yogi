#!/usr/bin/env bash
# TOOLSPACE-with-SALARY, Stage B: distill the SELF-REFINEMENT LOOP (a HOMOGENEOUS skill) with salary-authored
# traces. Stage A showed the 8B accepts its first running query even when WRONG (no semantic critique). Here a
# frontier teacher (claude -p) authors DRAFT→CHECK(does it answer the question? columns/joins/filters)→FINAL
# traces; the 8B distills the LOOP (skill grain). MEASURE: (a) held-out FINAL accuracy before→after; (b)
# frontier-dependence DECAY — salary buys the toolspace once, the 8B then runs the loop itself (amortize→0).
# Salary-capped; memory-safe LoRA. Held-out = SAME items[130:170] as one-shot(13)/oracle(16)/agentic.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit; BIRD=/tmp/yogi_bird/minidev/MINIDEV
W=/tmp/yogi_agentic; mkdir -p "$W/data"; NTR="${NTR:-64}"; NTE="${NTE:-40}"; CAP="${TEACHER_CAP:-64}"
# Phase A (salary): teacher authors DRAFT→CHECK→FINAL self-refinement traces.
"$PY" - "$W" "$BIRD" "$NTR" "$NTE" "$CAP" <<'PY'
import sqlite3,sys,re,json,subprocess,random
W,BIRD,NTR,NTE,CAP=sys.argv[1],sys.argv[2],int(sys.argv[3]),int(sys.argv[4]),int(sys.argv[5])
DBDIR=f"{BIRD}/dev_databases"; dbpath=lambda db:f"{DBDIR}/{db}/{db}.sqlite"
def cs(db):
    c=sqlite3.connect(dbpath(db)); ts=c.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall(); o=[]
    for (t,) in ts: o.append(f"{t}({', '.join(r[1] for r in c.execute(f'PRAGMA table_info(\"{t}\")').fetchall())})")
    c.close(); return "\n".join(o)
Q=json.load(open(f"{BIRD}/mini_dev_sqlite.json"))
cand=["toxicology","california_schools","debit_card_specializing","student_club","superhero","financial"]
use=[db for db in cand if len(cs(db))<2500]
items=[q for q in Q if q['db_id'] in use and q['difficulty'] in ('simple','moderate')]
random.seed(0); random.shuffle(items); train=items[:NTR]; test=items[130:130+NTE]  # disjoint; test == agentic held-out
def run(db,sql):
    if not re.match(r'(?is)^\s*select\b',sql.strip()) or re.search(r'(?i)\b(drop|delete|update|insert|attach|pragma|create|alter)\b',sql): return None
    try:
        c=sqlite3.connect(dbpath(db)); c.execute("PRAGMA query_only=ON"); r=c.execute(sql).fetchall(); c.close(); return r
    except Exception: return None
key=lambda rows: None if rows is None else tuple(sorted(tuple(str(x) for x in r) for r in rows))
def finalsql(t):  # FINAL = last ```sql``` block (after DRAFT)
    t=t.split('</think>')[-1]; m=re.findall(r'```(?:sql)?\s*(.*?)```',t,re.S); s=(m[-1] if m else t).strip()
    m2=re.findall(r'(?is)(select\b.*?)(?:;|$)',s); return (m2[-1].strip() if m2 else s)
INSTR=("Solve with SELF-REFINEMENT: \nDRAFT: a first SQLite SELECT. \nCHECK: does it correctly answer the "
       "question? Verify table/column names exist, joins, filters, aggregation. \nFINAL: the corrected query "
       "as ```sql ... ```.")
prompt=lambda q:(f"SQLite schema:\n{cs(q['db_id'])}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}\n{INSTR}")
def teacher(q):
    try: return subprocess.run(["claude","-p",prompt(q)],capture_output=True,text=True,timeout=150).stdout
    except Exception: return ""
rows=[]; ok=0; used=0
for q in train:
    if used>=CAP: break
    used+=1; out=teacher(q).split('</think>')[-1].strip(); fsql=finalsql(out)
    if key(run(q['db_id'],fsql))==key(run(q['db_id'],q['SQL'])) and key(run(q['db_id'],q['SQL'])) is not None:
        ok+=1; rows.append({"prompt":prompt(q)+" /no_think","completion":" "+out})
    if used%16==0: print(f"  teacher {used}/{min(CAP,len(train))} -> {ok} verified self-refine traces",flush=True)
print(f"TOOLSPACE traces (salary={used} claude calls): {ok} verified DRAFT-CHECK-FINAL",flush=True)
d=f"{W}/data"; open(f"{d}/train.jsonl","w").write("\n".join(json.dumps(r) for r in rows)+"\n")
open(f"{d}/valid.jsonl","w").write("\n".join(json.dumps(r) for r in rows[:max(3,len(rows)//6)])+"\n")
json.dump([{"db_id":q['db_id'],"q":q,"gold":q['SQL']} for q in test], open(f"{d}/test.json","w"))
print(f"SALARY_SPENT={used}",flush=True)
PY
# Phase B (free): eval held-out FINAL before, LoRA distill the loop (memory-safe), eval after.
"$PY" - "$W" "$STUDENT" "$BIRD" <<'PY'
import sqlite3,sys,re,json,subprocess,os
W,STU,BIRD=sys.argv[1],sys.argv[2],sys.argv[3]
DBDIR=f"{BIRD}/dev_databases"; dbpath=lambda db:f"{DBDIR}/{db}/{db}.sqlite"
def cs(db):
    c=sqlite3.connect(dbpath(db)); ts=c.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall(); o=[]
    for (t,) in ts: o.append(f"{t}({', '.join(r[1] for r in c.execute(f'PRAGMA table_info(\"{t}\")').fetchall())})")
    c.close(); return "\n".join(o)
def run(db,sql):
    if not re.match(r'(?is)^\s*select\b',sql.strip()) or re.search(r'(?i)\b(drop|delete|update|insert|attach|pragma|create|alter)\b',sql): return None
    try:
        c=sqlite3.connect(dbpath(db)); c.execute("PRAGMA query_only=ON"); r=c.execute(sql).fetchall(); c.close(); return r
    except Exception: return None
key=lambda rows: None if rows is None else tuple(sorted(tuple(str(x) for x in r) for r in rows))
def finalsql(t):
    t=t.split('</think>')[-1]; m=re.findall(r'```(?:sql)?\s*(.*?)```',t,re.S); s=(m[-1] if m else t).strip()
    m2=re.findall(r'(?is)(select\b.*?)(?:;|$)',s); return (m2[-1].strip() if m2 else s)
INSTR=("Solve with SELF-REFINEMENT: \nDRAFT: a first SQLite SELECT. \nCHECK: does it correctly answer the "
       "question? Verify table/column names exist, joins, filters, aggregation. \nFINAL: the corrected query "
       "as ```sql ... ```.")
prompt=lambda q:(f"SQLite schema:\n{cs(q['db_id'])}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}\n{INSTR} /no_think")
test=json.load(open(f"{W}/data/test.json"))
from mlx_lm import load,generate
def g(m,t,q): return generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":prompt(q['q'])}],add_generation_prompt=True,tokenize=False),max_tokens=320,verbose=False)
m,t=load(STU)
tb=sum(key(run(x['db_id'],finalsql(g(m,t,x))))==key(run(x['db_id'],x['gold'])) and key(run(x['db_id'],x['gold'])) is not None for x in test)
print(f"HELD-OUT self-refine FINAL before: {tb}/{len(test)}",flush=True)
del m,t; ad=f"{W}/adapter"; os.makedirs(ad,exist_ok=True)
r=subprocess.run([sys.executable,"-m","mlx_lm.lora","--model",STU,"--train","--data",f"{W}/data","--batch-size","1",
  "--num-layers","8","--iters","250","--learning-rate","1e-4","--max-seq-length","896","--grad-checkpoint","--adapter-path",ad],capture_output=True,text=True)
if not os.path.exists(f"{ad}/adapters.safetensors"): print("LoRA FAILED:\n"+(r.stderr or r.stdout)[-800:]); sys.exit(1)
m,t=load(STU,adapter_path=ad)
ta=sum(key(run(x['db_id'],finalsql(g(m,t,x))))==key(run(x['db_id'],x['gold'])) and key(run(x['db_id'],x['gold'])) is not None for x in test)
print(f"\n=== TOOLSPACE-with-SALARY RESULT (held-out self-refine FINAL, n={len(test)}) ===",flush=True)
print(f"  before {tb}/{len(test)} -> after {ta}/{len(test)}  (vs one-shot 13/40, oracle 16/40)",flush=True)
print(f"  INTERNALIZED LOOP ✓ iff after >> before AND > 40% — the homogeneous tool-use skill crosses the capability ceiling.",flush=True)
PY
