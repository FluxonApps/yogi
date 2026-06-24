#!/usr/bin/env bash
# TOOLSPACE EVOLUTION — iteration 1: does an INTERACTIVE multi-tool agent cross the 40% wall?
# The wall held for static rich-schema (37%). Bet: letting the model EXPLORE the DB before committing
# (foreign-key paths + on-demand sample VALUES) surfaces the right columns/joins/value-formats it otherwise
# guesses wrong. Tools: FKS (join paths, proactive) + VALUES table.col (on request) + RUN (execute+fix).
# Free verifier, zero salary, same held-out n=40 as every prior row. Compare to agentic-plain 16/40 (40%).
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit; BIRD=/tmp/yogi_bird/minidev/MINIDEV; T="${T:-3}"; NTE="${NTE:-40}"
"$PY" - "$STUDENT" "$BIRD" "$T" "$NTE" <<'PY'
import sqlite3,sys,re,json,random
STU,BIRD,T,NTE=sys.argv[1],sys.argv[2],int(sys.argv[3]),int(sys.argv[4])
DBDIR=f"{BIRD}/dev_databases"; dbpath=lambda db:f"{DBDIR}/{db}/{db}.sqlite"
def schema(db):
    c=sqlite3.connect(dbpath(db)); ts=c.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall(); o=[]
    for (t,) in ts: o.append(f"{t}({', '.join(r[1] for r in c.execute(f'PRAGMA table_info(\"{t}\")').fetchall())})")
    c.close(); return "\n".join(o)
def fks(db):  # TOOL: foreign-key join paths (proactive — joins are the #1 failure)
    c=sqlite3.connect(dbpath(db)); ts=[r[0] for r in c.execute("SELECT name FROM sqlite_master WHERE type='table'")]; o=[]
    for t in ts:
        for r in c.execute(f'PRAGMA foreign_key_list("{t}")').fetchall():
            o.append(f"{t}.{r[3]} -> {r[2]}.{r[4]}")
    c.close(); return "\n".join(o) if o else "(none declared)"
def values(db,table,col):  # TOOL: sample distinct values
    try:
        c=sqlite3.connect(dbpath(db)); r=[str(x[0]) for x in c.execute(f'SELECT DISTINCT "{col}" FROM "{table}" WHERE "{col}" IS NOT NULL LIMIT 5')]; c.close()
        return f"{table}.{col} e.g. {r}"
    except Exception as e: return f"{table}.{col}: (error {str(e)[:40]})"
def run(db,sql):  # TOOL: execute
    if not re.match(r'(?is)^\s*select\b',sql.strip()) or re.search(r'(?i)\b(drop|delete|update|insert|attach|pragma|create|alter)\b',sql): return (None,"only SELECT allowed")
    try:
        c=sqlite3.connect(dbpath(db)); c.execute("PRAGMA query_only=ON"); r=c.execute(sql).fetchall(); c.close(); return (r,None)
    except Exception as e: return (None,str(e)[:160])
key=lambda rows: None if rows is None else tuple(sorted(tuple(str(x) for x in r) for r in rows))
def exsql(t):
    t=t.split('</think>')[-1]; m=re.findall(r'```(?:sql)?\s*(.*?)```',t,re.S); s=(m[-1] if m else t).strip()
    m2=re.findall(r'(?is)(select\b.*?)(?:;|$)',s); return (m2[-1].strip() if m2 else s)
Q=json.load(open(f"{BIRD}/mini_dev_sqlite.json"))
cand=["toxicology","california_schools","debit_card_specializing","student_club","superhero","financial"]
use=[db for db in cand if len(schema(db))<2500]
items=[q for q in Q if q['db_id'] in use and q['difficulty'] in ('simple','moderate')]
random.seed(0); random.shuffle(items); test=items[130:130+NTE]
from mlx_lm import load,generate
m,t=load(STU)
def gen(p,mx=240): return generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":p}],add_generation_prompt=True,tokenize=False),max_tokens=mx,verbose=False)
def explore(q):  # Phase 1: model requests sample values for columns it cares about
    p=(f"SQLite schema:\n{schema(q['db_id'])}\nForeign keys:\n{fks(q['db_id'])}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}\n"
       f"Before writing SQL, investigate. List up to 4 columns whose example values you want to see, each on its own line as:\nVALUES table.column\nOutput only those lines. /no_think")
    out=gen(p,150).split('</think>')[-1]
    obs=[]
    for line in out.splitlines():
        mm=re.match(r'\s*VALUES\s+([A-Za-z_][\w]*)\.([A-Za-z_][\w]*)',line)
        if mm and len(obs)<4: obs.append(values(q['db_id'],mm.group(1),mm.group(2)))
    return obs
ok=0; tot=0
for i,q in enumerate(test):
    gold=key(run(q['db_id'],q['SQL'])[0])
    obs=explore(q); obstxt=("\nInvestigated values:\n"+"\n".join(obs)) if obs else ""
    base=(f"SQLite schema:\n{schema(q['db_id'])}\nForeign keys:\n{fks(q['db_id'])}{obstxt}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}\nWrite ONE SQLite SELECT query. Output only the SQL. /no_think")
    sql=exsql(gen(base)); rows,err=run(q['db_id'],sql); final=sql; ut=1; tot+=len(obs)
    for _ in range(T-1):
        if err is None and rows: break
        obs2=f"ERROR: {err}" if err else f"ran OK, {len(rows)} row(s): {str(rows[:3])[:140]}"
        sql=exsql(gen(base+f"\nYour previous query:\n{sql}\nResult — {obs2}\nIf wrong/errored, write a corrected SQLite SELECT (check joins via the foreign keys, column names, value formats). Else repeat. Output only SQL. /no_think")); rows,err=run(q['db_id'],sql); final=sql; ut+=1
    if key(run(q['db_id'],final)[0])==gold and gold is not None: ok+=1
    if (i+1)%10==0: print(f"  {i+1}/{len(test)} done ({ok} correct so far)",flush=True)
n=len(test)
print(f"\n=== TOOLSPACE v1 — explore(FKs+VALUES) + run/fix (held-out n={n}, zero salary) ===",flush=True)
print(f"  toolspace {ok}/{n} ({100*ok//n}%)   vs  agentic-plain 16/40 (40%)  one-shot 13/40 (33%)  rich-schema 15/40 (37%)",flush=True)
print(f"  avg sample-value probes used: {tot/n:.1f}",flush=True)
print(f"  CROSSES THE WALL ✓ iff toolspace > 40% — interactive exploration beats one-shot generation.",flush=True)
PY
