# Democratizing Intelligence — a research note for Yogi

**Question.** *Given a goal, can a local agent — irrespective of its current capabilities — get
measurably better at that goal, mostly on its own, on commodity hardware?* If yes, you don't need to
*own* a frontier model to *get* strong capability at *your* goal. That is what we mean by
**democratizing intelligence**, and it is the thesis Yogi exists to test.

This note states the thesis precisely, grounds each pillar in the current literature, names the four
conditions under which it works, locates where Yogi stands today, and feeds the companion plan
([democratization-roadmap.md](../plan/democratization-roadmap.md)).

---

## 1. The thesis as a machine

A goal-improvement engine with five parts:

1. **Goal** — a task with examples/instances.
2. **Verifier (grader)** — a cheap-as-possible signal for "is this output better?" *This is the
   lever; everything else is downstream of it.*
3. **Toolspace** — a **closed set of primitives** plus a **composition grammar** (the being can build
   new tools, but only out of primitives).
4. **Evolution** — search over prompts → tool *selection* → tool *creation* → population/selection.
5. **The ratchet (distill-to-weights)** — fold the being's own *verified* successes back into the
   model's weights, so the **floor rises** and the next round of search starts from a stronger base.

The engine's promise is a **ratchet**:

> scaffold transcends the base → the verifier confirms the gain → the gain is distilled into weights →
> the floor rises → the scaffold now squeezes more from a better base → repeat.

Layers 1–4 lift the *scaffold around a frozen model*; layer 5 lifts the *model itself*. Only with 5
does improvement compound instead of plateauing at "what clever prompting can extract from a fixed
model."

---

## 2. Frontier grounding (each pillar is established work)

### 2.1 Verification is the lever
Self-improvement is bottlenecked by the ability to *check* "better," not to *generate* it.
- **RLVR** (RL with Verifiable Rewards) replaces subjective preference with deterministic, checkable
  reward — unit tests, symbolic checkers, validators — and trains against it
  ([awesome-RLVR](https://github.com/opendilab/awesome-RLVR); [Tulu 3](https://arxiv.org/pdf/2411.15124)).
- **Scaling *verification* at inference** (sampling + scrutiny) is itself a major lever
  ([Sample, Scrutinize and Scale](https://arxiv.org/pdf/2502.01839)).
- Implication for us: the grader is the product. Goals with built-in verifiers (code→tests,
  math→checkers, games→score) are the easy on-ramp; subjective goals (like ASCII quality) need a
  frontier judge — which is precisely the *metered cost* the engine must amortize.

### 2.2 Small model + test-time compute + verification ≈ frontier (the democratization evidence)
- A **3B model beats a 405B** on MATH/AIME with compute-optimal test-time scaling — **100–1000× fewer
  FLOPs** ([Scaling Test-Time Compute](https://openreview.net/forum?id=4FWAwZtd2n);
  [VentureBeat summary](https://venturebeat.com/ai/how-test-time-scaling-unlocks-hidden-reasoning-abilities-in-small-language-models-and-allows-them-to-outperform-llms)).
- **Search-based methods favor *small* policy models** specifically; best-of-N favors large ones
  — so the small-local regime *wants* search + verification ([T1](https://arxiv.org/pdf/2504.04718)).
- This is the strongest external evidence that a weak local model + the right engine can reach
  capability that looks like a much larger model's, at a fraction of the cost.

### 2.3 The being can create its own tools (safely, by composition)
- **Voyager** keeps an **ever-growing skill library of executable code**, compositional and retrieved
  by description; complex skills are *composed from simpler ones*, which "compounds the agent's
  capability" — and it does so **without fine-tuning** (in-context lifelong learning)
  ([Voyager](https://arxiv.org/abs/2305.16291)).
- **DreamCoder** learns reusable **libraries of abstractions** via wake-sleep that accelerate future
  problem-solving — the canonical "grow your own primitives-of-thought" result.
- **LATM (LLMs as Tool Makers)** splits **tool-making** (expensive, strong model, once) from
  **tool-using** (cheap, weak model, many times): GPT-4-maker + GPT-3.5-user reaches GPT-4-level
  performance at GPT-3.5 cost ([LATM](https://arxiv.org/abs/2305.17126)). **This is the
  democratization pattern in one paper** — the frontier pays the one-time invention cost; the local
  model reaps it forever.
- Our `def/end/call` macro DSL is the bounded, safe instance of this: **closed alphabet, open
  composition** — an unbounded tool library where no tool can do more than the primitives allow.

### 2.4 Scaffolding transcends the base (base capability is not the ceiling)
- **Self-Refine**: generate → self-critique → refine improves outputs **~20% absolute, no training**,
  with one model as generator+critic+refiner ([Self-Refine](https://arxiv.org/pdf/2303.17651)).
  Weak-generator + strong-critic amplifies *beyond* base performance.
- **Symbolic/program synthesis** beats direct generation for structured output: LLMs fail to emit
  spatial characters but compose geometric *primitives* accurately
  ([Symbolic Graphics Programming](https://arxiv.org/pdf/2509.05208);
  [Visual Sketchpad](https://arxiv.org/pdf/2406.09403)). → *changing the action space* is a real lever
  (the ASCII toolspace).

### 2.5 The ratchet: distill verified traces into weights
- **ReST / STaR / RFT**: generate, filter by verifier, **fine-tune on the successful trajectories**,
  iterate ([ReST/Beyond Human Data](https://arxiv.org/pdf/2312.06585)).
- **AgentTuning / FireAct**: distill successful agent trajectories into weights, internalizing
  procedural skill; **top-1 trajectory curation** per goal yields a high-utility distillation set
  ([Re-ReST](https://arxiv.org/pdf/2406.01495);
  [self-generated trajectories](https://vsanimator.github.io/self_improvement/)).
- **Failure mode, named:** training on your own best samples causes **entropy decay / diversity
  collapse** ([B-STaR](https://arxiv.org/pdf/2412.17256)) — we hit this exactly (the niches=1 plateau),
  and the fix is diversity-preserving (best-per-niche) curation + an external grounding signal (the
  judge). This must be designed in, not discovered late.

### 2.6 Open-endedness needs diversity — and safety
- **Open-endedness** (Stanley/Lehman/Clune) generates "endless novel **but learnable** artifacts," and
  is argued **essential for superhuman intelligence**
  ([OE is Essential for ASI](https://arxiv.org/pdf/2406.04268)). Quality-Diversity (QDAIF) is the
  mechanism we already use — diversity is not a nicety, it's the antidote to 2.5's collapse.
- **Safety is explicitly framed as a precondition** for deploying open-ended systems
  ([Safety is Essential for Responsible Open-Ended Systems](https://arxiv.org/html/2502.04512v1)) —
  which validates the right-sized safety model below rather than contradicting the autonomy goal.

---

## 3. The four conditions (where it works, where it breaks)

The engine delivers **iff**:

1. **A verifier exists** and is cheap enough to run in the loop. No "is it better?" signal → no
   selection → no improvement. (Cheapest: programmatic checks. Costliest: a frontier judge = the
   salary.)
2. **The toolspace can express a solution.** If no composition of primitives + model can reach the
   goal, evolution can't conjure it. (ASCII *is* reachable via drawing primitives.)
3. **A bootstrap floor.** The model must be able to *drive* the tools at all. `qwen3:8b` couldn't even
   emit DSL programs — below the floor you must **teacher-distill the basic tool-use skill first**
   (LATM-style), *then* the ratchet runs.
4. **Compounding is protected.** Entropy collapse (2.5), Goodhart, and false-positive distillation
   will stall or degrade the ratchet — prevented by diversity-preserving curation, anti-Goodhart
   grading, and verify-before-distill.

The ASCII arc was a *stress test at the bootstrap floor* (condition 3 barely met) — which is why it
was hard and why it was informative.

---

## 4. Where Yogi stands today

**Built and working (the engine's skeleton is real):**
- Closed mutation surface + provenance/no-launder + microdollar budget (`being-core-mutation`,
  `being-value`).
- QD illumination, archive, population/selection, signed fork saga (`being-lineage`) — the open-ended
  search substrate (2.6).
- A goal with an **anti-Goodhart structural grader** + a **frontier judge** (CoT/criteria; the salary)
  — the verifier (2.1).
- **Toolspace**: the being evolves its drawing strategy (`DrawTool` via `tool_policy`); **tool
  creation** by macro composition (`def/end/call`) — closed alphabet, open composition (2.3).
- A **diversity-preserving self-distillation flywheel** (best-per-niche, the 2.5 fix) — but token-space
  (in-context), not yet weights.
- A **weight-distillation pipeline** (M3 LoRA on Apple-Silicon/MLX) — proven on another task, **not yet
  wired to the trace flywheel** (the missing layer-5 ratchet).

**What the ASCII arc proved / disproved (honest):**
- The harness is real and frontier-grounded; in-context levers lift quality to the *base's* ceiling
  (~0.40) and then stop — exactly the 2.4/2.5 prediction.
- `qwen3:8b` is below the bootstrap floor for ASCII *generation* (condition 3) — confirmed by probe and
  by literature ([ASCIIEval](https://arxiv.org/html/2410.01733v2): text-only LLMs lag; input-output FT
  *fails* — only rationale-assisted helps).

**The decisive gap:** the **ratchet (layer 5) is not closed**. We have every part except the loop that
takes verified traces → weights → a measurably higher floor. *Until that loop is closed and shown to
raise the floor on at least one goal, the democratization claim is unproven.*

---

## 5. Safety model — right-sized, two tiers

- **Essential tier (always on; it's an *enabler*, not a tax):** resource caps (salary metering),
  anti-Goodhart grading, **verify-before-distill**, diversity-preserving curation. These make the
  ratchet *trustworthy* — a bounded toolspace is also what makes compositions *verifiable*, which is
  what makes distilling them *safe*. Here safety and capability **align**.
- **Deferrable tier (keep as cheap insurance; load-bearing later):** existential capability
  containment — closed surface (no capability self-grant), sandbox, egress proxy, reaper. For a
  *local, single-user, bounded-goal* agent this is not the blocker; it becomes essential when the
  agent is networked, shared, or given real-world primitives. The literature agrees safety should
  *precede* open-ended *deployment* ([2502.04512](https://arxiv.org/html/2502.04512v1)).

The unifying principle: **bound the *action space*, then let both tools and the model evolve freely
inside it** — neither an invented tool nor an evolved model can act outside the toolspace. Full detail
in [evolution-and-safety.md](../evolution-and-safety.md).

---

## 6. The claim, falsifiable

> A local model below frontier capability, given a goal and a verifier, will **measurably raise its own
> floor** at that goal through verified-trace distillation — and reduce its per-task frontier
> dependence over time — within the four conditions of §3.

The plan exists to test this claim on one goal first, then to show it is **goal-agnostic**. If the
floor does not rise after a closed ratchet on a goal that meets the four conditions, the thesis is
wrong for that regime, and we report it.
