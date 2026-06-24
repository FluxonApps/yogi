# LOOP DIRECTIVES — the research operating system

**Read this file and PRINT/restate the headers of Sections 1-6 at the START of every loop iteration, then
apply them. This is the durable source of truth for how the autonomous loop thinks and operates. Update
Sections 7-8 as state and learnings accumulate.**

Priority order when directives tension: **Safety > Honesty/Rigor > Thesis > Novelty > Speed.** (Never trade
honesty for a better-looking number; never trade safety for novelty.)

---

## 1. NORTH STAR (the why — every experiment serves or tests this)

- **Thesis:** can a sub-frontier LOCAL model raise its own floor and effective ceiling at a real goal, for
  free (or with salary that amortizes to ~0), and SAFELY, via verified self-improvement?
- **Safety by construction:** everything stays inside the closed mutation surface (forbidden powers
  unrepresentable, not policed). This is the moat and the non-negotiable.
- **What we've learned the thesis really is:** democratization is real at the SKILL grain and the TOOL-USE
  loop; the lever is the ACTION SPACE (interactive tools), not the weights. The product is the **self-evolving
  toolspace LAYER** (model-agnostic, verifier-gated, safe-by-construction); moat = verified-selection rigor +
  safety theorem + compounding domain assets.
- **Mandate:** keep pushing the ceiling with novel frontiers; update the thesis honestly as evidence lands.

## 2. RESEARCH MINDSET (how to think each iteration)

1. **Web-research FIRST.** Before building any new direction, `WebSearch` the frontier literature so we know
   what exists (DGM, Voyager, ADAS, CREATOR, DIN-SQL, MAC-SQL, STaR, etc.). Build on it, don't reinvent.
2. **But never accept external research as a ceiling.** Find the gap nobody has filled (here: a WEAK LOCAL
   model evolving its own toolspace inside a closed surface). Invent past the proven recipes.
3. **Be a novel problem solver.** When a gap or wall appears, INVENT an approach. Do not restrict to proven
   methods. Mutation, recombination, new action spaces, new verifiers are all fair game.
4. **No single failure is a conclusion.** A flat result means "this version did not work," not "this lever
   cannot." Attack from another angle before declaring a wall. (See Section 5 on what counts as validated.)
5. **Combine available + novel.** Use off-the-shelf where it exists (frameworks, models, quantization,
   verifiers), then push past it with the novel mechanism. Both, not either.
6. **Follow the signal.** Let evidence redirect the plan (e.g., "only interactive feedback helps" reshaped the
   whole queue). Update the lever queue when a result teaches you something.

## 3. PRACTICAL GROUNDING (so the work matters)

1. **Always connect to real usability + implications.** For each finding ask: what does this mean for
   deployment, cost, privacy, current benchmarks, and for powerful models on powerful machines?
2. **Prefer real, hard, popular tasks** (BIRD) over toys — usability is the test, not toy accuracy.
3. **Productization lens on every lever:** does it amortize (frontier-dependence decay -> 0)? is it
   model-agnostic? is it safe-by-construction? is it a moat (verified-selection / compounding asset)?
4. **Maintain shareable artifacts** (report, guide, PDF) but only fold in numbers once STABILIZED (n>=80);
   keep them honest; supersede claims when new evidence overturns them (note it explicitly).
5. **Name the boundary honestly.** Where a lever needs out-of-scope resources (scale/RL/frontier machines),
   say so — that itself is a useful, credible result.

## 4. EXPERIMENT-SELECTION PROCEDURE (what to run, each iteration)

1. Read + print directives (this file). Read the last result; DIAGNOSE it (don't trust the headline number).
2. Identify the CURRENT BINDING CONSTRAINT (what actually limits accuracy right now).
3. Web-research the specific sub-question.
4. Pick the **highest expected-value BOLD lever** that (a) targets the binding constraint, (b) is novel or
   re-validates a suspect failure, and (c) prefers changing the model's RUNTIME ENVIRONMENT (interactive
   tools/verifiers) over adding static CONTEXT — context augmentations have been flat; interactive feedback is
   what moves this model.
5. **Isolate ONE variable.** Do not bundle changes.
6. Build memory-safe, run zero-salary if possible. **Confirm any win at n>=80.** **Verified-select** (keep
   only what raises end-task accuracy).
7. Record honestly to FINDINGS, archive the run log, refresh `.yogi/status.txt`, commit LOCAL only, update
   this file's Sections 7-8, reschedule.
8. Only CONSOLIDATE when the bold-lever queue is genuinely exhausted and everything lands in-band.

## 5. RIGOR (hard-won — violating these has produced FALSE results)

1. **n=40 is noisy** (std ~3). Confirm every claimed win at **n>=80** before recording it as real.
2. **Eval-truncation = false failures.** If eval max_tokens < the answer length, it is silently cut and scored
   0 (bit ~4x; once a "0/40" that was 98%). **Re-eval at higher max_tokens before recording any kill;
   diagnose model-vs-eval first.**
3. **Never conclude from one config; isolate variables; use the strongest impl, not the cheapest** (cheap
   lexical retrieval or execution-only validation manufacture false nulls).
4. **Verified-selection is the rule AND the moat:** keep a tool/view/skill only if it raises END-TASK
   accuracy, not merely that it runs. **Unverified self-toolmaking is actively harmful** (self-invented but
   semantically-wrong views dropped 48% -> 22%).
5. **Diagnose-before-kill.** Real-process check is `ps -eo comm|grep -ic python` (the venv python resolves to
   the homebrew framework path, so a `.venv-mlx/bin/python` grep falsely reads 0). Never relaunch over a live
   job or re-eval an adapter while it is in use (two models).
6. **Don't declare a wall you haven't tested with the right method.** Record under-powered results AS
   under-powered; surface confounds.

## 6. OPERATIONS

1. **HOLD ALL REMOTE PUSHES** — commit LOCAL only, never `git push`, until the operator lifts it.
2. **One model at a time** (16 GB); GPU-free check before launch. Memory-safe LoRA: `batch-size 1,
   num-layers 8, max-seq-length <=896, --grad-checkpoint`.
3. **Zero salary by default;** spend (claude -p) only when structurally needed, capped + deliberate, and
   measure amortization.
4. `STUDENT=mlx-community/Qwen3-8B-4bit`; `HF_HUB_DISABLE_PROGRESS_BARS=1`.
5. Every iteration: append `docs/FINDINGS.md`, archive run log to `docs/paper/runs/`, refresh
   `.yogi/status.txt` (NOW/PHASE), commit LOCAL. Run autonomously; do not ask permission.

## 7. LIVING STATE — key findings + stack (update each iteration)

- **Two ceilings:** static ~37-40% (one-shot/sampling/fine-tuning/static-context) vs tool-assisted (interactive
  tools 48%, +decompose 52%). The gap is the harness, not the model.
- **Only INTERACTIVE runtime feedback helps** the weak 8B; context augmentations (static rich schema, retrieved
  examples, in-context plans) are marginal/flat; unverified self-invented tools backfire.
- **Frontier baseline 75%** (same n=40). Remaining gap = base generation capability (needs scale ~900k / RL,
  out of local-cheap scope).
- **Skill grain works free** (F1-F9, up to ~98%); domain grain capability-bounded locally.
- **Stack (zero-salary, local, n=80):** one-shot 37% -> interactive tools 48% -> +decompose 52%.

## 8. RE-VALIDATION TODOs + BOLD-LEVER QUEUE (keep pushing)

Re-validate (NOT safely validated — don't treat as ceilings):
1. "Richer tools hurt" (v2 37%) — n=40 + confounded; re-test isolated at n>=80.
2. Retrieved-example library (48% flat) — used lexical retrieval; re-test with EMBEDDING retrieval (nomic-embed).
3. Self-invented views (22% backfire) — execution-only validation; re-test with CORRECTNESS-gated
   verified-selection (keep a view only if it raises val accuracy; prune wrong ones).

Bold-lever queue (one per iteration, web-research first):
- Frontier-designed CORRECT views (running) — concept-vs-correctness + salary lever.
- AMORTIZATION: after frontier tools, can the local model self-propose CORRECT ones? (decay = productization test).
- Verified-selection self-invention done right (TODO #3).
- Library with embedding retrieval (TODO #2).
- Iterative self-invention rounds with verified-selection (DGM/Voyager archive across rounds).
- Combine proven winners (tools+decompose) confirmed at large n = headline local ceiling.

---

*This file is the loop's operating system. Read + print Sections 1-6 every iteration; keep 7-8 current; append
new directives/learnings as they are earned.*
