#!/usr/bin/env bash
# CEILING-ATTACK (research-grounded): the oracle=single result means TEMPERATURE diversity fails (samples stay
# in ONE solution mode). Force STRUCTURAL diversity instead (TypedThinker / Diverse-Beam idea): K candidates
# each in a DIFFERENT SQL form (subquery / explicit-JOIN / CTE / step-by-step). Then (1) typed-ORACLE: does
# structural diversity expand the oracle past the temp-bound 53%? and (2) gold-free SELECTION by majority
# result ACROSS TYPES (diverse derivations agreeing = strong signal; unlike temp self-consistency which was
# flat). If typed-oracle >> 53% AND typed-self-consistency captures it, the generation-bound ceiling breaks.
# items[0:40], FKs+VALUES context + run/fix per candidate. Zero salary.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit; BIRD=/tmp/yogi_bird/minidev/MINIDEV; T="${T:-2}"; A="${A:-0}"; B="${B:-40}"
"$PY" - "$STUDENT" "$BIRD" "$T" "$A" "$B" <<'PY'
import sqlite3,sys,re,json,random,collections
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
def run(db,sql):
    if not re.match(r'(?is)^\s*select\b',sql.strip()) or re.search(r'(?i)\b(drop|delete|update|insert|attach|pragma|create|alter)\b',sql): return (None,1)
    try:
        c=sqlite3.connect(f"file:{dbpath(db)}?mode=ro",uri=True); c.execute("PRAGMA query_only=ON"); r=c.execute(sql).fetchall(); c.close(); return (r,0)
    except Exception: return (None,1)
key=lambda rows: None if rows is None else tuple(sorted(tuple(str(x) for x in r) for r in rows))
def exsql(t):
    t=t.split('</think>')[-1]; m=re.findall(r'```(?:sql)?\s*(.*?)```',t,re.S); s=(m[-1] if m else t).strip()
    m2=re.findall(r'(?is)(select\b.*?)(?:;|$)',s); return (m2[-1].strip() if m2 else s)
Q=json.load(open(f"{BIRD}/mini_dev_sqlite.json"))
cand=["toxicology","california_schools","debit_card_specializing","student_club","superhero","financial"]
use=[db for db in cand if len(schema(db))<2500]
items=[q for q in Q if q['db_id'] in use and q['difficulty'] in ('simple','moderate')]
random.seed(0); random.shuffle(items); test=items[A:B]
TYPES=["using one or more SUBQUERIES (no explicit JOIN keyword)",
       "using explicit JOIN clauses (foreign keys above)",
       "using a WITH (CTE) clause first, then the final SELECT",
       "by reasoning step by step, then the final SELECT"]
from mlx_lm import load,generate
m,t=load(STU)
def gen(p,mx=260): return generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":p}],add_generation_prompt=True,tokenize=False),max_tokens=mx,verbose=False)
def candidate(q,typ):
    db=q['db_id']; ctx=f"SQLite schema:\n{schema(db)}\nForeign keys:\n{fks(db)}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}"
    base=ctx+f"\nWrite ONE SQLite SELECT query, solving it {typ}. Output only the SQL. /no_think"
    sql=exsql(gen(base)); rows,err=run(db,sql); final=sql
    for _ in range(T-1):
        if err==0 and rows: break
        sql=exsql(gen(base+f"\nYour query:\n{sql}\nResult — {'ERROR' if err else 'ran '+str(len(rows))+' rows'}. If wrong/errored correct it ({typ}). Else repeat. Only SQL. /no_think")); rows,err=run(db,sql); final=sql
    return key(run(db,final)[0])
single=0; t_oracle=0; t_sc=0
for i,q in enumerate(test):
    gold=key(run(q['db_id'],q['SQL'])[0])
    if gold is None: continue
    res=[candidate(q,ty) for ty in TYPES]
    if res[0]==gold: single+=1
    if any(x==gold for x in res): t_oracle+=1
    nn=[x for x in res if x is not None]
    if nn and collections.Counter(nn).most_common(1)[0][0]==gold: t_sc+=1
    if (i+1)%10==0: print(f"  {i+1}/{len(test)}: single {single} typed-oracle {t_oracle} typed-SC {t_sc}",flush=True)
n=len(test)
print(f"\n=== TYPED STRUCTURAL DIVERSITY (K={len(TYPES)} forms, items[{A}:{B}], n={n}, zero salary) ===",flush=True)
print(f"  single(type0) {single}/{n} ({100*single//n}%)  typed-ORACLE {t_oracle}/{n} ({100*t_oracle//n}%)  typed-self-consistency {t_sc}/{n} ({100*t_sc//n}%)",flush=True)
print(f"  vs best so far ~53%. DIVERSITY-EXPANDS-ORACLE iff typed-oracle >> 53 (structural beats temp's oracle=single); CEILING-BROKEN iff typed-SC > 53.",flush=True)
PY
