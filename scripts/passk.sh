#!/usr/bin/env bash
# pass@k CURVE — does the local 8B's LATENT capability keep rising with more samples? Distinguishes the two
# boundary types: if oracle@k climbs with k, the constraint is SAMPLING+SELECTION (verifier + more samples →
# usable); if it plateaus, it's a HARD CAPABILITY bound. Generate K samples ONCE per question, compute
# oracle@{1,4,8,16} + self-consistency@K from the same samples. Zero salary, one model, eval-only.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit; BIRD=/tmp/yogi_bird/minidev/MINIDEV; K="${K:-16}"; NTE="${NTE:-40}"
"$PY" - "$STUDENT" "$BIRD" "$K" "$NTE" <<'PY'
import sqlite3,sys,re,json,random,collections
STU,BIRD,K,NTE=sys.argv[1],sys.argv[2],int(sys.argv[3]),int(sys.argv[4])
DBDIR=f"{BIRD}/dev_databases"; dbpath=lambda db:f"{DBDIR}/{db}/{db}.sqlite"
def compact_schema(db):
    c=sqlite3.connect(dbpath(db)); ts=c.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall(); o=[]
    for (t,) in ts: o.append(f"{t}({', '.join(r[1] for r in c.execute(f'PRAGMA table_info(\"{t}\")').fetchall())})")
    c.close(); return "\n".join(o)
Q=json.load(open(f"{BIRD}/mini_dev_sqlite.json"))
cand=["toxicology","california_schools","debit_card_specializing","student_club","superhero","financial"]
use=[db for db in cand if len(compact_schema(db))<2500]
items=[q for q in Q if q['db_id'] in use and q['difficulty'] in ('simple','moderate')]
random.seed(0); random.shuffle(items); test=items[130:130+NTE]  # SAME held-out as scale + best-of-N
def run_sql(db,sql):
    if not re.match(r'(?is)^\s*select\b',sql.strip()) or re.search(r'(?i)\b(drop|delete|update|insert|attach|pragma|create|alter)\b',sql): return None
    try:
        c=sqlite3.connect(dbpath(db)); c.execute("PRAGMA query_only=ON"); r=c.execute(sql).fetchall(); c.close(); return r
    except Exception: return None
key=lambda rows: None if rows is None else tuple(sorted(tuple(str(x) for x in r) for r in rows))
def exsql(t):
    t=t.split('</think>')[-1]; m=re.findall(r'```(?:sql)?\s*(.*?)```',t,re.S); s=(m[-1] if m else t).strip()
    m2=re.findall(r'(?is)(select\b.*?)(?:;|$)',s); return (m2[-1].strip() if m2 else s)
def prompt(q): return (f"SQLite schema:\n{compact_schema(q['db_id'])}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}\nWrite ONE SQLite SELECT query. Output only the SQL. /no_think")
from mlx_lm import load,generate
from mlx_lm.sample_utils import make_sampler
samp=make_sampler(temp=0.8); m,t=load(STU)
def gen(q): return generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":prompt(q)}],add_generation_prompt=True,tokenize=False),max_tokens=200,verbose=False,sampler=samp)
KS=[k for k in (1,4,8,16,32) if k<=K]
orc={k:0 for k in KS}; sc=0; n=len(test)
for i,q in enumerate(test):
    gold=key(run_sql(q['db_id'],q['SQL']))
    samples=[key(run_sql(q['db_id'],exsql(gen(q)))) for _ in range(K)]
    for k in KS:
        if gold is not None and any(s==gold for s in samples[:k]): orc[k]+=1
    nn=[s for s in samples if s is not None]
    if nn and collections.Counter(nn).most_common(1)[0][0]==gold and gold is not None: sc+=1
    if (i+1)%10==0: print(f"  {i+1}/{n} done",flush=True)
print(f"\n=== pass@k CURVE (held-out n={n}, K={K} samples/q, zero salary) ===",flush=True)
print("  oracle@k: "+"  ".join(f"@{k}={orc[k]}/{n}({100*orc[k]//n}%)" for k in KS),flush=True)
print(f"  self-consistency@{K}: {sc}/{n}",flush=True)
print("  INTERPRET: oracle@k RISING with k → sampling+selection bound (verifier usable); PLATEAU → hard capability bound.",flush=True)
PY
