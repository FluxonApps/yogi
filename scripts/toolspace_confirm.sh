#!/usr/bin/env bash
# TOOLSPACE CONFIRMATION — settle whether interactive exploration robustly beats one-shot at LARGER n.
# v1 (FKs+VALUES+run/fix) hit 47% on n=40 but v2 (richer) fell to 37% — within noise. Here: BOTH one-shot and
# the v1 toolset on the SAME fresh slice items[0:80] (disjoint from the original n=40), so the comparison is
# internal + larger. If v1-tools clearly beats one-shot at n=80, the tool benefit is real. Zero salary.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit; BIRD=/tmp/yogi_bird/minidev/MINIDEV; T="${T:-3}"; A="${A:-0}"; B="${B:-80}"
"$PY" - "$STUDENT" "$BIRD" "$T" "$A" "$B" <<'PY'
import sqlite3,sys,re,json,random
STU,BIRD,T,A,B=sys.argv[1],sys.argv[2],int(sys.argv[3]),int(sys.argv[4]),int(sys.argv[5])
DBDIR=f"{BIRD}/dev_databases"; dbpath=lambda db:f"{DBDIR}/{db}/{db}.sqlite"
def schema(db):
    c=sqlite3.connect(dbpath(db)); ts=c.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall(); o=[]
    for (t,) in ts: o.append(f"{t}({', '.join(r[1] for r in c.execute(f'PRAGMA table_info(\"{t}\")').fetchall())})")
    c.close(); return "\n".join(o)
def fks(db):
    c=sqlite3.connect(dbpath(db)); ts=[r[0] for r in c.execute("SELECT name FROM sqlite_master WHERE type='table'")]; o=[]
    for t in ts:
        for r in c.execute(f'PRAGMA foreign_key_list("{t}")').fetchall(): o.append(f"{t}.{r[3]} -> {r[2]}.{r[4]}")
    c.close(); return "\n".join(o) if o else "(none declared)"
def VALUES(db,table,col):
    try:
        c=sqlite3.connect(dbpath(db)); r=[str(x[0]) for x in c.execute(f'SELECT DISTINCT "{col}" FROM "{table}" WHERE "{col}" IS NOT NULL LIMIT 5')]; c.close()
        return f"{table}.{col} e.g. {r}"
    except Exception as e: return f"{table}.{col}: err {str(e)[:30]}"
def run(db,sql):
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
random.seed(0); random.shuffle(items); test=items[A:B]
from mlx_lm import load,generate
m,t=load(STU)
def gen(p,mx=240): return generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":p}],add_generation_prompt=True,tokenize=False),max_tokens=mx,verbose=False)
def oneshot(q):
    p=f"SQLite schema:\n{schema(q['db_id'])}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}\nWrite ONE SQLite SELECT query. Output only the SQL. /no_think"
    return exsql(gen(p))
def tools(q):
    db=q['db_id']
    ctx=f"SQLite schema:\n{schema(db)}\nForeign keys:\n{fks(db)}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}"
    ex=gen(ctx+"\nBefore writing SQL, list up to 4 columns whose example values you want, each as:\nVALUES table.column\nOutput only those lines. /no_think",150).split('</think>')[-1]
    obs=[]
    for line in ex.splitlines():
        mm=re.match(r'\s*VALUES\s+([A-Za-z_]\w*)\.([A-Za-z_]\w*)',line)
        if mm and len(obs)<4: obs.append(VALUES(db,mm.group(1),mm.group(2)))
    base=ctx+(("\nInvestigated:\n"+"\n".join(obs)) if obs else "")+"\nWrite ONE SQLite SELECT query. Output only the SQL. /no_think"
    sql=exsql(gen(base)); rows,err=run(db,sql); final=sql
    for _ in range(T-1):
        if err is None and rows: break
        fb=f"ERROR: {err}" if err else f"ran OK, {len(rows)} rows: {str(rows[:3])[:120]}"
        sql=exsql(gen(base+f"\nYour query:\n{sql}\nResult — {fb}\nIf wrong/errored correct it (use FKs+values). Else repeat. Only SQL. /no_think")); rows,err=run(db,sql); final=sql
    return final
o_ok=0; t_ok=0
for i,q in enumerate(test):
    gold=key(run(q['db_id'],q['SQL'])[0])
    if gold is None: continue
    if key(run(q['db_id'],oneshot(q))[0])==gold: o_ok+=1
    if key(run(q['db_id'],tools(q))[0])==gold: t_ok+=1
    if (i+1)%20==0: print(f"  {i+1}/{len(test)}: one-shot {o_ok}, tools {t_ok}",flush=True)
n=len(test)
print(f"\n=== TOOLSPACE CONFIRMATION (fresh slice items[{A}:{B}], n={n}, zero salary) ===",flush=True)
print(f"  one-shot   {o_ok}/{n} ({100*o_ok//n}%)",flush=True)
print(f"  v1-tools   {t_ok}/{n} ({100*t_ok//n}%)   (FKs + sample VALUES + run/fix)",flush=True)
print(f"  TOOL BENEFIT REAL ✓ iff tools >> one-shot at this larger n (delta robust to n=40 noise).",flush=True)
PY
