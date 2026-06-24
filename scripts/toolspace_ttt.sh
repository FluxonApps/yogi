#!/usr/bin/env bash
# TTT-PER-DATABASE (Section 9 #1, top novel pick) — does GRADIENT adaptation beat IN-CONTEXT? For each DB,
# fine-tune a tiny throwaway LoRA on THAT DB's train solved (question->SQL) pairs (items[80:], disjoint from
# held-out [0:80]); eval held-out one-shot WITH the per-DB adapter. Bypasses the weak 8B's in-context-learning
# limit with gradient updates. Compare base one-shot 37% (isolates TTT) and the in-context best (embed-lib 53%).
# Memory-safe LoRA. Zero salary.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit; BIRD=/tmp/yogi_bird/minidev/MINIDEV; W=/tmp/yogi_ttt; A="${A:-0}"; B="${B:-80}"; ITERS="${ITERS:-80}"
mkdir -p "$W"
"$PY" - "$STUDENT" "$BIRD" "$W" "$A" "$B" "$ITERS" <<'PY'
import sqlite3,sys,re,json,random,os,subprocess
STU,BIRD,W,A,B,ITERS=sys.argv[1],sys.argv[2],sys.argv[3],int(sys.argv[4]),int(sys.argv[5]),int(sys.argv[6])
DBDIR=f"{BIRD}/dev_databases"; dbpath=lambda db:f"{DBDIR}/{db}/{db}.sqlite"
def schema(db):
    c=sqlite3.connect(dbpath(db)); ts=c.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall(); o=[]
    for (t,) in ts: o.append(f"{t}({', '.join(r[1] for r in c.execute(f'PRAGMA table_info(\"{t}\")').fetchall())})")
    c.close(); return "\n".join(o)
def run(db,sql):
    if not re.match(r'(?is)^\s*select\b',sql.strip()) or re.search(r'(?i)\b(drop|delete|update|insert|attach|pragma|create|alter)\b',sql): return None
    try:
        c=sqlite3.connect(dbpath(db)); c.execute("PRAGMA query_only=ON"); r=c.execute(sql).fetchall(); c.close(); return r
    except Exception: return None
key=lambda rows: None if rows is None else tuple(sorted(tuple(str(x) for x in r) for r in rows))
def exsql(t):
    t=t.split('</think>')[-1]; m=re.findall(r'```(?:sql)?\s*(.*?)```',t,re.S); s=(m[-1] if m else t).strip()
    m2=re.findall(r'(?is)(select\b.*?)(?:;|$)',s); return (m2[-1].strip() if m2 else s)
cold=lambda q:(f"SQLite schema:\n{schema(q['db_id'])}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}\nWrite ONE SQLite SELECT query. Output only the SQL. /no_think")
Q=json.load(open(f"{BIRD}/mini_dev_sqlite.json"))
cand=["toxicology","california_schools","debit_card_specializing","student_club","superhero","financial"]
use=[db for db in cand if len(schema(db))<2500]
items=[q for q in Q if q['db_id'] in use and q['difficulty'] in ('simple','moderate')]
random.seed(0); random.shuffle(items); test=items[A:B]; train=items[80:]
# Phase A: per-DB LoRA on that DB's train solved pairs
adapters={}
for db in use:
    tr=[q for q in train if q['db_id']==db]
    if len(tr)<6: print(f"  [{db}] only {len(tr)} train -> skip TTT",flush=True); continue
    d=f"{W}/{db}"; os.makedirs(d,exist_ok=True)
    rows=[{"prompt":cold(q),"completion":" "+q['SQL']} for q in tr]
    open(f"{d}/train.jsonl","w").write("\n".join(json.dumps(r) for r in rows)+"\n")
    open(f"{d}/valid.jsonl","w").write("\n".join(json.dumps(r) for r in rows[:max(2,len(rows)//6)])+"\n")
    ad=f"{d}/adapter"; os.makedirs(ad,exist_ok=True)
    r=subprocess.run([sys.executable,"-m","mlx_lm.lora","--model",STU,"--train","--data",d,"--batch-size","1",
      "--num-layers","8","--iters",str(ITERS),"--learning-rate","1e-4","--max-seq-length","768","--grad-checkpoint","--adapter-path",ad],capture_output=True,text=True)
    if os.path.exists(f"{ad}/adapters.safetensors"): adapters[db]=ad; print(f"  [{db}] TTT LoRA trained on {len(tr)} pairs",flush=True)
    else: print(f"  [{db}] LoRA FAILED: {(r.stderr or r.stdout)[-200:]}",flush=True)
from mlx_lm import load,generate
def evalpass(adapter_for):  # adapter_for: db->path or None
    by={}
    for q in test: by.setdefault(q['db_id'],[]).append(q)
    ok=0
    # base pass: load once; ttt pass: load per db
    if adapter_for is None:
        m,t=load(STU)
        for q in test:
            gold=key(run(q['db_id'],q['SQL']))
            if gold is None: continue
            o=generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":cold(q)}],add_generation_prompt=True,tokenize=False),max_tokens=240,verbose=False)
            if key(run(q['db_id'],exsql(o)))==gold: ok+=1
        del m,t; return ok
    for db,qs in by.items():
        ad=adapter_for.get(db)
        m,t=load(STU,adapter_path=ad) if ad else load(STU)
        for q in qs:
            gold=key(run(q['db_id'],q['SQL']))
            if gold is None: continue
            o=generate(m,t,prompt=t.apply_chat_template([{"role":"user","content":cold(q)}],add_generation_prompt=True,tokenize=False),max_tokens=240,verbose=False)
            if key(run(q['db_id'],exsql(o)))==gold: ok+=1
        del m,t
    return ok
n=len([q for q in test if key(run(q['db_id'],q['SQL'])) is not None])
print("eval base one-shot...",flush=True); base=evalpass(None)
print("eval TTT one-shot (per-DB adapters)...",flush=True); ttt=evalpass(adapters)
print(f"\n=== TTT-PER-DATABASE one-shot (items[{A}:{B}], n={n}, {len(adapters)} adapters, zero salary) ===",flush=True)
print(f"  base one-shot {base}/{n} ({100*base//n}%)   TTT one-shot {ttt}/{n} ({100*ttt//n}%)",flush=True)
print(f"  vs in-context best: embed-library 43/80 (53%), decompose 42/80 (52%), tools 39/80 (48%)",flush=True)
print(f"  TTT (GRADIENT) HELPS ✓ iff TTT >> base one-shot; BEATS in-context iff TTT > 53%.",flush=True)
PY
