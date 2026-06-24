#!/usr/bin/env bash
# RE-VALIDATION (TODO #2): retrieved-example library with EMBEDDING retrieval (nomic-embed via ollama, the
# named shared embedding). The lexical-retrieval version was flat (48%); a stronger retriever may pull more
# relevant solved examples. Few-shot top-K similar (question, SQL) into the interactive tool-agent. Concrete
# templates, not an abstraction layer. items[0:80] test, lib = items[80:]. Compare base 48% / decompose 52% /
# lexical-library 48%. nomic (0.3GB) coexists with qwen per CLAUDE.md. Zero salary.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit; BIRD=/tmp/yogi_bird/minidev/MINIDEV; T="${T:-3}"; K="${K:-3}"; A="${A:-0}"; B="${B:-80}"
"$PY" - "$STUDENT" "$BIRD" "$T" "$K" "$A" "$B" <<'PY'
import sqlite3,sys,re,json,random,urllib.request,numpy as np
STU,BIRD,T,K,A,B=sys.argv[1],sys.argv[2],int(sys.argv[3]),int(sys.argv[4]),int(sys.argv[5]),int(sys.argv[6])
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
random.seed(0); random.shuffle(items); test=items[A:B]; lib=items[80:]
def embed(txt):
    req=urllib.request.Request("http://localhost:11434/api/embeddings",
        data=json.dumps({"model":"nomic-embed-text","prompt":txt}).encode(),headers={"Content-Type":"application/json"})
    return np.array(json.loads(urllib.request.urlopen(req,timeout=30).read())["embedding"],dtype=np.float32)
print("embedding library + test questions via nomic-embed...",flush=True)
for x in lib+test: x["_emb"]=embed(x["question"])
def retrieve(q,k):
    pool=[x for x in lib if x['db_id']==q['db_id']] or lib
    qe=q["_emb"]; qn=qe/ (np.linalg.norm(qe)+1e-8)
    scored=sorted(pool,key=lambda x: float(qn @ (x["_emb"]/(np.linalg.norm(x["_emb"])+1e-8))),reverse=True)
    return scored[:k]
print("library embedded; loading qwen...",flush=True)
from mlx_lm import load,generate
m,t=load(STU)
def gen(p,mx=240): return generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":p}],add_generation_prompt=True,tokenize=False),max_tokens=mx,verbose=False)
def agent(q):
    db=q['db_id']
    ex="\n".join(f"Q: {e['question']}\nSQL: {e['SQL']}" for e in retrieve(q,K))
    ctx=f"SQLite schema:\n{schema(db)}\nForeign keys:\n{fks(db)}\nSimilar solved examples:\n{ex}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}"
    exq=gen(ctx+"\nList up to 4 columns whose example values you want:\nVALUES table.column\nOnly those lines. /no_think",150).split('</think>')[-1]
    obs=[]
    for line in exq.splitlines():
        mm=re.match(r'\s*VALUES\s+([A-Za-z_]\w*)\.([A-Za-z_]\w*)',line)
        if mm and len(obs)<4: obs.append(VALUES(db,mm.group(1),mm.group(2)))
    base=ctx+(("\nInvestigated:\n"+"\n".join(obs)) if obs else "")+"\nWrite ONE SQLite SELECT query (adapt the patterns from the similar examples). Output only the SQL. /no_think"
    sql=exsql(gen(base)); rows,err=run(db,sql); final=sql
    for _ in range(T-1):
        if err is None and rows: break
        fb=f"ERROR: {err}" if err else f"ran OK, {len(rows)} rows: {str(rows[:3])[:120]}"
        sql=exsql(gen(base+f"\nYour query:\n{sql}\nResult — {fb}\nIf wrong/errored correct it (FKs, values, example patterns). Else repeat. Only SQL. /no_think")); rows,err=run(db,sql); final=sql
    return key(run(db,final)[0])
ok=0
for i,q in enumerate(test):
    gold=key(run(q['db_id'],q['SQL'])[0])
    if gold is None: continue
    if agent(q)==gold: ok+=1
    if (i+1)%20==0: print(f"  {i+1}/{len(test)}: embed-library {ok}",flush=True)
n=len(test)
print(f"\n=== EMBEDDING LIBRARY + TOOLS (nomic-embed retrieval, K={K}, items[{A}:{B}], n={n}, zero salary) ===",flush=True)
print(f"  embed-library {ok}/{n} ({100*ok//n}%)   vs base 39/80(48%) decompose 42/80(52%) lexical-lib 39/80(48%) one-shot 30/80(37%)",flush=True)
print(f"  RETRIEVAL HELPS ✓ iff > 48% — better retrieval beats the lexical null (else: in-context examples arent the lever).",flush=True)
PY
