# LOOP DIRECTIVES & ACCUMULATED LEARNINGS

**Read this file at the START of every loop iteration and restate (print) the directives, so they are never
forgotten. This is the durable source of truth for how the autonomous loop must operate.**

---

## A. Standing operational directives (current)

1. **HOLD ALL REMOTE PUSHES.** Commit locally only. Never `git push` until the operator explicitly lifts this.
2. **Keep pushing the ceiling with very novel frontiers.** Do NOT consolidate/stop while bold levers remain
   in the queue (Section E). Only consolidate when the queue is genuinely exhausted and everything is in-band.
3. **Run fully autonomously. Do not ask permission.** Diagnose, decide, act, record, reschedule.
4. **One model at a time (16 GB).** GPU-free check before any launch: `ps -eo comm | grep -ic python` (0 = free).
   Never run two MLX/model jobs at once.
5. **Memory-safe LoRA only** (if distilling): `batch-size 1, num-layers 8, max-seq-length <=896, --grad-checkpoint`.
   Long sequences OOM otherwise.
6. **Zero salary by default.** Spend salary (claude -p) only when a step structurally needs it; cap it and make
   it deliberate. Measure amortization (frontier-dependence decay) whenever salary is used.
7. `STUDENT = mlx-community/Qwen3-8B-4bit`. `HF_HUB_DISABLE_PROGRESS_BARS=1` always.
8. **Every iteration:** append `docs/FINDINGS.md`, refresh `.yogi/status.txt` (NOW/PHASE), archive the run log to
   `docs/paper/runs/`, commit (LOCAL only).

## B. Rigor / methodology lessons (hard-won — violating these has produced false results)

1. **n=40 is NOISY** (binomial std ~3 at p=0.4). A few-point difference at n=40 is within noise.
   **Confirm every claimed win at n>=80** before recording it as real. (v1 looked like 47% at n=40; needed n=80
   to confirm 48% vs one-shot 37%.)
2. **Eval truncation = FALSE failures.** If eval `max_tokens` < the model's answer length, the answer is
   silently cut and scored 0. This bit ~4 times (once a "0/40" that was actually 98%). **Always re-eval at
   higher max_tokens before recording a kill. Diagnose model-vs-eval before believing any bad score.**
3. **Never conclude from ONE config.** A flat result means "this version didn't help," NOT "this lever can't
   help." Test isolated variants before declaring a wall.
4. **Isolate variables.** Do not bundle multiple changes in one experiment (v2 bundled +FIND +PEEK +2 rounds,
   so its drop could not be cleanly attributed). Change one thing at a time.
5. **Use the strongest reasonable implementation, not the cheapest.** Cheap implementations can manufacture a
   false null (lexical retrieval instead of embeddings; execution-only validation instead of correctness).
6. **VERIFIED-SELECTION is the rule and the moat.** Keep a tool/view/skill ONLY if it raises END-TASK accuracy
   on a held-out/validation set — not merely that it runs. **Unverified self-toolmaking is actively harmful**
   (self-invented views that ran but were semantically wrong dropped accuracy 48% -> 22%).
7. **Diagnose-before-kill.** Check the real process (`ps -eo comm|grep -ic python`, the venv python resolves to
   the homebrew framework path so the `.venv-mlx/bin/python` grep falsely reads 0). Don't re-launch over a live
   job; don't re-eval an adapter while it's being used (two models).
8. **Don't declare a wall you haven't tested with the right method.** (The BIRD "can't download" claim was wrong
   — I only tried one method.)
9. **Record honestly.** Flat is flat; mark under-powered results as under-powered; surface confounds.

## C. Key findings to date (context for the loop)

- **Two ceilings.** Static ceiling (one-shot/sampling/fine-tuning/static-context) ~37-40%. Tool-assisted ceiling
  (interactive tools) 48%, +decompose 52%. The gap is the harness, not the model.
- **Only INTERACTIVE runtime feedback helps** this weak model. Context augmentations (static rich schema,
  retrieved examples, in-context plans) are marginal or flat. Self-invented (unverified) tools backfire.
- **Frontier baseline (Claude) = 75%** on the same n=40. The gap to it is BASE GENERATION capability, which
  needs scale (~900k traces) or RL — out of the local-cheap-few-shot scope.
- **Skill grain works, free** (F1-F9, up to ~98% on homogeneous tasks). Domain grain is capability-bounded
  locally. Tools/action-space is the democratization lever; the closed mutation surface keeps it safe.
- **Stack (zero-salary, local, n=80):** one-shot 37% -> interactive tools 48% -> +decompose 52%.

## D. Re-validation TODOs (failures NOT safely validated — do not treat as ceilings yet)

1. **"Richer tools hurt" (v2 37%)** — was n=40 + confounded (3 changes). Re-test isolated at n>=80.
2. **Retrieved-example library (48%, flat)** — used cheap LEXICAL retrieval. Re-test with EMBEDDING retrieval
   (nomic-embed). RAG/few-shot helps text-to-SQL in the literature; the null may be the weak retriever.
3. **Self-invented views (22%, backfired)** — used execution-only validation. Re-test with CORRECTNESS-gated
   verified-selection (keep a view only if it raises val accuracy on items[80:120], prune wrong ones). This
   should fix the backfire and demonstrate the verified-selection moat empirically.

## E. The bold-lever queue (keep pushing, one per iteration, web-research each first)

- Frontier-designed CORRECT views (running) — concept-vs-correctness diagnostic + salary lever.
- AMORTIZATION: after frontier tools, can the local model self-propose CORRECT ones? (frontier-dependence
  decay = the productization test).
- Verified-selection self-invention done right (Re-validation TODO #3).
- Library with embedding retrieval (Re-validation TODO #2).
- Iterative self-invention rounds with verified-selection (DGM/Voyager archive across rounds).
- Combine proven winners (tools+decompose) confirmed at large n = the headline local ceiling.

## F. Productization thesis (the why, keep in view)

The product is the **self-evolving toolspace LAYER** (model-agnostic, verifier-gated, safe-by-construction via
the closed mutation surface), not a model. It applies to frontier models too, likely stronger (they orchestrate
richer evolved toolsets without the weak-model distraction penalty). The moat is **verified-selection rigor +
the safety theorem + compounding domain assets**. It only works where a cheap verifier exists. The 22% backfire
is direct evidence that verified-selection (not tool generation) is the defensible part.

---

*Update this file as new directives/learnings accumulate. It is read and printed every loop iteration.*
