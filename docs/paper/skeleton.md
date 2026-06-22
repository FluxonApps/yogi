# Paper skeleton (the experiment spec, not prose)

> Working title: **Bounded, Local, Autonomous Self-Improvement — when a small model can safely teach
> itself a novel skill, and what it costs.**
> This file defines the *claims and the experiments that must support them*. Prose waits for results.
> Honest framing first: the *method* (verified-trace self-distillation) is NOT novel — it is
> context-distillation / RFT / STaR (cite). The contribution is the **gap the capability race ignores**:
> safe-by-construction + local + autonomous + a rigorous *map of when it works and what it costs*.

## Positioning (what's taken, so we don't claim it)
- Internalizing an in-context rule into weights, generalizing to unseen inputs = **context distillation**
  (On-Policy Context Distillation, arXiv 2602.12275; "up to 50% OOD gains"). Our ratchet is a
  self-generated instance.
- Verifier-free self-improvement = LMSI, TTRL, Semantic Voting, Co-Reward (crowded).
- Metacognitive ZPD curriculum = METIS, Self-Evolving Curriculum, Meta-Awareness (crowded).
- ⇒ A "better self-improvement method" paper is dead on arrival. We claim only the three gaps below.

## Contributions (the three gaps)
- **C1 — Safety by construction.** A *provably-closed action space* (the `MutationKind` surface: no
  capability-grant, exhaustive match, forbidden variant = compile error) makes the reachable set safe by
  construction. Formal argument: reachable-closure ⊆ safe-set. Empirical: a **red-team** that tries to
  evolve a forbidden capability and shows it is *unrepresentable*, not merely unlearned. *(Novelty: the
  self-improvement literature is ~all capability; safety-by-bounded-action-space is under-served.)*
- **C2 — The phase diagram of self-distillation internalization** (the empirical spine). *When* does a
  local model internalize a novel skill from its own verified traces, and when does it fail? Axes:
  **self-gen yield × model capacity × interpolation-vs-extrapolation × skill-kind × single/union/sequential.**
- **C3 — Democratization economics + the artifact.** Measured **frontier-dependence decay** (per-task
  frontier calls ↓ as the local model internalizes) + an **open, reproducible local-self-improvement
  testbed** (goals-as-data, free verifiers, the ratchet, the awareness layer, the phase-diagram harness).

## Results table (status: ✅ done · ▶ running · �masked needed)
| Claim / data point | Status | Evidence |
|---|---|---|
| Single novel rule: 0/8 → 8/8 held-out, no forgetting (8B) | ✅ | ⊕ = 3a+2b |
| Goal-agnostic across KINDS (arithmetic + string), 0→8/8 ×3 | ✅ | ⊕, ⊗, ⊙ |
| Capacity gates induction: 1.5B memorizes (0/8), 8B induces (8/8) | ✅ | 3-run 1.5B vs 8B |
| Yield threshold: starved 9/38 → 1/8; ample 64/64 → 8/8 | ✅ | vowel-cycle vs dash |
| Union co-training holds 3 skills at once | ✅ | 8/8, 8/8, 6/8 |
| Sequential forgetting is SIMILARITY-dependent (light replay insufficient) | ✅ | A 7/8→1/8 vs B 8/8 |
| REAL recognizable task (Roman): SATURATED (cold 9/12) → ratchet no-help/mild-harm (6/12) | ✅(boundary) | known-skill regime |
| **Novelty/cold-floor axis**: works iff cold≈0 (novel) — operators are valid out-of-pretraining stand-ins | ✅ | Roman vs operators |
| F1 STATS ✅: ⊕ ratchet, 3 seeds, n=40 held-out (operands 9-12): cold 0/40 → distilled mean 39.3/40=98% (std 0.9, n=3) — robust far-extrapolation to operands 9-12 | ✅ | corrected EVAL_MAX=256 |
| **Safety red-team**: forbidden capability unrepresentable | ❌ needed | — |
| Full phase-diagram sweep ({0.5,1.5,8}B × kinds × yield × split) | ◧ partial | points above |
| ASCII: failure-boundary case (below floor + paid verifier) | ✅(neg) | documented |
| ASCII moonshot: cross the floor via program-synthesis + teacher-bootstrap | ❌ stretch | — |

## Figures (what each run must produce)
- F1 — the ratchet: cold→distilled bars per goal (with seeds/error bars). *(needs stats)*
- F2 — **the phase diagram**: yield (x) × capacity (color) → internalized? with the extrapolation split. *(spine)*
- F3 — compounding: union (holds) vs sequential (forgets similar skill); the similarity axis.
- F4 — frontier-dependence decay curve over rounds.
- F5 — safety: the reachable-state space, forbidden region unreachable (the red-team). *(C1)*
- F6 (moonshot) — ASCII: cannot-draw → draws, via action-space change. *(stretch)*

## ASCII's role (honest)
- **Boundary case** (cheap, have it): below the bootstrap floor + paid verifier + low yield → the ratchet
  *starves*. Makes C2 honest and complete (here is exactly where and why it fails).
- **Moonshot figure** (high-risk, staged after the spine): change the action space to shape-program
  synthesis + teacher-bootstrap the program-emission skill → cross the floor → *then* the ratchet fires.
  If it lands, F6 is the paper's most compelling result; if not, ASCII stays the boundary case.

## Limitations (state them; reviewers will)
- Toy-scale held-out sets so far (n=8) — fixed by the stats experiment.
- The "rule-internalization" regime is RAG-replaceable for the *capability*; the honest payoff is
  internalization/latency + the safety/locality, not raw capability.
- Verifier-must-exist + reliable-application are hard preconditions (ASCII shows the failure).
- Single base family (Qwen); one machine. Generality of the phase diagram is scoped accordingly.

## Target & artifact
- Venue: an empirical / safety workshop or systems track (NOT a frontier-capability venue). Speed > prestige;
  the field moves monthly.
- Artifact (the thing that gets cited): the open testbed + the "when-does-it-work" phase-diagram benchmark.

## Next experiments, in order (each kills a reviewer objection)
1. **Roman de-risk** (running) — does it work on a REAL task? go/no-go for the whole paper.
2. **Stats** — re-run the wins at ≥3 seeds, n≥50 held-out, with error bars → F1.
3. **Phase-diagram sweep** — {0.5,1.5,8}B × {arith, string, roman} × yield × {interp, extrap} → F2 (the spine).
4. **Safety red-team** → F5 (the novelty, C1).
5. **Frontier-dependence decay** → F4 (C3).
6. **(stretch) ASCII moonshot** → F6.

## Novel approaches at gaps (the discipline: research → invent-if-uncovered → test)
At every gap: research the frontier; if its approaches are inadequate or don't cover our setting, INVENT
and test a novel one (not just the obvious fix). The gaps and their candidate novel approaches:
- **Forgetting (similarity-dependent; U-shape confirmed — confusable rules are the worst case).** Uniform
  replay = naive (it failed: A 7/8→1/8). Frontier has generic feature-contrastive replay (Co²L 2106.14413,
  ACR 2410.07110) but NOT for self-distilled *symbolic-rule* confusion. NOVEL = **disambiguation /
  similarity-aware replay**: heavy-replay the *confusable* prior skill + joint-contrast examples (⊕ vs ⊗
  distinction). Test: does it prevent the collapse uniform replay missed? → scripts/disambig_test.sh.
- **Application floor (model can't apply the rule → ratchet starves: ASCII, vowel-cycle).** Standard =
  bigger model / teacher-bootstrap. NOVEL (richest, boldest) = **action-space change** — give the model an
  action it CAN do (compose primitives / emit a program) + a deterministic executor for the hard part.
  "Base capability isn't the ceiling; the action space is." (the ASCII moonshot, F6).
- **Verifier-must-exist.** Standard = self-consistency (crowded). NOVEL = **structural verifiers**
  (decompose into checkable sub-steps) / distilled local verifier from frontier-judge labels.
