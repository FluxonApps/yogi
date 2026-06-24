#!/usr/bin/env bash
# SELF-CONSISTENCY over the TOOL-AGENT — do the two levers compose? Voting was flat over one-shot (33->33,
# correct wasn't the majority). Over the confirmed 48% tool-agent (FKs+VALUES+run/fix), the correct answer is
# more frequent, so majority-of-R executed results may push past 48%. R rollouts/q; report single-run (1st
# rollout) vs majority. items[0:40], zero salary.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit; BIRD=/tmp/yogi_bird/minidev/MINIDEV; T="${T:-3}"; R="${R:-3}"; A="${A:-0}"; B="${B:-40}"
"$PY" - "$STUDENT" "$BIRD" "$T" "$R" "$A" "$B" <<'PY'
import sqlite3,sys,re,json,random,collections
STU,BIRD,T,R,A,B=sys.argv[1],sys.argv[2],int(sys.argv[3]),int(sys.argv[4]),int(sys.argv[5]),int(sys.argv[6])
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
from mlx_lm.sample_utils import make_sampler
samp=make_sampler(temp=0.7); m,t=load(STU)
def gen(p,mx=240,s=False):
    kw={"sampler":samp} if s else {}
    return generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":p}],add_generation_prompt=True,tokenize=False),max_tokens=mx,verbose=False,**kw)
def agent(q,s):
    db=q['db_id']; ctx=f"SQLite schema:\n{schema(db)}\nForeign keys:\n{fks(db)}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}"
    ex=gen(ctx+"\nBefore writing SQL, list up to 4 columns whose values you want:\nVALUES table.column\nOnly those lines. /no_think",150,s).split('</think>')[-1]
    obs=[]
    for line in ex.splitlines():
        mm=re.match(r'\s*VALUES\s+([A-Za-z_]\w*)\.([A-Za-z_]\w*)',line)
        if mm and len(obs)<4: obs.append(VALUES(db,mm.group(1),mm.group(2)))
    base=ctx+(("\nInvestigated:\n"+"\n".join(obs)) if obs else "")+"\nWrite ONE SQLite SELECT query. Output only the SQL. /no_think"
    sql=exsql(gen(base,240,s)); rows,err=run(db,sql); final=sql
    for _ in range(T-1):
        if err is None and rows: break
        fb=f"ERROR: {err}" if err else f"ran OK, {len(rows)} rows: {str(rows[:3])[:120]}"
        sql=exsql(gen(base+f"\nYour query:\n{sql}\nResult — {fb}\nIf wrong/errored correct it (FKs+values). Else repeat. Only SQL. /no_think",240,s)); rows,err=run(db,sql); final=sql
    return key(run(db,final)[0])
single=0; scc=0
for i,q in enumerate(test):
    gold=key(run(q['db_id'],q['SQL'])[0])
    if gold is None: continue
    res=[agent(q, r>0) for r in range(R)]   # rollout 0 greedy = single baseline; rest sampled
    if res[0]==gold: single+=1
    nn=[x for x in res if x is not None]
    if nn and collections.Counter(nn).most_common(1)[0][0]==gold: scc+=1
    if (i+1)%10==0: print(f"  {i+1}/{len(test)}: single {single}, SC {scc}",flush=True)
n=len(test)
print(f"\n=== SELF-CONSISTENCY over TOOL-AGENT (items[{A}:{B}], n={n}, R={R} rollouts, zero salary) ===",flush=True)
print(f"  tool-agent single  {single}/{n} ({100*single//n}%)",flush=True)
print(f"  tool-agent + SC@{R}  {scc}/{n} ({100*scc//n}%)",flush=True)
print(f"  LEVERS COMPOSE ✓ iff SC >> single — voting over the tool-agent pushes past the ~48% single-run ceiling.",flush=True)
PY
