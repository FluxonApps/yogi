#!/usr/bin/env bash
# TOOLSPACE EVOLUTION (verified selection) — add ONE candidate tool to the confirmed base and keep it only if
# it raises accuracy (DGM/Voyager-style; v2 lesson: select, don't expand). Base = FKs + sample VALUES +
# run/fix (confirmed 39/80 = 48%). Candidate this run: LIKEFIND <text> (fuzzy: which columns contain a value
# via LIKE) — targets WHERE-value mismatches. Same slice items[0:80], compare to base 39/80. Zero salary.
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
def tinfo(db):
    c=sqlite3.connect(dbpath(db)); ts=[r[0] for r in c.execute("SELECT name FROM sqlite_master WHERE type='table'")]; txt={}
    for t in ts:
        info=c.execute(f'PRAGMA table_info("{t}")').fetchall(); txt[t]=[r[1] for r in info if 'CHAR' in (r[2] or '').upper() or 'TEXT' in (r[2] or '').upper()]
    c.close(); return txt
def VALUES(db,table,col):
    try:
        c=sqlite3.connect(dbpath(db)); r=[str(x[0]) for x in c.execute(f'SELECT DISTINCT "{col}" FROM "{table}" WHERE "{col}" IS NOT NULL LIMIT 5')]; c.close()
        return f"{table}.{col} e.g. {r}"
    except Exception as e: return f"{table}.{col}: err {str(e)[:30]}"
def LIKEFIND(db,val,txt):
    hits=[]; c=sqlite3.connect(dbpath(db)); c.execute("PRAGMA query_only=ON"); pat=f"%{val}%"
    for t,cs in txt.items():
        for col in cs:
            try:
                row=c.execute(f'SELECT "{col}" FROM "{t}" WHERE "{col}" LIKE ? LIMIT 1',(pat,)).fetchone()
                if row: hits.append(f"{t}.{col} (e.g. {str(row[0])[:30]})")
            except Exception: pass
            if len(hits)>=6: break
        if len(hits)>=6: break
    c.close(); return f"LIKEFIND '{val}' -> {hits if hits else 'no match'}"
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
def agent(q):  # base + LIKEFIND candidate, single explore round
    db=q['db_id']; txt=tinfo(db)
    ctx=f"SQLite schema:\n{schema(db)}\nForeign keys:\n{fks(db)}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}"
    ex=gen(ctx+"\nBefore writing SQL, investigate. Emit up to 4 lines, each:\n  VALUES table.column\n  LIKEFIND <text>   (find which column holds a value)\nOutput only those lines. /no_think",160).split('</think>')[-1]
    obs=[]
    for line in ex.splitlines():
        if len(obs)>=4: break
        mv=re.match(r'\s*VALUES\s+([A-Za-z_]\w*)\.([A-Za-z_]\w*)',line); mf=re.match(r'\s*LIKEFIND\s+(.+)',line)
        if mv: obs.append(VALUES(db,mv.group(1),mv.group(2)))
        elif mf: obs.append(LIKEFIND(db,mf.group(1).strip().strip("'\""),txt))
    base=ctx+(("\nInvestigated:\n"+"\n".join(obs)) if obs else "")+"\nWrite ONE SQLite SELECT query. Output only the SQL. /no_think"
    sql=exsql(gen(base)); rows,err=run(db,sql); final=sql
    for _ in range(T-1):
        if err is None and rows: break
        fb=f"ERROR: {err}" if err else f"ran OK, {len(rows)} rows: {str(rows[:3])[:120]}"
        sql=exsql(gen(base+f"\nYour query:\n{sql}\nResult — {fb}\nIf wrong/errored correct it (FKs+values). Else repeat. Only SQL. /no_think")); rows,err=run(db,sql); final=sql
    return final
ok=0
for i,q in enumerate(test):
    gold=key(run(q['db_id'],q['SQL'])[0])
    if gold is None: continue
    if key(run(q['db_id'],agent(q))[0])==gold: ok+=1
    if (i+1)%20==0: print(f"  {i+1}/{len(test)}: base+LIKEFIND {ok}",flush=True)
n=len(test)
print(f"\n=== TOOLSPACE EVOLVE — base + LIKEFIND candidate (items[{A}:{B}], n={n}, zero salary) ===",flush=True)
print(f"  base+LIKEFIND {ok}/{n} ({100*ok//n}%)   vs  base(FKs+VALUES) 39/80 (48%)",flush=True)
print(f"  KEEP LIKEFIND ✓ iff > 39/80 (verified selection: keep only tools that raise accuracy).",flush=True)
PY
