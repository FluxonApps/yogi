#!/usr/bin/env bash
# TOOLSPACE EVOLUTION — iter 2: richer toolset + multi-round exploration. v1 (FKs+VALUES) crossed the wall
# at 47%. Add FIND (value -> which column) and PEEK (run an exploratory sub-query), 2 explore rounds. If v2
# > v1, the ratchet holds (more/better tools -> higher accuracy) and v1's 47% wasn't noise. Free verifier,
# zero salary, same held-out n=40. Baselines: one-shot 13, run/fix 16, rich-schema 15, v1 19.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit; BIRD=/tmp/yogi_bird/minidev/MINIDEV; T="${T:-3}"; NTE="${NTE:-40}"; RND="${RND:-2}"
"$PY" - "$STUDENT" "$BIRD" "$T" "$NTE" "$RND" <<'PY'
import sqlite3,sys,re,json,random
STU,BIRD,T,NTE,RND=sys.argv[1],sys.argv[2],int(sys.argv[3]),int(sys.argv[4]),int(sys.argv[5])
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
def tinfo(db):  # table -> [cols], and text columns for FIND
    c=sqlite3.connect(dbpath(db)); ts=[r[0] for r in c.execute("SELECT name FROM sqlite_master WHERE type='table'")]; cols={}; txt={}
    for t in ts:
        info=c.execute(f'PRAGMA table_info("{t}")').fetchall(); cols[t]=[r[1] for r in info]
        txt[t]=[r[1] for r in info if 'CHAR' in (r[2] or '').upper() or 'TEXT' in (r[2] or '').upper()]
    c.close(); return cols,txt
def VALUES(db,table,col):
    try:
        c=sqlite3.connect(dbpath(db)); r=[str(x[0]) for x in c.execute(f'SELECT DISTINCT "{col}" FROM "{table}" WHERE "{col}" IS NOT NULL LIMIT 5')]; c.close()
        return f"VALUES {table}.{col} = {r}"
    except Exception as e: return f"VALUES {table}.{col}: error {str(e)[:40]}"
def FIND(db,val,txt):
    hits=[]; c=sqlite3.connect(dbpath(db)); c.execute("PRAGMA query_only=ON")
    for t,cs in txt.items():
        for col in cs:
            try:
                if c.execute(f'SELECT 1 FROM "{t}" WHERE "{col}" = ? LIMIT 1',(val,)).fetchone(): hits.append(f"{t}.{col}")
            except Exception: pass
            if len(hits)>=6: break
        if len(hits)>=6: break
    c.close(); return f"FIND '{val}' -> {hits if hits else 'no exact match (try LIKE in PEEK)'}"
def PEEK(db,sql):
    if not re.match(r'(?is)^\s*select\b',sql.strip()) or re.search(r'(?i)\b(drop|delete|update|insert|attach|pragma|create|alter)\b',sql): return "PEEK: only SELECT allowed"
    try:
        c=sqlite3.connect(dbpath(db)); c.execute("PRAGMA query_only=ON"); r=c.execute(sql).fetchall(); c.close(); return f"PEEK -> {str(r[:3])[:160]}"
    except Exception as e: return f"PEEK error: {str(e)[:100]}"
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
random.seed(0); random.shuffle(items); test=items[130:130+NTE]
from mlx_lm import load,generate
m,t=load(STU)
def gen(p,mx=240): return generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":p}],add_generation_prompt=True,tokenize=False),max_tokens=mx,verbose=False)
TOOLS=("Tools (emit up to 3 per round, one per line):\n  VALUES table.column   (see example values)\n"
       "  FIND <text>           (which column holds this value)\n  PEEK <SELECT ...>     (run a small exploratory query)\n  or READY")
ok=0; probes=0
for i,q in enumerate(test):
    db=q['db_id']; cols,txt=tinfo(db); gold=key(run(db,q['SQL'])[0])
    ctx=f"SQLite schema:\n{schema(db)}\nForeign keys:\n{fks(db)}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}"
    obs=[]
    for rnd in range(RND):
        p=f"{ctx}\n"+("\n".join(obs) if obs else "")+f"\nInvestigate before writing SQL.\n{TOOLS}\nOutput only tool lines. /no_think"
        out=gen(p,160).split('</think>')[-1]
        if re.search(r'\bREADY\b',out) and obs: break
        did=0
        for line in out.splitlines():
            if did>=3: break
            mv=re.match(r'\s*VALUES\s+([A-Za-z_]\w*)\.([A-Za-z_]\w*)',line)
            mf=re.match(r'\s*FIND\s+(.+)',line); mp=re.match(r'\s*PEEK\s+(select\b.+)',line,re.I)
            if mv: obs.append(VALUES(db,mv.group(1),mv.group(2))); did+=1; probes+=1
            elif mp: obs.append(PEEK(db,mp.group(1).strip())); did+=1; probes+=1
            elif mf: obs.append(FIND(db,mf.group(1).strip().strip("'\""),txt)); did+=1; probes+=1
        if did==0: break
    base=f"{ctx}\n"+("Investigation:\n"+"\n".join(obs) if obs else "")+"\nWrite ONE SQLite SELECT query. Output only the SQL. /no_think"
    sql=exsql(gen(base)); rows,err=run(db,sql); final=sql
    for _ in range(T-1):
        if err is None and rows: break
        fb=f"ERROR: {err}" if err else f"ran OK, {len(rows)} row(s): {str(rows[:3])[:140]}"
        sql=exsql(gen(base+f"\nYour query:\n{sql}\nResult — {fb}\nIf wrong/errored, correct it (use the foreign keys + investigated values). Else repeat. Output only SQL. /no_think")); rows,err=run(db,sql); final=sql
    if key(run(db,final)[0])==gold and gold is not None: ok+=1
    if (i+1)%10==0: print(f"  {i+1}/{len(test)} done ({ok} correct, {probes} probes total)",flush=True)
n=len(test)
print(f"\n=== TOOLSPACE v2 — multi-round explore (VALUES+FIND+PEEK) + run/fix (n={n}, zero salary) ===",flush=True)
print(f"  toolspace-v2 {ok}/{n} ({100*ok//n}%)   vs  v1 19/40 (47%)  run/fix 16 (40%)  one-shot 13 (33%)",flush=True)
print(f"  avg probes/q: {probes/n:.1f}",flush=True)
print(f"  RATCHET HOLDS ✓ iff v2 > v1 (richer tools push higher) — confirms toolspace evolution + rules out noise.",flush=True)
PY
