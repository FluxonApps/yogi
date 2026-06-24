#!/usr/bin/env bash
# TOOLSPACE input-enrichment lever: does a RICHER schema (sample values per column) + the winning agentic
# loop (Stage A, run_sql + execution feedback) RAISE the ~40% ceiling? Hypothesis: much of the 8B's
# generation failure is SCHEMA IGNORANCE (wrong column/table, wrong value format e.g. 'M' vs 'Male') — an
# INPUT gap, not capability. If agentic+rich > 40%, the ceiling was partly schema-knowledge (a new positive).
# Free, local (sample values queried from the DB). One model, eval-only. Compare to agentic 16/40 (40%).
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit; BIRD=/tmp/yogi_bird/minidev/MINIDEV; T="${T:-5}"; NTE="${NTE:-40}"
"$PY" - "$STUDENT" "$BIRD" "$T" "$NTE" <<'PY'
import sqlite3,sys,re,json,random
STU,BIRD,T,NTE=sys.argv[1],sys.argv[2],int(sys.argv[3]),int(sys.argv[4])
DBDIR=f"{BIRD}/dev_databases"; dbpath=lambda db:f"{DBDIR}/{db}/{db}.sqlite"
def plain_schema(db):
    c=sqlite3.connect(dbpath(db)); ts=c.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall(); o=[]
    for (t,) in ts: o.append(f"{t}({', '.join(r[1] for r in c.execute(f'PRAGMA table_info(\"{t}\")').fetchall())})")
    c.close(); return "\n".join(o)
def rich_schema(db):   # cols + up to 2 sample distinct values per column (value-format knowledge)
    c=sqlite3.connect(dbpath(db)); ts=c.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall(); o=[]
    for (t,) in ts:
        cols=[r[1] for r in c.execute(f'PRAGMA table_info("{t}")').fetchall()]; parts=[]
        for col in cols:
            try:
                vs=[str(r[0]) for r in c.execute(f'SELECT DISTINCT "{col}" FROM "{t}" WHERE "{col}" IS NOT NULL LIMIT 2').fetchall()]
                vs=[v[:18] for v in vs]
                parts.append(f"{col}[{','.join(vs)}]" if vs else col)
            except Exception: parts.append(col)
        o.append(f"{t}({', '.join(parts)})")
    c.close(); return "\n".join(o)
Q=json.load(open(f"{BIRD}/mini_dev_sqlite.json"))
cand=["toxicology","california_schools","debit_card_specializing","student_club","superhero","financial"]
use=[db for db in cand if len(plain_schema(db))<2500]
RICH={db:rich_schema(db) for db in use}   # precompute once per db
items=[q for q in Q if q['db_id'] in use and q['difficulty'] in ('simple','moderate')]
random.seed(0); random.shuffle(items); test=items[130:130+NTE]   # SAME held-out as agentic/pass@k/best-of-N
print(f"rich-schema sizes: {[(db,len(RICH[db])) for db in use]}",flush=True)
def run(db,sql):
    if not re.match(r'(?is)^\s*select\b',sql.strip()) or re.search(r'(?i)\b(drop|delete|update|insert|attach|pragma|create|alter)\b',sql): return (None,"only SELECT allowed")
    try:
        c=sqlite3.connect(dbpath(db)); c.execute("PRAGMA query_only=ON"); r=c.execute(sql).fetchall(); c.close(); return (r,None)
    except Exception as e: return (None,str(e)[:160])
key=lambda rows: None if rows is None else tuple(sorted(tuple(str(x) for x in r) for r in rows))
def exsql(t):
    t=t.split('</think>')[-1]; m=re.findall(r'```(?:sql)?\s*(.*?)```',t,re.S); s=(m[-1] if m else t).strip()
    m2=re.findall(r'(?is)(select\b.*?)(?:;|$)',s); return (m2[-1].strip() if m2 else s)
base=lambda q:(f"SQLite schema (with sample values per column):\n{RICH[q['db_id']]}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}\nWrite ONE SQLite SELECT query. Output only the SQL. /no_think")
def refine(sql,fb,rows):
    obs=f"ERROR: {fb}" if fb else f"ran OK, returned {len(rows)} row(s): {str(rows[:3])[:160]}"
    return (f"Your previous query:\n{sql}\nExecution result — {obs}\nIf it errored or doesn't answer the question, write a CORRECTED SQLite SELECT (fix table/column names, joins, filters, value formats). Otherwise repeat it. Output only the SQL. /no_think")
from mlx_lm import load,generate
m,t=load(STU)
def gen(p): return generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":p}],add_generation_prompt=True,tokenize=False),max_tokens=240,verbose=False)
agentic=0; turns=[]
for i,q in enumerate(test):
    gold=key(run(q['db_id'],q['SQL'])[0])
    sql=exsql(gen(base(q))); rows,err=run(q['db_id'],sql); final=sql; ut=1
    for _ in range(T-1):
        if err is None and rows: break
        sql=exsql(gen(base(q)+"\n"+refine(sql,err,rows))); rows,err=run(q['db_id'],sql); final=sql; ut+=1
    turns.append(ut)
    if key(run(q['db_id'],final)[0])==gold and gold is not None: agentic+=1
    if (i+1)%10==0: print(f"  {i+1}/{len(test)} done (avg turns {sum(turns)/len(turns):.1f})",flush=True)
n=len(test)
print(f"\n=== AGENTIC + RICH SCHEMA (held-out n={n}, T={T}, sample-values, zero salary) ===",flush=True)
print(f"  agentic+rich {agentic}/{n} ({100*agentic//n}%)   vs   agentic-plain 16/40 (40%)   one-shot 13/40 (33%)",flush=True)
print(f"  RAISES CEILING ✓ iff agentic+rich > 40% — schema-knowledge (not capability) was part of the bound.",flush=True)
PY
