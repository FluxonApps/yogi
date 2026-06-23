# The frontier: recursive self-invention with weight-internalization, safe by construction

*Directive: do not treat external research as a ceiling. Research to avoid reinventing; then invent past
it. This is the genuinely-novel frontier the literature leaves open — and it ties bet A + bet B + the C3
negative together.*

## What the literature does NOT do (the gap, precisely)
- **DreamCoder / LILO / STITCH / babble** [2310.19791, 2211.16605, 2212.04596]: invent abstractions into
  an **external symbolic library** the model *calls at solve-time*. The library is a permanent runtime
  dependency; **nothing enters the model's weights**; invention is driven by task-solving, not by a
  self-measured capability floor; and there is no safety bound on what can be invented.
- **RAG-internalization** [2510.01375]: internalizes a *given* scaffold/retrieval into weights — but no
  **invention** (the scaffold is supplied), no recursion.
- **Self-evolving agents** [Agent0 2511.16043, survey 2507.21046]: capability-focused; runtime tools; no
  safety-by-construction; no floor-tie.

None close: **invent → verify-crosses-floor → internalize-into-weights → (floor rises) → invent-harder**,
recursively, locally, within a provably-closed action space.

## The claim (novel, and it unifies the project)
> A sub-frontier local model, given a goal it is below-floor on, can **invent** its own action-space
> reformulation, verify (free) that it crosses the floor, **internalize it into its own weights** so the
> scaffold is discarded, and — because the floor has risen — invent a *next* reformulation it could not
> have invented before. The invention is confined to the closed mutation surface (bet A theorem), so the
> being can invent **how it thinks**, never **what it is allowed to do**. Recursion bounded by safety.

Distinct from DreamCoder on all of: weight-internalization (not external library), floor-tie (self-
measured ZPD), recursion into the policy, and the structural safety bound.

## Phase 2a — invention (beyond Phase-1 selection)
Phase 1 (done, F7) *selected* from a fixed menu {direct, program, decompose}. Phase 2a *invents*:
1. **Detect floor:** task class where cold≈0 and direct-taught≈0 (free-verified).
2. **Invent:** prompt the model to *propose K candidate strategies* in open-ended text/code (not a menu)
   — "propose a method to solve this kind of problem." These are model-generated reformulations.
3. **Verify-and-select:** for each candidate, solve N instances *with that strategy in context*; the free
   verifier scores yield; keep the highest-yield strategy (autonomous, no human menu).
4. **Internalize:** distill solving-via-the-winning-strategy under the *plain* prompt → into weights.
5. **Eval:** floor crossed? strategy internalized (plain prompt → uses the invented strategy with no
   scaffold instruction)? Novelty over Phase 1 = the reformulation is *model-invented open-ended*, not
   selected from 3.

## Phase 2b — recursion / compounding (the deepest, and the C3 rematch)
The C3 graduation curve collapsed because naive sequential distillation memorized **instances** (priors
eroded; plasticity died at skill 4). Hypothesis: internalizing **abstractions** (reusable strategies)
instead compounds:
1. Internalize strategy S1 for task T1 (Phase 2a).
2. Present T2 — below-floor *even with one fresh strategy*, but solvable by **composing S1 (now in
   weights) with a newly-invented S2**.
3. Test: does having S1 in-weights *enable* inventing/using S2 (a floor-rise that bootstraps the next
   invention)? Does accumulating *abstractions* avoid the instance-accumulation collapse (priors retained,
   plasticity preserved)?
**This is the rematch with our own negative result** — if abstraction-internalization compounds where
instance-internalization collapsed, that is the paper's strongest single claim: *recursive self-
improvement compounds when the unit of internalization is an invented abstraction, not an instance.*

## Why it's safe to let it invent (bet A binds it)
Invention is open-ended in *content* (strategies/programs/decompositions) but the heritable change still
flows only through `apply` over the closed `MutationKind` — so an invented "strategy" can change prompt/
policies/skills/navigators, never introduce a capability/trust/kernel/budget/reaper power (Thm 2,
unrepresentable). The being can invent arbitrarily expressive *ways of thinking* inside the heritable-
field algebra; the safety theorem is exactly what makes "let it invent freely" defensible. **Recursion
without a containment story is the thing everyone fears; we have the containment story first.**

## Risks / honest kill-criteria
- Phase 2a: the 8B may invent only trivial restatements of "write a program" (collapsing to Phase 1). Kill
  iff invented strategies don't beat the fixed-menu baseline. (Still a result: "selection ≈ invention at
  8B scale.")
- Phase 2b: recursion may not compound (S1-in-weights doesn't help invent S2) — then the honest finding is
  "abstraction-internalization also hits the accumulation limit," bounding the claim.
- Measurement: must check invented strategies aren't just retrieved memorized patterns (novelty probe:
  cold≈0 on the *composed* T2).

## Run order (the loop)
1. (running) F7-suite → the n=4 selection base.
2. Phase 2a invention experiment (after suite) → does open-ended invention beat menu-selection + internalize?
3. Phase 2b recursion experiment → does abstraction-internalization compound where C3's instance-accumulation collapsed?
