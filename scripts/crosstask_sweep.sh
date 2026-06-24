#!/usr/bin/env bash
# HARNESS END-TO-END VALIDATION + first CROSS-TASK baseline. Runs the generic OneShot Method via evaluate()
# on all 4 task types (SQL/code/math/ASCII) through the SAME harness — proves the standardized harness runs
# the model end-to-end (not just the CPU self-test) and gives the first cross-task one-shot pattern point.
# Small demo sets for code/math/ascii (n caveat noted); BIRD at n=40. Zero salary.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
.venv-mlx/bin/python - <<'PY'
import sys; sys.path.insert(0,"scripts")
from yogi_harness import Model, OneShot, evaluate
from yogi_tasks import BIRDTask, CodeTask, MathTask, ASCIIArtTask
print("loading model once (shared across all tasks)...",flush=True)
m=Model()
print("\n=== HARNESS CROSS-TASK SWEEP (OneShot via the standard evaluate() runner) ===",flush=True)
for task in [BIRDTask(), CodeTask(), MathTask(), ASCIIArtTask()]:
    r=evaluate(task, OneShot(), m, n=40, verbose=False)
    warn=("  ["+r["WARN"]+"]") if "WARN" in r else ""
    print(f"  {task.id:20s} one-shot {r['ok']}/{r['n']} ({r['acc']}%){warn}",flush=True)
print("\n  Harness runs the model end-to-end across 4 verifier types via ONE interface. (small n on code/math/ascii = noisy; real datasets are the follow-up for robust cross-task patterns.)",flush=True)
PY
