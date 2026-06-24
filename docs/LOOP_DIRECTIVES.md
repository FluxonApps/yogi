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

1. **Remote pushes: ALLOWED** (operator lifted the hold 2026-06-24). Push after meaningful milestones (a
   finding recorded, report/PDF updated); keep commits clean. (History: hold-all-pushes was in force, now lifted.)
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
- **Stack (zero-salary, local, n=80):** one-shot 37 -> tools 48 -> decompose 52 -> embedding few-shot 53 (BEST); combined(all three)=52 SATURATES (levers overlap). Weight-update HURTS (TTT 32, distill flat). Ceiling ~53%, likely GENERATION-bound. Views/lexical-retrieval/self-consistency dont help.

## 8. RE-VALIDATION TODOs + BOLD-LEVER QUEUE (keep pushing)

Re-validate (NOT safely validated — don't treat as ceilings):
1. "Richer tools hurt" (v2 37%) — n=40 + confounded; re-test isolated at n>=80.
2. Retrieved-example library (48% flat) — used lexical retrieval; re-test with EMBEDDING retrieval (nomic-embed).
3. Self-invented views (22% backfire) — execution-only validation; re-test with CORRECTNESS-gated
   verified-selection (keep a view only if it raises val accuracy; prune wrong ones).

Bold-lever queue (one per iteration, web-research first; CURRENT priority order):
- [DONE] Frontier-designed views = 41% < base 48% -> views are NOT the lever regardless of correctness.
- [RUNNING] Library with EMBEDDING retrieval (TODO #2) — if flat, in-context examples arent the lever.
- **NEXT: TTT per-database (Section 9 #1)** — gradient adaptation per DB; directly attacks the proven
  in-context-learning weakness; top pick.
- Execution-guided search / decoding (Section 9 #2) — verified search, attacks the generation ceiling.
- Verifier-internalization / self-PRM (Section 9 #3) — breadth beyond execution-verifiable + denser signal.
- Combine proven winners (interactive tools + decompose) confirmed at large n = headline local ceiling.
- Then: diverse-strategy verified ensemble; verified self-curriculum; cost-optimal verified routing (Section 9).

## 9. NOVEL RESEARCH / INVENTION BACKLOG (web-researched 2026-06-24; do not forget)

Ranked by EV against our findings (weak in-context learning; generation-bound; only interactive feedback
helps). Web-research each before building; isolate; confirm n>=80; verified-select; local commit only.

TIER 1 (run these first):
1. **Test-time training (TTT), per-database (TOP PICK).** Fine-tune a tiny throwaway LoRA on each DB's solved
   (question, SQL) pairs, load it for that DB, combine with the tool-agent. Bypasses the in-context-learning
   weakness with GRADIENT updates (library/few-shot were flat). ARC: ~6x from per-instance LoRA. Feasible
   per-DB (cheap). Anchors: arxiv 2411.07279, MarkTechPost TTT-ARC.
2. **Execution-guided decoding / verified search.** Guide GENERATION with the verifier: prune partial queries
   whose partial-execution is invalid; beam-search over valid-by-construction queries. Search, not free
   generation -> attacks the generation ceiling. Anchors: arxiv 1807.03100 (execution-guided decoding),
   ExeSQL 2025.findings-emnlp.1320.
3. **Verifier-internalization / self-generated process reward model.** Distill a LOCAL verifier (from
   execution outcomes + a little frontier judgment) that scores partial/candidate SQL without executing ->
   denser step-level signal AND free gating beyond execution-verifiable domains (the productization breadth).
   Anchors: ReST-MCTS*/PRM survey, SRT (majority-vote-as-proxy, arxiv 2505.21444), ReVeal/MR-RLVR.

TIER 2:
4. **Capability-containment theorem for the EVOLVING toolspace (the publishable differentiator).** Prove the
   verified-selected toolspace keeps the reachable behavior set inside SafeSet (decidable because the surface
   is closed). Literature frame now exists: bounded-capacity <=> PAC-safe (arxiv 2510.04399), SEVerA verified
   self-evolution / zero violations (arxiv 2603.25111), Two-Gate (validation-margin + capacity-cap) guardrail.
5. **Verified self-curriculum.** Model generates its own problems at the edge of ability (verified-solvable but
   currently-failed), TTT/distills on them to raise the floor. Curriculum by verified difficulty (vs flat
   self-distill which was flat).
6. **Diverse-strategy verified ensemble.** Heterogeneous agents (tool / decompose / TTT / exec-guided) propose;
   free verifier selects. Diversity is what homogeneous self-consistency lacked (it was flat).
7. **Cost-optimal verified routing (productization experiment).** Router trained on the FREE verifier signal
   (self-consistency spread, execution-confidence) decides per query: local / more-compute / escalate to
   frontier. No labels; maps+optimizes the accuracy-vs-cost frontier.

TIER 3 (lower priority): activation steering as a "tool" (e.g. a verify-your-joins steering vector);
cross-domain tool/skill transfer (does a verified tool generalize across DBs?); clean salary-amortization
study (frontier-dependence decay across rounds on a SKILL = validates salary-as-one-time-ignition).

KEY REFRAME: the 2025 literature VALIDATES Yogi's core bets — PAC-safe iff bounded-capacity (= closed mutation
surface) and "safe self-improvement is bounded by the evaluation infrastructure" (= the free verifier). So the
safety theorem is now stateable rigorously, and the verifier is both the moat and the breadth limit.

---

*This file is the loop's operating system. Read + print Sections 1-6 every iteration; keep 7-9 current; append
new directives/learnings + novel inventions as they are earned.*
