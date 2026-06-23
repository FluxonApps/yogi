#!/usr/bin/env python3
"""Speedup benchmark + helpers for the local MLX harness (research-grounded).

The bottleneck (BIRD/SQL): every question re-processes the SAME long schema prompt. Techniques measured:
  A  naive        — full templated prompt (schema DDL + question), recomputed per question
  B  compact      — compact schema string (table(cols)) instead of verbose CREATE DDL → shorter prompts
  C  prefix-cache — process the shared schema prefix ONCE, reuse the KV cache across the DB's questions
                    (mlx-lm make_prompt_cache; research: ~40x on long shared contexts)
Run when the GPU is free (one model at a time). Reports wall-time + tok/s per technique on real BIRD
questions so we ADOPT what's measured, not assumed. Defensive: if a cache API differs, that technique is
skipped with a note (never crashes the bench).

Usage: .venv-mlx/bin/python scripts/mlx_fast.py            # benchmark
Exposes cached_generate() for the harness to import once a technique is validated.
"""
import sys, time, json, sqlite3, copy, os
os.environ.setdefault("HF_HUB_DISABLE_PROGRESS_BARS", "1")
from mlx_lm import load, generate

STUDENT = "mlx-community/Qwen3-8B-4bit"
BIRD = "/tmp/yogi_bird/minidev/MINIDEV"
NT = " /no_think"

def compact_schema(dbpath):
    c = sqlite3.connect(dbpath)
    rows = c.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall()
    out = []
    for (t,) in rows:
        cols = [r[1] for r in c.execute(f"PRAGMA table_info('{t}')").fetchall()]
        out.append(f"{t}({', '.join(cols)})")
    c.close()
    return "\n".join(out)

def ddl_schema(dbpath):
    c = sqlite3.connect(dbpath)
    rows = c.execute("SELECT sql FROM sqlite_master WHERE type='table' AND sql IS NOT NULL").fetchall()
    c.close()
    return "\n".join(r[0] for r in rows)

def _prompt(tok, schema, q):
    txt = (f"SQLite schema:\n{schema}\n-- Hint: {q['evidence']}\nQuestion: {q['question']}\n"
           f"Write ONE SQLite SELECT query. Output only the SQL.{NT}")
    return tok.apply_chat_template([{"role": "user", "content": txt}], add_generation_prompt=True, tokenize=False)

def cached_generate(model, tok, primed_cache, full_prompt, max_tokens=200):
    """Generate reusing a primed prefix cache (deepcopy so the prefix is reused, not consumed)."""
    c = copy.deepcopy(primed_cache)
    return generate(model, tok, prompt=full_prompt, max_tokens=max_tokens, prompt_cache=c, verbose=False)

def bench():
    Q = json.load(open(f"{BIRD}/mini_dev_sqlite.json"))
    db = "superhero"  # small schema, several questions
    qs = [q for q in Q if q["db_id"] == db][:6]
    dbpath = f"{BIRD}/dev_databases/{db}/{db}.sqlite"
    ddl = ddl_schema(dbpath); comp = compact_schema(dbpath)
    print(f"db={db}  ddl_chars={len(ddl)}  compact_chars={len(comp)}  questions={len(qs)}", flush=True)
    print("loading model (once)...", flush=True)
    model, tok = load(STUDENT)

    def timeit(label, fn):
        t0 = time.time(); ntok = fn(); dt = time.time() - t0
        print(f"  {label:16s} {dt:6.1f}s   {ntok/dt:6.1f} tok/s   ({ntok} tok over {len(qs)} q)", flush=True)
        return dt

    # A naive: full DDL prompt per question
    def A():
        n = 0
        for q in qs:
            out = generate(model, tok, prompt=_prompt(tok, ddl, q), max_tokens=120, verbose=False)
            n += len(out.split())
        return n
    # B compact schema
    def B():
        n = 0
        for q in qs:
            out = generate(model, tok, prompt=_prompt(tok, comp, q), max_tokens=120, verbose=False)
            n += len(out.split())
        return n
    # C prefix-cache the (compact) schema-bearing prefix, reuse across questions
    def C():
        try:
            from mlx_lm.models.cache import make_prompt_cache
        except Exception as e:
            print(f"  prefix-cache     SKIPPED (API: {e!r})", flush=True); return 0
        # shared prefix = templated text up to the per-question part is hard with chat templates;
        # instead prime a cache on the compact-schema preamble (non-chat) and reuse via deepcopy.
        prefix = f"SQLite schema:\n{comp}\n"
        cache = make_prompt_cache(model)
        try:
            generate(model, tok, prompt=prefix, max_tokens=1, prompt_cache=cache, verbose=False)  # prime
        except Exception as e:
            print(f"  prefix-cache     SKIPPED (prime: {e!r})", flush=True); return 0
        n = 0
        for q in qs:
            suffix = (f"-- Hint: {q['evidence']}\nQuestion: {q['question']}\n"
                      f"Write ONE SQLite SELECT query. Output only the SQL.{NT}")
            try:
                out = cached_generate(model, tok, cache, suffix, max_tokens=120)
            except Exception as e:
                print(f"  prefix-cache     SKIPPED (gen: {e!r})", flush=True); return 0
            n += len(out.split())
        return n

    print("=== SPEEDUP BENCHMARK (6 BIRD questions, shared schema) ===", flush=True)
    da = timeit("A naive-DDL", A)
    db_ = timeit("B compact", B)
    dc = timeit("C prefix-cache", C)
    print("=== verdict ===", flush=True)
    if db_ > 0: print(f"  compact vs naive:      {da/db_:.2f}x", flush=True)
    if dc > 0:  print(f"  prefix-cache vs naive: {da/dc:.2f}x", flush=True)
    print("  adopt the fastest correct technique into the SQL harness.", flush=True)

if __name__ == "__main__":
    bench()
