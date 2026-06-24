#!/usr/bin/env bash
# AUTONOMOUS TOOL INVENTION (the bold frontier) — the local 8B invents reusable read-only VIEWS that pre-join
# / pre-aggregate the schema (from the foreign keys), then writes SQL against the SIMPLER schema. Unlike
# context augmentations (which were all flat), this CHANGES the model's effective runtime environment, which
# is the only thing that helped (interactive feedback). Safe: read-only DB (mode=ro) + TEMP views only; agent
# SQL is SELECT-only. Verified-select vs base 48% on items[0:80]. Zero salary, one model.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit; BIRD=/tmp/yogi_bird/minidev/MINIDEV; T="${T:-3}"; A="${A:-0}"; B="${B:-80}"
"$PY" - "$STUDENT" "$BIRD" "$T" "$A" "$B" <<'PY'
import sqlite3,sys,re,json,random
STU,BIRD,T,A,B=sys.argv[1],sys.argv[2],int(sys.argv[3]),int(sys.argv[4]),int(sys.argv[5])
DBDIR=f"{BIRD}/dev_databases"; dbpath=lambda db:f"{DBDIR}/{db}/{db}.sqlite"
roconn=lambda db: sqlite3.connect(f"file:{dbpath(db)}?mode=ro",uri=True)
def schema(db):
    c=roconn(db); ts=c.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall(); o=[]
    for (t,) in ts: o.append(f"{t}({', '.join(r[1] for r in c.execute(f'PRAGMA table_info(\"{t}\")').fetchall())})")
    c.close(); return "\n".join(o)
def fks(db):
    c=roconn(db); ts=[r[0] for r in c.execute("SELECT name FROM sqlite_master WHERE type='table'")]; o=[]
    for t in ts:
        for r in c.execute(f'PRAGMA foreign_key_list("{t}")').fetchall(): o.append(f"{t}.{r[3]} -> {r[2]}.{r[4]}")
    c.close(); return "\n".join(o) if o else "(none declared)"
SELONLY=lambda s: re.match(r'(?is)^\s*select\b',s.strip()) and not re.search(r'(?i)\b(drop|delete|update|insert|attach|pragma|alter)\b',s)
def open_with_views(db,views):
    c=roconn(db)
    made=[]
    for name,sel in views:
        try: c.execute(f'CREATE TEMP VIEW "{name}" AS {sel}'); c.execute(f'SELECT * FROM "{name}" LIMIT 1'); made.append(name)
        except Exception: pass
    return c,made
def run_c(c,sql):
    if not SELONLY(sql): return (None,"only SELECT allowed")
    try: return (c.execute(sql).fetchall(),None)
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
def gen(p,mx=320): return generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":p}],add_generation_prompt=True,tokenize=False),max_tokens=mx,verbose=False)
def invent(db):  # 8B invents reusable views from schema + FKs + sample questions
    qs=[q['question'] for q in items if q['db_id']==db][:3]
    p=(f"SQLite schema:\n{schema(db)}\nForeign keys:\n{fks(db)}\nExample questions:\n- "+"\n- ".join(qs)+
       "\nInvent up to 2 reusable VIEWS that PRE-JOIN related tables (using the foreign keys) so future queries avoid re-deriving joins. "
       "Each as: CREATE VIEW name AS SELECT ...; Use clear column aliases. Output only the CREATE VIEW statements. /no_think")
    out=gen(p,360).split('</think>')[-1]
    views=[]
    for mm in re.finditer(r'(?is)create\s+view\s+"?([A-Za-z_]\w*)"?\s+as\s+(select\b.*?)(?:;|$)',out):
        if SELONLY(mm.group(2)): views.append((mm.group(1),mm.group(2).strip()))
    return views[:2]
VIEWS={db:invent(db) for db in use}
for db in use:
    c,made=open_with_views(db,VIEWS[db]); c.close(); VIEWS[db]=[(n,s) for (n,s) in VIEWS[db] if n in made]
    print(f"  invented views [{db}]: {[n for n,_ in VIEWS[db]]}",flush=True)
def vdesc(db):
    if not VIEWS[db]: return ""
    out=[]
    c,_=open_with_views(db,VIEWS[db])
    for n,_s in VIEWS[db]:
        try: cols=[r[1] for r in c.execute(f'PRAGMA table_info("{n}")').fetchall()]; out.append(f"{n}({', '.join(cols)})")
        except Exception: pass
    c.close(); return "\nInvented views you may query directly:\n"+"\n".join(out) if out else ""
def agent(q):
    db=q['db_id']; c,_=open_with_views(db,VIEWS[db])
    ctx=f"SQLite schema:\n{schema(db)}\nForeign keys:\n{fks(db)}{vdesc(db)}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}"
    base=ctx+"\nWrite ONE SQLite SELECT query (you may use the invented views to simplify joins). Output only the SQL. /no_think"
    sql=exsql(gen(base)); rows,err=run_c(c,sql); final=sql
    for _ in range(T-1):
        if err is None and rows: break
        fb=f"ERROR: {err}" if err else f"ran OK, {len(rows)} rows: {str(rows[:3])[:120]}"
        sql=exsql(gen(base+f"\nYour query:\n{sql}\nResult — {fb}\nIf wrong/errored correct it (use the views or the foreign keys). Else repeat. Only SQL. /no_think")); rows,err=run_c(c,sql); final=sql
    r=key(run_c(c,final)[0]); c.close(); return r
def goldkey(q):
    c=roconn(q['db_id']); 
    try: g=key(c.execute(q['SQL']).fetchall())
    except Exception: g=None
    c.close(); return g
ok=0
for i,q in enumerate(test):
    gold=goldkey(q)
    if gold is None: continue
    if agent(q)==gold: ok+=1
    if (i+1)%20==0: print(f"  {i+1}/{len(test)}: invent+tools {ok}",flush=True)
n=len(test)
nv=sum(len(VIEWS[db]) for db in use)
print(f"\n=== AUTONOMOUS VIEW INVENTION + TOOLS (items[{A}:{B}], n={n}, {nv} views invented, zero salary) ===",flush=True)
print(f"  invent+tools {ok}/{n} ({100*ok//n}%)   vs  base tools 39/80 (48%)  decompose 42/80 (52%)  one-shot 30/80 (37%)",flush=True)
print(f"  SELF-INVENTION HELPS ✓ iff > 42/80 — the model's own invented abstractions beat hand-designed scaffolds.",flush=True)
PY
