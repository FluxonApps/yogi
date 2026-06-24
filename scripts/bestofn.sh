#!/usr/bin/env bash
# BEST-OF-N + EXECUTION SELF-CONSISTENCY (different lever: runtime compute, not distillation). The
# distillation arms are robustly flat; this tests if the local 8B is USABLE on real BIRD via test-time
# compute (CogniSQL: best-of-6 = +9.7%). Gold-free selector: sample N SQLs, execute each, pick the
# MAJORITY result-set; correct iff majority result == gold result. one-shot vs best-of-N. Zero salary.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit; BIRD=/tmp/yogi_bird/minidev/MINIDEV; N="${N:-8}"; NTE="${NTE:-40}"
"$PY" - "$STUDENT" "$BIRD" "$N" "$NTE" <<'PY'
import sqlite3,sys,re,json,random,collections
STU,BIRD,N,NTE=sys.argv[1],sys.argv[2],int(sys.argv[3]),int(sys.argv[4])
DBDIR=f"{BIRD}/dev_databases"; dbpath=lambda db:f"{DBDIR}/{db}/{db}.sqlite"
def compact_schema(db):
    c=sqlite3.connect(dbpath(db)); ts=c.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall(); o=[]
    for (t,) in ts: o.append(f"{t}({', '.join(r[1] for r in c.execute(f'PRAGMA table_info(\"{t}\")').fetchall())})")
    c.close(); return "\n".join(o)
Q=json.load(open(f"{BIRD}/mini_dev_sqlite.json"))
cand=["toxicology","california_schools","debit_card_specializing","student_club","superhero","financial"]
use=[db for db in cand if len(compact_schema(db))<2500]
items=[q for q in Q if q['db_id'] in use and q['difficulty'] in ('simple','moderate')]
random.seed(0); random.shuffle(items); test=items[130:130+NTE]  # SAME held-out as the scale test
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
samp=make_sampler(temp=0.8)
m,t=load(STU)
def gen(q,sample): 
    kw={"sampler":samp} if sample else {}
    return generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":prompt(q)}],add_generation_prompt=True,tokenize=False),max_tokens=200,verbose=False,**kw)
one=0; bon=0; orc=0
for q in test:
    gold=key(run_sql(q['db_id'],q['SQL']))
    # one-shot greedy
    if key(run_sql(q['db_id'],exsql(gen(q,False))))==gold and gold is not None: one+=1
    # best-of-N self-consistency: N samples -> majority NON-NULL executed result-set
    res=[key(run_sql(q['db_id'],exsql(gen(q,True)))) for _ in range(N)]
    if gold is not None and any(r==gold for r in res): orc+=1   # ORACLE: any sample correct (latent capability)
    res=[r for r in res if r is not None]
    if res:
        maj=collections.Counter(res).most_common(1)[0][0]
        if maj==gold and gold is not None: bon+=1   # self-consistency (gold-free majority)
print(f"\n=== BEST-OF-{N} SELF-CONSISTENCY (held-out n={len(test)}, runtime compute, zero salary) ===",flush=True)
print(f"  one-shot {one}/{len(test)}  ->  best-of-{N} {bon}/{len(test)}",flush=True)
print(f"  USABLE-VIA-TEST-TIME-COMPUTE ✓ iff best-of-N >> one-shot (the 8B's latent capability surfaced).",flush=True)
PY
