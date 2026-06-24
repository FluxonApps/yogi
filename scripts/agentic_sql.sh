#!/usr/bin/env bash
# TOOLSPACE / ACTION-SPACE lever (Stage A, FREE): does giving the 8B a run_sql TOOL with execution feedback
# cross the ~40% one-shot/oracle CAPABILITY ceiling? The bet (F6/F7): the ceiling is one-shot GENERATION;
# the agentic LOOP (draft→run→read-error→fix) is a HOMOGENEOUS skill that crosses it. Gold-free stop (runs +
# non-empty). Compare to one-shot 13/40 + oracle 16/40 on the SAME held-out. Zero salary (Stage B = distill
# the loop with salary-authored traces). One model, eval-only.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit; BIRD=/tmp/yogi_bird/minidev/MINIDEV; T="${T:-5}"; NTE="${NTE:-40}"
"$PY" - "$STUDENT" "$BIRD" "$T" "$NTE" <<'PY'
import sqlite3,sys,re,json,random
STU,BIRD,T,NTE=sys.argv[1],sys.argv[2],int(sys.argv[3]),int(sys.argv[4])
DBDIR=f"{BIRD}/dev_databases"; dbpath=lambda db:f"{DBDIR}/{db}/{db}.sqlite"
def compact_schema(db):
    c=sqlite3.connect(dbpath(db)); ts=c.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall(); o=[]
    for (t,) in ts: o.append(f"{t}({', '.join(r[1] for r in c.execute(f'PRAGMA table_info(\"{t}\")').fetchall())})")
    c.close(); return "\n".join(o)
Q=json.load(open(f"{BIRD}/mini_dev_sqlite.json"))
cand=["toxicology","california_schools","debit_card_specializing","student_club","superhero","financial"]
use=[db for db in cand if len(compact_schema(db))<2500]
items=[q for q in Q if q['db_id'] in use and q['difficulty'] in ('simple','moderate')]
random.seed(0); random.shuffle(items); test=items[130:130+NTE]   # SAME held-out as pass@k / best-of-N
def run_verbose(db,sql):  # the run_sql TOOL: returns (rows, error_or_None)
    if not re.match(r'(?is)^\s*select\b',sql.strip()) or re.search(r'(?i)\b(drop|delete|update|insert|attach|pragma|create|alter)\b',sql): return (None,"only SELECT allowed")
    try:
        c=sqlite3.connect(dbpath(db)); c.execute("PRAGMA query_only=ON"); r=c.execute(sql).fetchall(); c.close(); return (r,None)
    except Exception as e: return (None,str(e)[:160])
key=lambda rows: None if rows is None else tuple(sorted(tuple(str(x) for x in r) for r in rows))
def exsql(t):
    t=t.split('</think>')[-1]; m=re.findall(r'```(?:sql)?\s*(.*?)```',t,re.S); s=(m[-1] if m else t).strip()
    m2=re.findall(r'(?is)(select\b.*?)(?:;|$)',s); return (m2[-1].strip() if m2 else s)
base=lambda q:(f"SQLite schema:\n{compact_schema(q['db_id'])}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}\nWrite ONE SQLite SELECT query. Output only the SQL. /no_think")
def refine(sql,fb,rows):
    obs = f"ERROR: {fb}" if fb else f"ran OK, returned {len(rows)} row(s): {str(rows[:3])[:160]}"
    return (f"Your previous query:\n{sql}\nExecution result — {obs}\nIf it errored or doesn't answer the question, write a CORRECTED SQLite SELECT (fix table/column names, joins, filters). Otherwise repeat it. Output only the SQL. /no_think")
from mlx_lm import load,generate
m,t=load(STU)
def gen(p): return generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":p}],add_generation_prompt=True,tokenize=False),max_tokens=220,verbose=False)
agentic=0; turns_used=[]
for i,q in enumerate(test):
    gold=key(run_verbose(q['db_id'],q['SQL'])[0])
    sql=exsql(gen(base(q))); rows,err=run_verbose(q['db_id'],sql); final=sql; ut=1
    for turn in range(T-1):
        if err is None and rows:   # TOOL says: runs + non-empty → gold-free accept
            break
        sql=exsql(gen(base(q)+"\n"+refine(sql,err,rows))); rows,err=run_verbose(q['db_id'],sql); final=sql; ut+=1
    turns_used.append(ut)
    if key(run_verbose(q['db_id'],final)[0])==gold and gold is not None: agentic+=1
    if (i+1)%10==0: print(f"  {i+1}/{len(test)} done (avg turns {sum(turns_used)/len(turns_used):.1f})",flush=True)
n=len(test)
print(f"\n=== AGENTIC SQL — run_sql tool + execution-feedback refine (held-out n={n}, T={T}, zero salary) ===",flush=True)
print(f"  agentic {agentic}/{n} ({100*agentic//n}%)   vs   one-shot 13/40 (33%)   oracle best-of-8 16/40 (40%)",flush=True)
print(f"  avg turns used: {sum(turns_used)/n:.1f}/{T}",flush=True)
print(f"  ACTION-SPACE CROSSES CEILING ✓ iff agentic > 40% (iteration+feedback beats blind sampling's ceiling).",flush=True)
PY
