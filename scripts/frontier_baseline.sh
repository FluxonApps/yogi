#!/usr/bin/env bash
# Frontier baseline on the SAME held-out n=40 (items[130:170]) — replaces the report's ~78% estimate with a
# measured number, execution-verified identically to every local-model row. claude -p one-shot SQL.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
PY=.venv-mlx/bin/python; BIRD=/tmp/yogi_bird/minidev/MINIDEV; NTE="${NTE:-40}"
"$PY" - "$BIRD" "$NTE" <<'PY'
import sqlite3,sys,re,json,subprocess,random
BIRD,NTE=sys.argv[1],int(sys.argv[2])
DBDIR=f"{BIRD}/dev_databases"; dbpath=lambda db:f"{DBDIR}/{db}/{db}.sqlite"
def cs(db):
    c=sqlite3.connect(dbpath(db)); ts=c.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall(); o=[]
    for (t,) in ts: o.append(f"{t}({', '.join(r[1] for r in c.execute(f'PRAGMA table_info(\"{t}\")').fetchall())})")
    c.close(); return "\n".join(o)
Q=json.load(open(f"{BIRD}/mini_dev_sqlite.json"))
cand=["toxicology","california_schools","debit_card_specializing","student_club","superhero","financial"]
use=[db for db in cand if len(cs(db))<2500]
items=[q for q in Q if q['db_id'] in use and q['difficulty'] in ('simple','moderate')]
random.seed(0); random.shuffle(items); test=items[130:130+NTE]
def run(db,sql):
    if not re.match(r'(?is)^\s*select\b',sql.strip()) or re.search(r'(?i)\b(drop|delete|update|insert|attach|pragma|create|alter)\b',sql): return None
    try:
        c=sqlite3.connect(dbpath(db)); c.execute("PRAGMA query_only=ON"); r=c.execute(sql).fetchall(); c.close(); return r
    except Exception: return None
key=lambda rows: None if rows is None else tuple(sorted(tuple(str(x) for x in r) for r in rows))
def exsql(t):
    m=re.findall(r'```(?:sql)?\s*(.*?)```',t,re.S); s=(m[-1] if m else t).strip()
    m2=re.findall(r'(?is)(select\b.*?)(?:;|$)',s); return (m2[-1].strip() if m2 else s)
def frontier(q):
    p=(f"SQLite schema:\n{cs(q['db_id'])}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}\nWrite ONE correct SQLite SELECT. Output only the query in a ```sql ...``` block.")
    try: return subprocess.run(["claude","-p",p],capture_output=True,text=True,timeout=150).stdout
    except Exception: return ""
ok=0
for i,q in enumerate(test):
    if key(run(q['db_id'],exsql(frontier(q))))==key(run(q['db_id'],q['SQL'])) and key(run(q['db_id'],q['SQL'])) is not None: ok+=1
    if (i+1)%10==0: print(f"  {i+1}/{len(test)} done -> {ok} correct",flush=True)
print(f"\n=== FRONTIER BASELINE (Claude, one-shot, held-out n={len(test)}, execution-verified) ===",flush=True)
print(f"  frontier {ok}/{len(test)} ({100*ok//len(test)}%)   vs local 8B one-shot 13/40 (33%) / +tool 16/40 (40%)",flush=True)
PY
