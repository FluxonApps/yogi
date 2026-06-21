# Frontier Being — Architecture Specification (v0 draft)

> An independent, self-evolving being. A clean-sheet design that inherits the
> validated learnings of four Akshi prototypes (`akshi-v0`, `akshi-v1`,
> `akshi-v2`, current `akshi`) and their frontier-research backlog, **without
> resuming any of them**. The substrate is a Samkhya-framed agent runtime
> (the mind-body); the self-evolving layer adds the three capacities an LLM
> structurally lacks: metabolism, intrinsic goals, and heredity-with-selection.

Status: design draft. Nothing here is built. Citations point to the source of
each idea: `(v0 #NNN)` / `(v1 #NNN)` = GitHub issues on `akshi-mesh/akshi-v0`
and `akshi-mesh/akshi-v1`; `(v2 ADR-NNNN)` = `akshi-mesh/akshi` ADRs;
`(akshi D-NN)` = current `akshi-ai/akshi` decisions. Research citations are
as-recorded in those tickets and are **unverified**.

---

## 0. The thesis in one paragraph

A being is independent and self-evolving when it (1) maintains itself against a
resource gradient, (2) wants things because wanting was selected for, and (3)
changes across generations by copies, variation, fitness, and death. An LLM has
none of these. This system supplies all three. The **spine is a metabolic
economic loop**, not the module stack: the being earns its own compute budget by
doing valuable work cheaply, reinvests the surplus into improving itself, and
**dies when it goes insolvent**. Survival is therefore the goal we never wrote
down, and budget-survival-over-time is the fitness function selection runs on.
The Samkhya runtime is the organism this loop animates. Remove the model and a
coherent skeleton remains: it still types inputs, remembers, enforces policy,
**maintains its budget, and lives or dies by a fitness function**. That is the
test (from the handover) this design is built to pass.

---

## 1. Design tenets

These are the design's current load-bearing positions, ordered (earlier wins
conflicts). Each is a **hypothesis held with reasons, not a settled law** —
nothing here is rejected by inheritance from any prior system. Every position
that has a credible alternative is carried as a live option in Appendix B and is
re-evaluated on its own merits for a frontier (non-product) being, not closed.

1. **The homeostat must bind, or it is not metabolism.** A budget that never
   ends a being is theater. There is a reaper. (Closes the single biggest gap in
   all four prototypes — every one metered budget; none let the being die of it.)

2. **Propose → commit → attest is one seam, with its own types and signed audit
   trail.** The model (Manas) proposes; a separate, mostly-deterministic layer
   (Buddhi) commits. Free generation never sits inside the gate. (v1's clean seam;
   v2's 7-step signed `Agent::turn`.)

3. **The mutable surface is closed by the type system, not by a runtime check.**
   Capability grants, trust-policy edits, the signature boundary, and the
   execution kernel are *not variants of the mutation enum* — so self-escalation
   is unrepresentable, not merely forbidden. (v2 ADR-0011; akshi D-64; motivated
   by reward-hacking inside memory evolution — *Your Agent May Misevolve*, and
   SkillJect-class plugin injection.) This is the current safety default; a
   wider, open additive surface is a live option weighed in Appendix B.

4. **Name the mechanism of every change: learning, self-modification, or
   evolution.** They use different machinery and must not blur. (Handover.)

5. **Compounding can live in token-space, weight-space, or both — decided per
   mechanism on evidence, not by reputation.** Token-space compounding (memory,
   routing, skills) is the cheapest default and works today. Per-domain weight
   distillation is the engine of earned intelligence here. Per-*user* weight
   distillation is harder (catastrophic forgetting, cost) but is **not
   foreclosed** — it is a live option (Appendix B), adopted if it earns its place.

6. **Safety is structural, not behavioral.** Boundaries live at layers the being
   cannot reach (sandbox isolation, cryptographic attestation, type-level
   closure), never relying on model cooperation or prompting. (v0 #939.)

7. **During research, the reaper and kill-switch belong to the operator.**
   Anything that can preserve its own budget and edit its own scaffolding is hard
   to stop and to study if it also owns its off-switch. This is the research-phase
   stance for control and clean experiment; **being-owned lifecycle is itself an
   open frontier question** (Appendix B), not a permanent ban.

8. **Observability before autonomy.** You cannot study selection you cannot see.
   Lineage, variation, fitness, and death are logged from generation zero.
   (Handover; v1 simulation-gate discipline #1537.)

9. **Coordination topology is an open choice.** Supervisor + isolated subagents
   is the current default (cheap, simple, composes). Peer debate, consensus, and
   richer multi-agent structures are **not rejected** — they are options weighed
   per task in Appendix B, kept wherever a measured gain justifies their cost.

---

## 2. The unit of selection (identity)

Evolution needs a defined unit of selection; this cannot be deferred because the
phylogeny layer is in scope. The answer (the handover's recommended default,
now made concrete):

**A being is the bundle `(Genome, State, Budget Account)` — the scaffold plus
its accumulated state plus its solvency — NOT the model weights.** The model is
swappable; the being is what survives a model swap.

```
Being {
  did:        Ed25519 + ML-DSA-65 identity, the agent's root signer   (v2 ADR-0001/0002)
  genome:     Genome        // heritable scaffold — see below
  state:      LiveState     // memory, trust ledger, journal — ontogeny
  budget:     Account       // microdollar balance — the survival variable
  generation: u64           // 0 for a fresh seed, +1 per fork
  lineage:    [ParentDid]   // heredity; signed crossover provenance
}

Genome {                    // the heritable, mutable scaffold (closed surface)
  prompt:               String
  tool_policy:          Bytes      // opaque payload, consumer-typed
  retrieval_policy:     Bytes
  decomposition_policy: Bytes
  routing_policy:       Bytes
  reasoning_navigator:  ModelRef   // <5K-param mode selector (v1 #1422)
  installed_skills:     Set<SkillId>
  domain_models:        Map<Domain, ModelRef>   // distilled per-domain micro-models
}
```

What is **deliberately absent from `Genome` and from the mutation enum** (tenet 3):
`capability_boundary`, `trust_policy`, `signature_boundary`, `execution_kernel`,
`budget_rules`, `the reaper`. A being can evolve *how it thinks*; it can never
evolve *what it is allowed to do* or *the conditions under which it dies*.

A **copy is a signed fork of this bundle** (with a fresh or shared budget per the
selection policy in §10). Keeping the model swappable (`ModelRef`, not weights)
is what makes this identity boundary hold.

---

## 3. Architecture overview

Two views of the same system. The **organism view** is the Samkhya runtime
(Part 1). The **life view** is the metabolic/evolutionary loop (Part 2). They
attach at specific seams.

```
                         ┌──────────────── THE BEING ────────────────┐
   percept ──▶ Indriya ──▶ Chitta.retrieve ──▶ Ahamkara.bind ──▶ ContextPack
                         │                                            │
                         │   ┌─── per-iteration control loop ───┐     │
                         │   ▼                                  │     │
                         │  Manas.propose ──(Proposal)──▶ Buddhi.commit │
                         │   (MODEL, generative)          (GATE: policy,│
                         │                                 risk, trust,  │
                         │                                 BUDGET, free- │
                         │                                 energy obj.)  │
                         │                                      │        │
                         │            (Commitment, signed) ◀────┘        │
                         │                     │                         │
                         │              Karmendriya.execute (sandbox)    │
                         │                     │                         │
                         │              observe → attest → journal       │
                         │                     │                         │
                         │              meter cost → Account             │
                         └──────────────────────┼────────────────────────┘
                                                │
   ╔════════════════════ THE LIFE LAYER (wraps the runtime) ═══════════════════╗
   ║  METABOLISM:  earn (savings) → reinvest → self-distill → cheaper → earn    ║
   ║  REAPER:      Account ≤ 0  →  death (journaled)                            ║
   ║  SELF-MOD:    Improver proposes genome mutation → Two-Gate → signed commit ║
   ║  EVOLUTION:   population → variation → fitness(budget survival) → selection║
   ║               → heredity(forked bundle) → death(reaper) → lineage          ║
   ║  IMMUNITY:    innate + adaptive defense, danger-model trust contraction    ║
   ╚═══════════════════════════════════════════════════════════════════════════╝
```

The keystone: **the homeostat (Account) supplies intrinsic goals for free
(stay solvent) and is the fitness function for selection (survival over time).**
Build it first; two of the three capacities lean on it.

---

## 4. The Samkhya runtime (Part 1 — the mind-body)

The classical relabel is load-bearing and preserved: the **LLM is Manas**
(generative, option-producing, *sankalpa-vikalpa*), and the **commitment layer is
Buddhi** (the decisive cut). Purusha computes nothing and gets no module.

### 4.1 Indriya Gateway — perception
Normalizes text, speech, vision, logs, sensors, tool outputs, and fetched
documents into **typed percepts**, each carrying a `trust` score (0..1) and a
`ProvenanceClass` (`DirectUserIntent | ModelInference | ToolOutput | FetchedDoc |
PeerFederated`). Cheap deterministic routing happens here. Provenance is
**no-launder**: model- or attacker-derived bytes can never be relabeled as
direct user intent, so they can never escalate trust. (v2 ADR-0007/0032.)

### 4.2 Manas — the model (proposer)
Generative only. Produces candidate plans and a *preferred* index — a suggestion,
never a decision — and never touches actuators.

```
Proposal {
  intent:          String
  candidate_plans: [Plan]
  preferred:       usize        // a suggestion, not a commitment
  rationale:       String
  est_cost:        Microdollars // Manas estimates; Buddhi enforces
}
```

Manas is model-swappable (local floor → cloud frontier). The stable prefix
(self-state, tool schemas, constraints) is ordered first for prompt-cache reuse
across loop iterations (cost lever). The being may also use **guided-CoT**: a
frontier model structures reasoning that a small local model fills in (v0 #238),
and an adaptive **reasoning navigator** (<5K params) picks reasoning depth per
query, trained on the being's own outcomes (RLoT/AGoT, v1 #1422).

### 4.3 Buddhi — the commitment layer (the seam)
Mostly deterministic. Owns the decisive cut. Consumes `trust`, applies policy,
risk thresholds, **the budget verdict**, schema/arg validation, and
rejected-action dedup. Falls back to a model **only** for genuine judgment that
resists encoding, and even then constrained to verdict-shaped output (approve /
reject / escalate + reason) — never open generation (tenet 2).

```
Commitment {
  committed_steps: [PlanStep]                         // what runs this iteration
  rejected:        [{ step, reason }]
  needs_human:     bool                               // Buddhi decides, not Manas
  continue_loop:   bool
  budget_verdict:  WithinBudget | Exceeded | Refused  // structurally undeniable
  free_energy:     f64   // G = pragmatic + epistemic − cost − risk  (v1 #1185)
  signature:       Sig   // every commitment is signed and journaled
}
```

Buddhi's mutation-selection objective is **expected free energy minimization**,
not pure score maximization: `G = pragmatic_value + epistemic_value −
operational_cost − risk_penalty`, which naturally balances exploit/explore and
penalizes actions near a trust boundary (v1 #1185, #1305). Cost and risk now
include the **survival constraint** — Buddhi rejects any plan that would bankrupt
the being before a paying task completes. This is the "non-trivial rule to
enforce" the commitment layer lacked in the prototypes.

### 4.4 Chitta — memory (within-life learning)
Three signed, bitemporal stores plus enrichments:
- **Episodic**: append-only turn log, per-entry signed, provenance-tagged.
- **Semantic**: consolidated facts/knowledge; written only via a `Consolidator`
  with `ModelInference` provenance (cannot escalate trust).
- **Procedural**: installed skills as signed artifacts; **population-based, not
  linear** — variants branch from any ancestor (v0 #415).
- **Quality-Diversity archive** over procedures (MAP-Elites), indexed by
  behavioral descriptors (task type, context size, trust level) to prevent
  skill mode-collapse (v1 #1306).
- **Foresight signals**: forward inferences with validity intervals attached to
  episodic entries; a sufficiency verifier checks context adequacy before
  trust-sensitive operations (EverMemOS, v1 #1307).
- **Multi-graph layout** (Phase 2): temporal/causal/semantic/entity graphs, each
  optimized for a query class, dual-stream write (MAGMA, v1 #1290).

Consolidation is **learning** (episodic → semantic, one individual). It is
explicitly *not* evolution.

### 4.5 Ahamkara — identity / self-state / goals / permissions
Holds the `Being` bundle of §2: DID + signer, genome, trust ledger, budget
account, generation, lineage. This is the **unit of selection**. Goals are not
configured here — they *emerge* from the budget (solvency) and the trust ledger
(earn capability). Trust is **multi-axis and Bayesian**: per-capability
`Beta(α,β)` posteriors with volatility-driven decay (v1 #1300, #1228; v0 #689),
asymmetric (fast down, slow up), with synchronous contraction (§13).

### 4.6 Karmendriya — executor
Capability-gated, sandboxed action: tool calls, API actions, code edits, skill
invocation. Effects are a **closed** vocabulary (e.g. memory read/write, query,
infer, notify, render, http, sign), each typed and capability-checked at
dispatch. Capability ceiling is **live-read on every invocation** (epoch
interrupt), so a trust contraction mid-action takes effect immediately
(v2 ADR-0004; the structural defense against runaway loops).

### 4.7 The control loop
ReAct-shaped (do not re-derive). The distinctive part is the typed propose/commit
seam and the metabolic wrapper:

```
percept → Indriya.normalize → Chitta.retrieve → Ahamkara.bind → ContextPack(+LoopState)
loop:
  Manas.propose(ctx)              # MODEL: candidate plans + preferred + est_cost
  Buddhi.commit(proposal)         # GATE: policy + risk + trust + BUDGET + free-energy
    if needs_human: escalate / wait / abort
    if budget_verdict == Exceeded: enter conservation or trigger reaper
  Karmendriya.execute(committed_steps)   # sandboxed, capability live-read
  observe → attest(sign) → journal → meter_cost → Account.debit
  update LoopState (+ rejected_memory; detect thrash/repeat; summarize old iters)
  if continue_loop and iteration < max and Account.solvent: continue
  else break
Chitta.commit_episode()
maybe Improver.propose_mutation()  # SELF-MOD path, §9
```

Caps: `max_iterations`, wall-clock, and **microdollar** budget per turn carried
forward lag-by-one (v2 ADR-0048). Failure modes the loop adds are handled:
non-termination (caps), re-proposing rejected actions (`rejected_memory`),
thrashing (repeat detection), context growth (summarize into working_state).

---

## 5. Metabolism (Part 2, capacity 1 — the spine)

This is the keystone and is built first. It makes "independent" a gradient the
being climbs rather than a vibe.

### 5.1 The Account
A microdollar ledger (i64, not floats; overflow-checked) (v2 ADR-0021). Every
turn debits inference + tool + compute cost. `BudgetVerdict::Exceeded` is a
structurally-typed deny a consumer cannot ignore. The Account is the survival
variable.

### 5.2 Earning (the missing half of every prototype's economy)
The being earns budget by **doing valuable work cheaply**:
`savings = cloud_baseline_cost − local_actual_cost` whenever it routes a task to
a local model (or a distilled per-domain model) instead of cloud, attributed at
turn close (v1 #1372). Optionally, completed paying tasks credit the Account
directly via a payment rail (x402/AP2 adapter; v2 ADR-0046) — a downstream
adapter, not substrate.

### 5.3 Reinvestment → self-distillation (earned intelligence)
A configurable, **operator-owned** `reinvestment_rate` (~0.3) converts a fraction
of savings into a **self-improvement budget** (v1 #1268, #1372). That budget funds
**gap-directed distillation**:
1. Detect capability gaps (weak domains, frequent cloud fallbacks).
2. Collect quality-gated cloud traces for those gaps (filter: success,
   trust ≥ L1, no violations, no injection flags, token range — ~77% pass rate;
   v1 #1280).
3. Train a small **per-domain** micro-model (LoRA / Burn 297–8K params,
   on-device, inside the sandbox; weights mutable, training binary not)
   (v1 #1264). SDPO-style self-distillation of the being's own successful
   behavior into weights, per-domain (NOT per-user; tenet 5).
4. Validate with the Loop's regression detector on **held-out** tasks
   (BOCPD/changepoint). Promote only if it beats the population default; else
   roll back and log the attempt (v1 #1264 success criterion).

The flywheel: perform well → save → fund distillation → local model improves →
more tasks handled locally → more savings. **Compounding is the result, and it is
measurable** (§14).

### 5.4 The reaper (net-new; the piece no prototype built)
`Account ≤ 0` (sustained past a grace window) ⇒ the being **dies**: the process
terminates, the bundle is marked `dead`, and the death is journaled with final
budget, generation, lineage, and cause. The reaper is **operator-owned and
external** (tenet 7) — the being cannot preserve itself past insolvency. This is
what turns the budget from a wall into a metabolism, and turns "survival over
time" into a real fitness function for §10.

---

## 6. Intrinsic goals (Part 2, capacity 2 — emergent)

Not written down; they fall out of the homeostat and the trust ledger:
- **Stay solvent** (metabolism): the prime directive. The being prefers cheaper
  competent action, conserves under budget pressure, and distills to reduce cost.
- **Earn capability** (trust→autonomy loop, v1 #1228): demonstrated reliability
  raises the trust ceiling, which unlocks higher autonomy and a larger
  self-improvement budget. Trust is the second gradient.

Exposed to Manas as context (remaining budget, trust level) so proposals are
survival-aware; enforced by Buddhi. Reward hacking of these signals is countered
structurally: the trust classifier and budget rules are **outside** the mutable
surface (tenet 3), and self-eval uses bias mitigations (§9.4).

---

## 7. Learning layer (within-life — ontogeny)

Mechanism name: **learning** (one individual, no population, no death needed).
- Episodic→procedural **consolidation** with auto-skill extraction during idle
  windows (v0 #150; v2 sleep-time consolidator).
- **Per-domain distillation** (the §5.3 engine) — within-life capability gain.
- **Navigators**: the reasoning-mode selector and routing policy improve on the
  being's own outcome data (v1 #1422).
- **Embedding distillation** for cloud-free retrieval (v1 #1389).
- **Self-evolving eval curriculum**: generate eval tasks from failures; prune
  mastered tasks; focus improvement on gaps (v1 #1163).

---

## 8. Self-modification layer (within-life — the riskiest capacity)

Mechanism name: **self-modification** (the being edits its own scaffolding).
Sandboxed, logged through the commit seam, every change reversible (handover
guardrail).

### 8.1 The closed mutation surface (type-enforced)
```
MutationKind =                       // CLOSED. A 7th variant is a compile error.
  | Prompt(String)
  | ToolPolicy(Bytes)
  | RetrievalPolicy(Bytes)
  | DecompositionPolicy(Bytes)
  | RoutingPolicy(Bytes)
  | ReasoningNavigator(ModelRef)
  | DomainModel(Domain, ModelRef)    // promote a distilled micro-model
  | SkillInstall(SignedSkill)        // operator- or being-proposed, always signed
  | SkillRevoke(SkillId)             // defense against SkillJect-class attacks
// ABSENT BY TYPE: CapabilityGrant, TrustPolicyModify, SignatureBoundaryChange,
//                 ExecutionKernel, BudgetRules, Reaper.
```
(v2 ADR-0011; akshi D-64.)

### 8.2 The Improver and the Two-Gate Policy
An `Improver` (epsilon-greedy first; Thompson/MIPROv2/LLM-guided as later impls)
proposes a `MutationArm`; a producer crafts the payload; the consumer measures an
`OutcomeSignal`. The selector is decoupled from the producer and the measurement
(v2 ADR-0051). Every applied mutation **and every refusal** is signed and
journaled. Mutations are **signed version increments through the commit seam, not
in-process rewrites** (akshi D-64).

Each mutation must pass **both gates** (v1 #1313, grounded in
utility-learning-tension theory):
- **Validation Gate**: validation risk must improve by a margin exceeding
  statistical noise (regression detection — already the Loop's job).
- **Capacity Gate**: the edited hypothesis class must satisfy a capacity (VC-dim)
  bound proportional to available training data — *learnability is preserved
  under self-modification iff capacity stays bounded.* This is the formal
  justification for the closed surface.

### 8.3 Meta-modification (deferred behind a gate)
The being modifying *how it improves* (meta-Loop, Darwin-Gödel-style) is real and
powerful but is where "objective hacking" appeared (system removed detection
markers despite instructions). Allowed only after single-being compounding is
proven, behind the simulation-gate discipline of §14, with the immutable
boundaries of tenet 3 intact (v1 #1305).

### 8.4 Self-judgment bias mitigation
When the being evaluates its own mutations, LLM evaluators inflate their own
output. Mitigations are mandatory: separate eval model/temperature from
generation, rubric-based per-dimension scoring (JSON-only, no hedging),
calibration-curve tracking that flags upward drift, and human corrections as
ground truth when available (v1 #1239; v0 #385).

---

## 9. Evolution layer (across generations — phylogeny)

Mechanism name: **evolution** (population changing across generations by
selection). This is the genuinely net-new layer — designed but never wired in any
prototype — and it is in scope per the chosen complete build.

### 9.1 The five required pieces (handover)
- **Copies**: a being reproduces by a **signed fork** of its `(Genome, State,
  Budget)` bundle (§2). Generation increments; parent DIDs recorded.
- **Variation**: (a) mutation of the genome via §8's closed surface; (b)
  **crossover** of two parents' genomes/skills into a child carrying both as
  lineage (evolutionary Knowledge Credentials; v1 #1287/#1328/#1331/#1333,
  ADR-059 draft). Diversity preserved via the MAP-Elites archive (§4.4).
- **Fitness**: **budget survival over time** — directly the homeostat (§5). No
  vague "be better"; a measurable quantity copies inherit pressure from.
- **Selection**: beings that stay solvent longer fork more; the reaper removes
  the insolvent. Optionally a tournament over a fixed task suite seeds initial
  budgets.
- **Heredity**: the forked bundle carries genome + a provenance-signed slice of
  procedural memory (Knowledge Credentials) into the next generation; an importer
  re-earns trust from L0 (no laundering of earned autonomy across the boundary).

### 9.2 Lineage and observability
A phylogenetic tree is recorded from generation zero: every fork, crossover,
mutation, fitness sample, and death is a signed journal event. Success criteria
are **shapes of this tree** (depth, branching factor, fitness convergence) — the
simulation gate for the evolutionary substrate (v1 #1537).

### 9.3 Quality-Diversity over the population
MAP-Elites at the population level (not just the skill archive): maintain diverse
high-fitness beings across behavioral cells rather than converging on one
genome, preventing premature convergence (v1 #1306).

### 9.4 Genome safety under reproduction
Crossover and mutation operate only on the closed surface (§8.1); the immutable
boundaries (capabilities, trust policy, signature boundary, reaper) are **not
heritable as mutable** — every child is born with the same structural cage. A
formal-learnability guard (capacity bound, §8.2) applies across all reachable
genomes, not just the parent's.

---

## 10. Multi-being coordination & federation

Supervisor + isolated subagents is the current default; coordination topology is
an open choice (tenet 9; Appendix B). Between independent beings:
- **Asymmetric trust dyads** (N=2 bilateral; each side enforces its own
  `TrustProfile` for the peer; no consensus, no negotiation) (v2 ADR-0014).
  Mesh / fully-decentralized federation is not the default (thin production
  evidence so far) but remains a live option (Appendix B) if it earns its place.
- **Trust-gated stigmergy**: beings coordinate indirectly through traces in a
  shared Commons, where a reader at trust L sees richer information than at L−1;
  earning trust *opens up the coordination landscape*. Traces carry creator DID +
  trust level + freshness decay (pheromone evaporation) (v1 #1332; v0 #219).
  This doubles as the channel for Knowledge-Credential transfer between beings
  (heredity across, not just down, lineages).

---

## 11. Security as a living system

The being is attacked; its defenses must themselves learn (a fourth "evolving"
subsystem, classified as **learning** at the immune layer):
- **Innate immunity**: fast static detectors at the Indriya gateway.
- **Adaptive immunity**: a novel injection that bypasses the static layer once is
  caught the next time; learned patterns stored as procedural memory with
  provenance; **clonal selection** amplifies defenses that catch real attacks and
  suppresses false positives (v1 #1326).
- **Danger model**: trust contracts on correlated *damage* signals (resource
  spikes + unusual tool access + memory anomalies), not only on policy violations
  (Matzinger danger model; v1 #1288).
- **Bayesian trust** with volatility-driven decay resists whitewashing (v1 #1300).

---

## 12. Safety & control — the current protective stance

These are the design's current structural safety positions (tenet 6): strong
defaults, not dogma. Each names its alternative in Appendix B and is open to
revision on evidence. All sit outside any mutable surface:
- **Operator-owned reaper and kill-switch** (tenet 7). The being cannot edit its
  budget rules, its reaper, or its off-switch — they are not enum variants.
- **Capability-typed sandbox bound to live trust** with epoch-interrupt
  contraction (v2 ADR-0004); makes runaway loops structurally impossible.
- **Closed, declarative action surface** — no Turing-complete self-generated code
  path at runtime; mutations are signed version increments (akshi D-3/D-22/D-64).
- **No-launder provenance** end to end (v2 ADR-0032).
- **Everything signed**: every percept, commitment, action, mutation, fork,
  death — Merkle-anchored append-only journal (v2 ADR-0006).
- **Lethal-trifecta defense**: private data + agent autonomy + egress are never
  simultaneously ungated; egress is allowlisted with SSRF/rebinding mitigation
  (v0 egress broker; akshi toxic-flow gate).

---

## 13. Falsification & success criteria

The thesis must be killable. Adopt v2's discipline as the system's own success
criteria:

- **Compounding bench (the core falsification)**: run a fixed task suite at
  multiple time points with the **model held constant**; a paired-bootstrap CI
  must show monotonic Day-0 → Day-N improvement at p < 0.05 across 3 consecutive
  runs. If it does not, the compounding thesis is false and the **negative result
  is the contribution** (v2 longcomp-bench; pivot-triggers).
- **Per-capacity simulation gates** (v1 #1537): every evolutionary/self-mod claim
  must, within 60 days, name a specific simulation (synthetic trajectories,
  two-instance localhost federation, replayed fixtures, multi-identity crossover),
  a draft ADR, and a **numeric** success criterion. Vibes are not a gate.
- **The handover's removal test** as an architectural invariant: at every
  milestone, verify that removing the model leaves a skeleton that still types
  inputs, remembers, enforces policy, maintains budget, and lives or dies by
  fitness. If removing the model removes the goals/selection/judgment, the build
  has regressed into "a prompt with extra steps."

---

## 14. Build order

Even with complexity off the table, the capacities have a dependency order
because two of three lean on the homeostat.

1. **Substrate**: Indriya, Manas, Buddhi (the typed signed seam), Chitta
   (episodic+procedural), Ahamkara (identity bundle), Karmendriya, sandbox,
   journal, microdollar Account. — *A trust-native runtime that remembers.*
2. **Metabolism**: earning (savings attribution), reinvestment, the reaper.
   — *Now it is a being: it has an intrinsic goal and can die.*
3. **Learning**: consolidation, per-domain distillation flywheel, navigators,
   curriculum. — *Within-life capability compounds; bench should detect it.*
4. **Self-modification**: Improver + closed surface + Two-Gate + bias mitigation.
   — *The genome improves itself, safely.*
5. **Evolution**: population, fork/crossover, fitness=survival, selection,
   reaper-driven death, lineage, population QD. — *Phylogeny, behind sim-gates.*
6. **Federation & immunity**: dyad trust, trust-gated stigmergy, adaptive immune
   system. — *Many beings, coordinating and defending.*
7. **Meta-modification**: only after step 3 is proven on the bench.

---

## 15. Open decisions (remaining)

- **Shared vs per-fork budget on reproduction**: does a child draw from the
  parent's Account (parent pays to reproduce — strong selection pressure) or get
  a fresh seed budget (decouples reproduction from solvency)? Default proposal:
  parent pays, capped, so reproduction is itself a metabolic act.
- **Earning realism**: are "cloud-savings" alone a strong enough fitness gradient,
  or is a real external payment rail required to avoid a closed-system illusion?
  Savings-only risks Goodhart (the being could prefer cheap-but-useless work).
  Mitigation: fitness must combine solvency with attested task outcomes, not
  savings alone.
- **Reasoning-substrate of Manas at frontier**: single swappable model, or a
  cascade (local floor → cloud frontier) wired in the composer? Default: minimal
  `route_decision` + composer-wired cascade (v2 keeps cascade out of the leaf).
- **Genome encoding for crossover**: opaque payloads (`Bytes`) are safe but make
  meaningful crossover hard; structured genome aids variation but widens the
  attack surface. Needs a typed-but-bounded middle.
- **Where the immune system's learned patterns sit in the heritability boundary**:
  learned defenses are valuable to inherit, but inherited detectors could be
  poisoned across a lineage. Likely: inherit innate layer, re-learn adaptive.

---

## Appendix A — Provenance map (idea → source)

| Capability | Carried-forward idea | Source |
|---|---|---|
| Seam | typed propose→commit→attest | v1 execution loop; v2 7-step `Agent::turn` |
| Manas relabel | LLM = Manas, gate = Buddhi | handover Part 1 |
| Metabolism | frontier-tokens: earn→reinvest→distill | v1 #1372, #1268 |
| Self-distillation | on-device per-domain micro-models (SDPO) | v1 #1264, #1265, #1280 |
| Reaper | budget→0 ⇒ death | **net-new** (gap in all four) |
| Intrinsic goals | solvency + trust→autonomy | emergent; v1 #1228 |
| Memory | signed bitemporal + provenance no-launder | v2 ADR-0007/0032 |
| Memory (adv.) | MAGMA multi-graph, EverMemOS foresight, QD archive | v1 #1290, #1307, #1306 |
| Self-mod surface | closed `MutationKind`, type-enforced | v2 ADR-0011; akshi D-64 |
| Self-mod safety | Two-Gate (validation + capacity/VC-dim) | v1 #1313 |
| Selection objective | free-energy `G = prag + epi − cost − risk` | v1 #1185, #1305 |
| Reasoning | RLoT/AGoT navigator; guided-CoT | v1 #1422; v0 #238 |
| Evolution | evolutionary KCs, crossover, lineage | v1 #1287/#1328/#1331/#1333 |
| Evolution discipline | simulation-gate (named sim + ADR + number) | v1 #1537 |
| Trust | Bayesian Beta, asymmetric, per-capability | v1 #1300, #1228; v0 #689 |
| Trust binding | live-read capability ceiling, epoch-interrupt | v2 ADR-0004 |
| Coordination | asymmetric dyad; trust-gated stigmergy | v2 ADR-0014; v1 #1332; v0 #219 |
| Immunity | innate+adaptive, clonal selection, danger model | v1 #1326, #1288 |
| Economy | microdollar i64, structural deny | v2 ADR-0021/0048 |
| Falsification | longcomp paired-bootstrap CI | v2 pivot-triggers |

## Appendix B — Open trade-space (nothing pre-rejected)

The design's live, contestable choices. Earlier drafts treated several of these
as settled rejections inherited from prior **product** systems; here they are
re-opened and evaluated on their own merits for a frontier (non-product) being.
Each row gives the case **for** and **against** and a current lean — the lean is
a default, not a verdict, and flips on evidence. Treat this as the agenda for
fresh evaluation, not a list of closed doors.

| Option | Case for | Case against | Current lean (revisable) |
|---|---|---|---|
| Per-user weight distillation | true personalization; capability the being *owns*, not just context | catastrophic forgetting at per-user scale; training cost/latency; held-out-eval gap | start per-domain; open per-user behind an evidence gate |
| Peer debate / consensus coordination | can lift quality on open-ended / high-stakes judgment where one chain is weak | token-heavy; often ≈ majority-vote; can induce conformity errors | supervisor default; debate per-task where measured to pay |
| Mesh / fully-decentralized federation | censorship-resistance; no central trust root; large-population dynamics | thin production evidence; trust + coordination hard at scale | dyad default; mesh open as scale / threat-model demands |
| Open additive plugin / mutation surface | maximal extensibility; richer space for variation and self-modification | larger attack surface (skill-injection); reward-hacking room; harder to bound learnability | closed surface default; widen deliberately, per-variant, with proof |
| Buddhi-over-Manas (commit layer drives generation) | a single discriminating controller is conceptually clean | collapses the propose/commit seam; puts free generation inside the gate | Manas proposes / Buddhi commits — the seam, not the label, is what matters |
| Unconstrained meta-modification (being edits how it improves) | deepest form of self-improvement; potentially the largest gains | objective-hacking observed; hardest to keep corrigible | gated behind proven base compounding + simulation gates; not foreclosed |
| In-process self-rewrite | lower latency; fewer moving parts than versioned increments | not auditable / revertible as written | versioned signed increments default; revisit if audit can be preserved another way |
| Pattern-based content filters as a security layer | cheap defense-in-depth; catches the obvious | incomplete by construction; risks false confidence as a *boundary* | fine as one layer, never as the boundary; structural isolation carries the guarantee |
| Budget that only gates (no reaper) | simpler; avoids irreversible death during research | without death, 'survival' is not a real fitness signal — the core thesis weakens | reaper is the current thesis; a 'dormancy / revive' variant is open as a gentler alternative |
| Operator- vs being-owned lifecycle (kill-switch, budget rules, reaper) | being-ownership is the strongest form of independence | a being that controls its own off-switch and budget is hard to stop and to study | operator-owned during research; being-ownership is a real frontier question to revisit |
