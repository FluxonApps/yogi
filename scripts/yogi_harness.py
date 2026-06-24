"""Yogi eval harness — task-agnostic, verifier-gated, method-composable. Bakes in the rigor lessons.

Add a TASK: implement the Task protocol (any FREE deterministic verifier — execution, unit tests, exact match).
Add a METHOD: implement solve(ex, task, model) -> prediction.
The Eval runner enforces the hard-won rigor: a safe max_tokens with a TRUNCATION re-check (the recurring
false-zero bug), an n<80 UNDERPOWERED warning, and verified_select() (the moat: keep only what raises accuracy).

SQL is just one Task. See yogi_tasks.py for BIRDTask (execution verifier) AND CodeTask (unit-test verifier),
which proves the harness is generic beyond SQL. Model import is lazy so CPU-only tasks/self-tests don't load MLX.
"""
import os, re, json, random, subprocess
os.environ.setdefault("HF_HUB_DISABLE_PROGRESS_BARS", "1")

STUDENT = "mlx-community/Qwen3-8B-4bit"


# ---------------------------------------------------------------- Task protocol
class Task:
    """A task = examples + a free deterministic verifier + how to present/extract. SQL-agnostic."""
    id = "task"
    def examples(self):            raise NotImplementedError      # -> list[dict]
    def split(self, seed=0):                                       # -> (train, held_out)
        ex = list(self.examples()); random.Random(seed).shuffle(ex)
        h = max(1, len(ex) // 3); return ex[h:], ex[:h]
    def context(self, ex):         raise NotImplementedError      # prompt body (schema/spec/question)
    def instruction(self):         return "Answer."               # imperative appended to context
    def extract(self, raw):        return raw.strip()             # pull the answer from model output
    def verify(self, pred, ex):    raise NotImplementedError      # FREE deterministic verifier -> bool
    def gold(self, ex):            return ex.get("gold")          # for diagnostics


# ---------------------------------------------------------------- Model runner (lazy MLX)
class Model:
    def __init__(self, name=STUDENT):
        from mlx_lm import load
        self.m, self.t = load(name); self.name = name
    def gen(self, prompt, max_tokens=512, sample=False):
        """Returns (text, hit_cap). hit_cap=True flags likely TRUNCATION (the recurring false-zero bug)."""
        from mlx_lm import generate
        kw = {}
        if sample:
            from mlx_lm.sample_utils import make_sampler; kw["sampler"] = make_sampler(temp=0.7)
        p = self.t.apply_chat_template([{"role": "user", "content": prompt}], add_generation_prompt=True, tokenize=False)
        out = generate(self.m, self.t, prompt=p, max_tokens=max_tokens, verbose=False, **kw)
        try: hit_cap = len(self.t.encode(out)) >= max_tokens - 2
        except Exception: hit_cap = False
        return out, hit_cap


# ---------------------------------------------------------------- Method protocol
class Method:
    name = "method"
    def solve(self, ex, task, model): raise NotImplementedError   # -> prediction (already extracted)


class OneShot(Method):
    """Generic baseline: works for ANY task. Bakes in the truncation guard."""
    name = "one-shot"; max_tokens = 512
    def solve(self, ex, task, model):
        prompt = task.context(ex) + "\n" + task.instruction()
        out, cap = model.gen(prompt, self.max_tokens)
        if cap:                                   # RIGOR: truncated -> retry once at 2x before trusting a miss
            out, _ = model.gen(prompt, self.max_tokens * 2)
        return task.extract(out)


# ---------------------------------------------------------------- Eval runner (rigor baked in)
def evaluate(task, method, model, n=80, seed=0, verbose=True):
    _, held = task.split(seed)
    test = held[:n]
    ok = 0
    for i, ex in enumerate(test):
        try: pred = method.solve(ex, task, model)
        except Exception: pred = None
        if pred is not None and task.verify(pred, ex): ok += 1
        if verbose and (i + 1) % 20 == 0: print(f"  {i+1}/{len(test)}: {method.name} {ok}", flush=True)
    res = {"task": task.id, "method": method.name, "n": len(test), "ok": ok,
           "acc": round(100 * ok / max(1, len(test)))}
    if len(test) < 80:
        res["WARN"] = "UNDERPOWERED (n<80) — confirm at n>=80 before recording a win"
    return res


def verified_select(task, model, candidates, baseline_acc, val_n=80, seed=0):
    """THE MOAT: keep a candidate method/tool ONLY if it raises held-out accuracy over baseline.
    Unverified self-toolmaking is actively harmful (a runnable tool can still lower end-task accuracy)."""
    kept = []
    for c in candidates:
        a = evaluate(task, c, model, n=val_n, seed=seed, verbose=False)["acc"]
        status = "KEEP" if a > baseline_acc else "PRUNE"
        print(f"  verified-select [{c.name}] acc={a} vs base {baseline_acc} -> {status}", flush=True)
        if a > baseline_acc: kept.append((c, a))
    return kept


def gpu_free():
    """True if no other model job is running (the venv python resolves to the framework path; count broadly)."""
    out = subprocess.run(["bash", "-c", "ps -eo comm|grep -ic python"], capture_output=True, text=True).stdout
    return int(out or "0") <= 1
