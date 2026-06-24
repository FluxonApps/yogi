"""Yogi tasks — concrete Task implementations proving the harness is GENERIC beyond SQL.

- BIRDTask  : text-to-SQL, verifier = EXECUTION accuracy (run candidate vs gold on the real DB).
- CodeTask  : Python from a spec, verifier = UNIT TESTS (run candidate against asserts in a sandboxed subprocess).

Both implement the SAME Task protocol and run through the SAME evaluate() runner. Adding a third verifiable
task (math/answer-check, regex/extraction, JSON-schema, etc.) is just another small class.

Self-test (CPU only, no model): `python scripts/yogi_tasks.py` verifies that each task's GOLD answers pass
their own verifier and the interface is uniform — i.e. the harness handles non-SQL tasks.
"""
import os, re, json, random, sqlite3, subprocess, sys, tempfile
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from yogi_harness import Task


# ----------------------------------------------------------------- BIRD text-to-SQL (execution verifier)
class BIRDTask(Task):
    id = "bird-sql"
    CAND = ["toxicology", "california_schools", "debit_card_specializing", "student_club", "superhero", "financial"]
    def __init__(self, bird="/tmp/yogi_bird/minidev/MINIDEV"):
        self.bird = bird
    def _dbp(self, db): return f"{self.bird}/dev_databases/{db}/{db}.sqlite"
    def _schema(self, db):
        c = sqlite3.connect(self._dbp(db)); ts = c.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall(); o = []
        for (t,) in ts: o.append(f"{t}({', '.join(r[1] for r in c.execute(f'PRAGMA table_info(\"{t}\")').fetchall())})")
        c.close(); return "\n".join(o)
    def _run(self, db, sql):
        if not re.match(r'(?is)^\s*select\b', sql.strip()) or re.search(r'(?i)\b(drop|delete|update|insert|attach|pragma|create|alter)\b', sql): return None
        try:
            c = sqlite3.connect(f"file:{self._dbp(db)}?mode=ro", uri=True); c.execute("PRAGMA query_only=ON")
            r = c.execute(sql).fetchall(); c.close(); return r
        except Exception: return None
    def _key(self, rows): return None if rows is None else tuple(sorted(tuple(str(x) for x in r) for r in rows))
    def examples(self):
        Q = json.load(open(f"{self.bird}/mini_dev_sqlite.json"))
        use = [db for db in self.CAND if len(self._schema(db)) < 2500]
        return [q for q in Q if q['db_id'] in use and q['difficulty'] in ('simple', 'moderate')]
    def split(self, seed=0):
        ex = self.examples(); random.Random(seed).shuffle(ex); return ex[80:], ex[:80]   # train, held-out (n=80)
    def context(self, ex):
        return f"SQLite schema:\n{self._schema(ex['db_id'])}\n-- Hint: {ex['evidence']}\nQuestion: {ex['question']}"
    def instruction(self): return "Write ONE SQLite SELECT query. Output only the SQL. /no_think"
    def extract(self, raw):
        raw = raw.split('</think>')[-1]; m = re.findall(r'```(?:sql)?\s*(.*?)```', raw, re.S); s = (m[-1] if m else raw).strip()
        m2 = re.findall(r'(?is)(select\b.*?)(?:;|$)', s); return (m2[-1].strip() if m2 else s)
    def verify(self, pred, ex):
        g = self._key(self._run(ex['db_id'], ex['SQL'])); p = self._key(self._run(ex['db_id'], pred)); return g is not None and g == p
    def gold(self, ex): return ex['SQL']


# ----------------------------------------------------------------- Python code (unit-test verifier)
def _run_pytests(code, tests, timeout=8):
    """Sandbox-ish: run candidate code + asserts in a separate subprocess with a timeout. SELECT-only-style
    safety isn't applicable to arbitrary code, so this is the place a real deployment uses a hardened sandbox."""
    src = code + "\n" + tests + "\nprint('ALL_PASS')\n"
    with tempfile.NamedTemporaryFile("w", suffix=".py", delete=False) as f:
        f.write(src); path = f.name
    try:
        r = subprocess.run([sys.executable, path], capture_output=True, text=True, timeout=timeout)
        return "ALL_PASS" in r.stdout
    except Exception:
        return False
    finally:
        try: os.unlink(path)
        except Exception: pass


class CodeTask(Task):
    id = "python-code"
    PROBS = [
        {"spec": "Implement `def solve(xs)` returning the sum of a list of numbers (0 for empty).",
         "gold": "def solve(xs):\n    return sum(xs)",
         "tests": "assert solve([1,2,3])==6\nassert solve([])==0\nassert solve([-1,1])==0"},
        {"spec": "Implement `def solve(s)` returning True iff string s is a palindrome (case-sensitive).",
         "gold": "def solve(s):\n    return s == s[::-1]",
         "tests": "assert solve('aba') is True\nassert solve('ab') is False\nassert solve('') is True"},
        {"spec": "Implement `def solve(n)` returning the nth Fibonacci number (fib(0)=0, fib(1)=1).",
         "gold": "def solve(n):\n    a,b=0,1\n    for _ in range(n): a,b=b,a+b\n    return a",
         "tests": "assert solve(0)==0\nassert solve(1)==1\nassert solve(7)==13"},
        {"spec": "Implement `def solve(s)` returning the number of vowels (aeiou, case-insensitive) in s.",
         "gold": "def solve(s):\n    return sum(c.lower() in 'aeiou' for c in s)",
         "tests": "assert solve('hello')==2\nassert solve('XYZ')==0\nassert solve('AeI')==3"},
    ]
    def examples(self): return self.PROBS
    def split(self, seed=0): return self.PROBS, self.PROBS        # tiny demo set: held-out = all
    def context(self, ex): return f"Task:\n{ex['spec']}"
    def instruction(self): return "Write the Python function. Output only a ```python ...``` block. /no_think"
    def extract(self, raw):
        raw = raw.split('</think>')[-1]; m = re.findall(r'```(?:python)?\s*(.*?)```', raw, re.S); return (m[-1] if m else raw).strip()
    def verify(self, pred, ex): return _run_pytests(pred, ex['tests'])
    def gold(self, ex): return ex['gold']


# ----------------------------------------------------------------- CPU-only self-test (no model)
if __name__ == "__main__":
    print("=== HARNESS GENERICITY SELF-TEST (CPU, no model) ===")
    results = []
    # BIRD: gold SQL must verify True against itself (execution verifier works)
    try:
        bt = BIRDTask(); held = bt.split(0)[1]; sample = held[:10]
        okb = sum(bt.verify(bt.gold(ex), ex) for ex in sample)
        results.append(("BIRDTask (SQL, execution verifier)", okb, len(sample)))
    except Exception as e:
        results.append(("BIRDTask", f"ERR {e}", 0))
    # Code: gold solutions must pass their own unit tests (unit-test verifier works)
    ct = CodeTask(); okc = sum(ct.verify(ct.gold(ex), ex) for ex in ct.examples())
    results.append(("CodeTask (Python, unit-test verifier)", okc, len(ct.examples())))
    print()
    for name, ok, n in results:
        flag = "OK" if isinstance(ok, int) and ok == n else "CHECK"
        print(f"  [{flag}] {name}: gold verifies {ok}/{n}")
    tasks_ok = sum(1 for _, ok, n in results if isinstance(ok, int) and ok == n)
    print(f"\n  GENERIC ACROSS {tasks_ok}/{len(results)} TASK TYPES (SQL execution + Python unit-tests) via ONE Task interface + ONE evaluate() runner.")
    print("  => the harness is NOT SQL-specific: any free-verifier task plugs in (math/answer-check, regex, JSON-schema, ...).")
