# Democratization Roadmap — closing the ratchet, then generalizing

> STATUS 2026-06-22: P0 (goal=data, Goal trait) ✓ · P1 (ratchet raises the floor) ✓ on the real agent (qwen3:8b 0→8/8) · goal-agnostic ✓ across 3 goals / 2 kinds, zero forgetting, zero salary · boundary = self-gen yield (application-reliability). NEXT: pure-STaR regime + agent-driven loop. See docs/research/democratizing-intelligence.md §7.

Companion to [research/democratizing-intelligence.md](../research/democratizing-intelligence.md).
Goal of this plan: **prove the claim of §6 of the research note** — a sub-frontier local model raises
its own floor at a goal via verified-trace distillation, then show it is goal-agnostic — and do it in
the cheap-safety, local-only regime.

## Sequencing rationale

The ASCII arc showed `qwen3:8b` sits *below the bootstrap floor* for ASCII generation (condition 3).
So we do **not** try to prove the ratchet there first. We prove it on a goal that meets all four
conditions cleanly (cheap programmatic verifier; model already above the floor), *then* return to the
hard bootstrap case, *then* generalize. Prove the engine before stress-testing it.

Order: **P0 finalize → P1 close the ratchet (make-or-break) → P2 tool-creation in-loop → P3 bootstrap
the hard goal → P4 shrink frontier dependence → P5 generalize.**

---

## Phase 0 — Finalize & generalize the domain (no new capability, just shape)

**Why:** today a goal = a hand-written crate. For "any goal" a goal must be **data**.

**Deliverables**
- A `Domain` trait / struct in a new `being-domain` crate: `{ name, instances(), Verifier, Toolspace }`
  where `Verifier: fn(&Instance, &Output) -> f64` (the §2.1 grader) and `Toolspace = { primitives,
  compose }` (the §2.3 closed alphabet).
- `being-ascii` refactored to *implement* `Domain` (instance #1), nothing lost.
- A **second domain that meets all four conditions**: a tiny **programmatic-verifier** goal where
  qwen is already above the floor (candidate: short string/number transforms or micro-codegen checked
  by `assert`-style tests — a local RLVR verifier, §2.1).

**Acceptance:** both domains run through the *same* `illuminate` loop with zero domain-specific code in
`being-lineage`; `cargo test --all` + clippy green.

**Cost/safety:** model-free (green-gate untouched); essential-tier safety only.

---

## Phase 1 — Close the ratchet (THE make-or-break)

**Why:** this is the unproven core (§4 "decisive gap"). Everything else is decoration without it.

**Deliverables**
- **Trace capture:** during illumination, log every *verified-success* episode as a `{prompt → output}`
  (or `{prompt → tool-program}`) training record, **diversity-preserved (best-per-niche, the B-STaR
  fix)** and **top-1-per-instance curated** (per [self-improvement curation](https://vsanimator.github.io/self_improvement/)).
- **Distill step:** feed the curated traces to the existing MLX LoRA pipeline (`scripts/distill_lora.sh`,
  adapted) — RFT/ReST on self-generated successes ([ReST](https://arxiv.org/pdf/2312.06585)).
- **Hot-swap:** load the adapter as the being's base model (the `DomainModel` mutation already permits a
  model swap), and **re-evaluate on held-out instances**.

**Acceptance (quantitative, falsifiable):**
- On the Phase-0 programmatic goal: **distilled-floor > cold-floor** on held-out instances by a
  margin beyond noise (pre-register e.g. ≥ +0.1 pass-rate or +1 absolute on a 0–10 grade), with the
  *same* scaffold at eval time (so the gain is in the weights, not the prompt).
- The improvement **survives a general-capability check** (a small held-out "don't forget" set, à la
  the M3 replay set) — no catastrophic forgetting.

**Kill-criterion (honest):** if a properly-curated, diversity-preserved distill does **not** raise the
held-out floor on a goal meeting all four conditions, the thesis is wrong for this regime — record it
in FINDINGS and stop, rather than chase it.

**Cost/safety:** foreground MLX (GPU) + metered judge; verify-before-distill is the key safety gate.

---

## Phase 2 — Tool-creation in the loop (compounding via a skill library)

**Why:** the durable, transferable form of capability is an **accumulating, composable tool library**
(Voyager/DreamCoder), and it is the cheapest democratization lever (LATM: invent once, reuse forever).

**Deliverables**
- A **persistent tool library**: verified macros (`def/end` compositions) that clear the grader are
  admitted, embedded-and-retrieved by description (Voyager-style), and become callable primitives for
  later composition.
- **LATM split:** the **frontier (Claude) is the tool-*maker*** (writes/needs-a-new-tool, paid once,
  metered), the **local model is the tool-*user*** (composes existing tools, free) — with a dispatcher
  deciding reuse-vs-make ([LATM](https://arxiv.org/abs/2305.17126)).

**Acceptance:** measured **tool-reuse rate climbs** across generations and **per-task frontier calls
fall** as the library fills (the LATM cost-amortization effect, observed locally).

**Cost/safety:** closed-alphabet composition keeps every invented tool incapable of out-of-toolspace
action (essential tier); only *new primitives* remain operator-gated.

---

## Phase 3 — Bootstrap the hard goal (ASCII), honestly

**Why:** ASCII is the below-floor case (condition 3). Test whether teacher-distillation can *install
the bootstrap skill*, then let the ratchet run.

**Deliverables**
- **Teacher-distill tool-use:** Claude writes valid drawing *programs* (it can — it draws well); LoRA
  qwen to *emit DSL programs at all* (a learnable code/compose skill, unlike raw spatial emission which
  [ASCIIEval](https://arxiv.org/html/2410.01733v2) shows input-output FT *can't* teach).
- Then run Phase-1 ratchet + Phase-2 library on ASCII.

**Acceptance:** post-bootstrap qwen emits non-empty valid programs ≥ X% of the time (crosses the
floor); *then* the ratchet raises ASCII quality past the 0.40 in-context ceiling. **Kill-criterion:**
if even teacher-bootstrap can't get the 8B above the floor, record "8B is the wrong base for ASCII
generation; engine sound, base insufficient" — a clean, valid result.

**Cost/safety:** higher frontier spend (Claude writes a teacher corpus) — strictly metered/capped.

---

## Phase 4 — Shrink frontier dependence (the democratization metric)

**Why:** "democratized" means the *local* system needs the frontier *less* over time.

**Deliverables**
- **Distill the verifier/critic too** where safe: train a small local grader/critic from frontier-judge
  labels (a local PRM), so routine grading stops paying the salary; reserve the frontier for hard/novel
  cases (active-learning routing).
- A dashboard line: **frontier-calls-per-unit-progress over rounds**.

**Acceptance:** per-task frontier calls **trend down** across rounds while floor holds/rises — i.e. the
agent is becoming self-sufficient at the goal.

**Cost/safety:** local-grader risks reward-hacking → keep periodic frontier audits (anti-Goodhart,
essential tier).

---

## Phase 5 — Generalize (prove goal-agnostic)

**Why:** the thesis is *any* goal, not one.

**Deliverables:** a **third domain added as pure data/config** (no new engine code) — ideally one a
contributor could supply — run end-to-end through P1–P4.

**Acceptance:** a new goal with `{instances, verifier, toolspace}` ratchets up with **zero changes to
`being-lineage`/`being-core-mutation`** — the engine is the product, the goal is data.

---

## Cross-cutting

**Safety (per phase):** essential tier always on (resource caps, anti-Goodhart, verify-before-distill,
diversity-preservation); deferrable tier (closed surface / sandbox / egress / reaper) kept as cheap
insurance, marked load-bearing at networked/shared deployment.

**Green-gate invariant (unchanged):** `cargo test --all` + clippy never load a model; all model/MLX
work is foreground/bin only; salary hard-capped so no loop drains the subscription.

**Top risks & honest kill-criteria**
- *Ratchet doesn't lift the floor* (P1) → thesis wrong for this regime; report and stop.
- *Entropy collapse on self-distillation* → diversity-preserved curation + external grounding (already
  the flywheel design); monitor niche coverage.
- *Goodhart on a distilled local grader* (P4) → periodic frontier audits.
- *Base below floor even after bootstrap* (P3) → valid null; the engine is still proven on P1/P2 goals.

**Definition of done for the thesis:** Phases 1, 2, 4, 5 green on ≥2 goals — a sub-frontier local model
that raises its own floor *and* lowers its frontier dependence at goals supplied as data. That is the
democratization claim, demonstrated.
