#!/usr/bin/env bash
# DECISIVE CEILING TEST — oracle headroom of the BEST agent (embedding-retrieved few-shot + tools). Sample it
# R times per question; report single (1st), self-consistency (majority executed result), and ORACLE (any
# sample correct). If ORACLE >> 53% a selector/search/ensemble/verifier could still push the ceiling; if
# ORACLE ~= 53% the BIRD ceiling is GENERATION-bound and inference-time ceiling-pushing is exhausted.
# items[0:40], R rollouts, nomic retrieval, zero salary.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit; BIRD=/tmp/yogi_bird/minidev/MINIDEV; T="${T:-3}"; K="${K:-3}"; R="${R:-4}"; A="${A:-0}"; B="${B:-40}"
"$PY" - "$STUDENT" "$BIRD" "$T" "$K" "$R" "$A" "$B" <<'PY'
import sqlite3,sys,re,json,random,urllib.request,collections,numpy as np
STU,BIRD,T,K,R,A,B=sys.argv[1],sys.argv[2],int(sys.argv[3]),int(sys.argv[4]),int(sys.argv[5]),int(sys.argv[6]),int(sys.argv[7])
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
    if not re.match(r'(?is)^\s*select\b',sql.strip()) or re.search(r'(?i)\b(drop|delete|update|insert|attach|pragma|create|alter)\b',sql): return (None,1)
    try:
        c=sqlite3.connect(dbpath(db)); c.execute("PRAGMA query_only=ON"); r=c.execute(sql).fetchall(); c.close(); return (r,0)
    except Exception: return (None,1)
key=lambda rows: None if rows is None else tuple(sorted(tuple(str(x) for x in r) for r in rows))
def exsql(t):
    t=t.split('</think>')[-1]; m=re.findall(r'```(?:sql)?\s*(.*?)```',t,re.S); s=(m[-1] if m else t).strip()
    m2=re.findall(r'(?is)(select\b.*?)(?:;|$)',s); return (m2[-1].strip() if m2 else s)
Q=json.load(open(f"{BIRD}/mini_dev_sqlite.json"))
cand=["toxicology","california_schools","debit_card_specializing","student_club","superhero","financial"]
use=[db for db in cand if len(schema(db))<2500]
items=[q for q in Q if q['db_id'] in use and q['difficulty'] in ('simple','moderate')]
random.seed(0); random.shuffle(items); test=items[A:B]; lib=items[80:]
def embed(txt):
    req=urllib.request.Request("http://localhost:11434/api/embeddings",data=json.dumps({"model":"nomic-embed-text","prompt":txt}).encode(),headers={"Content-Type":"application/json"})
    return np.array(json.loads(urllib.request.urlopen(req,timeout=30).read())["embedding"],dtype=np.float32)
print("embedding...",flush=True)
for x in lib+test: x["_e"]=embed(x["question"])
def retrieve(q,k):
    pool=[x for x in lib if x['db_id']==q['db_id']] or lib; qn=q["_e"]/(np.linalg.norm(q["_e"])+1e-8)
    return sorted(pool,key=lambda x: float(qn @ (x["_e"]/(np.linalg.norm(x["_e"])+1e-8))),reverse=True)[:k]
from mlx_lm import load,generate
from mlx_lm.sample_utils import make_sampler
samp=make_sampler(temp=0.7); m,t=load(STU)
def gen(p,mx=240,s=False):
    kw={"sampler":samp} if s else {}
    return generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":p}],add_generation_prompt=True,tokenize=False),max_tokens=mx,verbose=False,**kw)
def agent(q,s):
    db=q['db_id']; ex="\n".join(f"Q: {e['question']}\nSQL: {e['SQL']}" for e in retrieve(q,K))
    ctx=f"SQLite schema:\n{schema(db)}\nForeign keys:\n{fks(db)}\nSimilar solved examples:\n{ex}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}"
    exq=gen(ctx+"\nList up to 4 columns whose example values you want:\nVALUES table.column\nOnly those lines. /no_think",150,s).split('</think>')[-1]
    obs=[]
    for line in exq.splitlines():
        mm=re.match(r'\s*VALUES\s+([A-Za-z_]\w*)\.([A-Za-z_]\w*)',line)
        if mm and len(obs)<4: obs.append(VALUES(db,mm.group(1),mm.group(2)))
    base=ctx+(("\nInvestigated:\n"+"\n".join(obs)) if obs else "")+"\nWrite ONE SQLite SELECT query (adapt the example patterns). Output only the SQL. /no_think"
    sql=exsql(gen(base,240,s)); rows,err=run(db,sql); final=sql
    for _ in range(T-1):
        if err==0 and rows: break
        fb=f"ERROR" if err else f"ran OK {len(rows)} rows"
        sql=exsql(gen(base+f"\nYour query:\n{sql}\nResult — {fb}. If wrong/errored correct it. Else repeat. Only SQL. /no_think",240,s)); rows,err=run(db,sql); final=sql
    return key(run(db,final)[0])
single=0; sc=0; orc=0
for i,q in enumerate(test):
    gold=key(run(q['db_id'],q['SQL'])[0])
    if gold is None: continue
    res=[agent(q, r>0) for r in range(R)]
    if res[0]==gold: single+=1
    if gold is not None and any(x==gold for x in res): orc+=1
    nn=[x for x in res if x is not None]
    if nn and collections.Counter(nn).most_common(1)[0][0]==gold: sc+=1
    if (i+1)%10==0: print(f"  {i+1}/{len(test)}: single {single} sc {sc} oracle {orc}",flush=True)
n=len(test)
print(f"\n=== ORACLE HEADROOM of best agent (embed+tools, R={R}, items[{A}:{B}], n={n}) ===",flush=True)
print(f"  single {single}/{n} ({100*single//n}%)  self-consistency {sc}/{n} ({100*sc//n}%)  ORACLE(any-of-{R}) {orc}/{n} ({100*orc//n}%)",flush=True)
print(f"  HEADROOM EXISTS iff ORACLE >> ~53% (a selector/verifier/search could push it); else GENERATION-bound (ceiling reached).",flush=True)
PY
