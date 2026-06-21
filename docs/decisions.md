# Yogi — Decisions (append-only)

Design decisions, especially for `⚠` safety-critical milestones, surfaced for review per
`CLAUDE.md`. Each: the decision, why, citations (web-researched), and the revisit trigger.

---

## D-M1-1 — Reaper: irreversible Death over a preserved body; operator-only reseed

**Decision.** On insolvency the **operator-owned** reaper fires — external, unforgeable, and
**agent-uninterceptable** (the being has no API to observe-and-veto or block it). It performs a
**graceful-then-brutal** stop (bounded drain window → forced termination) and journals an
**irreversible `Death`** event (one-way; the being cannot launder itself back to `alive`). The dead
bundle's state is snapshotted to cold storage (inspectable) but does not run, hold a population slot,
or consume budget. **No in-runtime undeath and no agent-revivable dormancy.** An operator may reseed
a **new** bundle (new identity, fresh Account) from a corpse; the original Death stands.

**Why.** Death is required for selection only in the population-slot sense, not process-destruction.
Irreversibility *under operator control* is safer: the off-switch game shows agent-controlled
reversibility invites exploit/resistance, and frontier models empirically resist shutdown up to ~97%
of trials — so the kill must be external and unforgeable regardless of death-vs-dormancy. Preserving
the corpse keeps research inspectability (autopsy, diff, replay). Mirrors Erlang/OTP
graceful-then-brutal and the OS OOM-killer.

**Citations.** Off-switch game (arXiv 1611.08219); shutdown resistance (arXiv 2509.14260); mortal
agents / death-as-meta-goal (arXiv 2411.12304); OTP supervisor docs; CRIU (checkpoint/restore).

**M1 builds:** external insolvency trip + graceful-then-brutal stop + irreversible `Death` event.
Snapshot/reseed = preservation layer; CRIU-grade live restore deferred. **Revisit:** M6 selection-arm
semantics about emergent self-preservation.

---

## D-M1-2 — Economy: single ledger, maintenance-first, reserve floor + per-bet cap

**Decision.** One survival **Account** (microdollar `i64`). All spend — operating **and** investment
(exploration / distillation / reproduction) — debits it. Maintenance is serviced first; investment
spends only from surplus above a configurable `reserve_floor`, and is capped per charge by
`max_investment_per_charge`. Spend **categories are telemetry tags, not separate spendable budgets.**
A two-ledger variant stays behind a feature flag, **default OFF**, for the M6 bench to A/B.

**Why.** A walled-off investment budget is a *soft budget constraint* → moral hazard and tolerated
negative-EV bets, and it severs the feedback that makes "survived" a valid fitness signal (causal
Goodhart: survival stops being *caused* by good total resource decisions). Single-ledger keeps the
survival signal incentive-compatible and causally valid; biology uses one energy budget with a
priority ordering. The reserve floor + per-bet cap recover most of two-ledger's protection (no
death-by-single-rational-bet) **without** softening the constraint.

**Citations.** Kornai soft budget constraint (Kyklos 1986; Roland/Berkeley); principal–agent moral
hazard; Goodhart in RL (arXiv 2310.09144); "Survival is the Only Reward" (arXiv 2601.12310); cellular
resource allocation (PNAS E6457; Current Biology). **Revisit:** M6 A/B of ledger topology.

---

## D-M1-3 — Supervisor: in-process reference monitor, process-split-ready, out-of-band kill now

**Decision.** The M1 supervisor is an **in-process Rust module** behind a single `SupervisorPort`
trait façade. Authority state (balance counter, kill flag) is **private** — no `pub` fields, no raw
accessors; being-code holds only the trait handle. The **out-of-band watchdog kill path is built now**
(a separate thread/flag the being must periodically feed; independent of the executor loop, so a
wedged being is still killable). A regression test asserts being-code **cannot name** the authority
types.

**Why.** Reference-monitor criteria: always-invoked, tamper-proof, small-enough-to-verify. In-process
satisfies always-invoked + verifiable; "tamper-proof" is not yet *exercised* at M1 because no
untrusted or self-modifying code shares the address space — so it is honest as a **discipline**
boundary (not a security boundary), designed so the later split is a transport swap, not a rewrite.
The out-of-band kill is the safety escape hatch worth over-building.

**HARD UPGRADE GATE.** Move to **separate-process privilege separation** (OpenSSH-style) + a
**Wasmtime capability sandbox** for tool/executor code **before M4 self-modification lands, or before
any model-generated/native code executes — whichever comes first.**

**Citations.** Reference monitor (NIST SP 800-53 AC-25; Anderson); OpenSSH privsep; Wasmtime security
model; in-process isolation limits (USENIX Sec'23, arXiv 2306.08127); watchdog-timer pattern.

---

## D-M3-1 — Compounding leads in token-space (memory), not weights

**Decision.** M3's primary, always-on compounding mechanism is **token-space**: episodic→semantic
**consolidation** + **embedding retrieval** (the `nomic-embed-text` shared embedding). Per-domain
**weight distillation** is a *secondary, optional, foreground-only* arm — a small student
(≤ ~1.7B, e.g. a small qwen3) trained via MLX LoRA with `qwen3:8b` as teacher, loaded sequentially,
never concurrently — and is **deferred** until token-space compounding is shown to saturate on the
bench.

**Why.** On 16 GB unified memory, LoRA-training `qwen3:8b` is infeasible (the fp16 weights alone are
~16 GB; QLoRA + optimizer state + activations does not fit alongside the OS). Small-student training
is possible but heavy and occasional, so it cannot be the everyday path. Token-space compounding
needs no training, is loop-safe (the embedding model is ~0.3 GB and is still only ever called
foreground/runtime, never in `cargo test`/hooks), and can produce a measurable Day-N bench signal
first. This matches architecture §15 (token-space compounds today; weight-space is the harder bet)
and CLAUDE.md's one-model-at-a-time / no-inference-in-the-loop rules.

**Status.** Confirmed + sharpened by web research (citations below).

**Key refinements from research — retrieval and verifier-fed skills are the real levers:**
- **Retrieval quality dominates consolidation.** LongMemEval: the same fixed model loses 30–60% when
  forced to read full history vs. oracle retrieval; retrieval-stage optimizations contribute far more
  than ingestion. ⇒ build retrieval first; consolidation exists to make retrieval *cleaner*, not to be
  the win itself.
- **The decisive compounding lever is skill-learning with a verifier signal.** Letta Skill Learning:
  +9% absolute on a fixed model from trajectory-derived skills, rising to **+15.7% when error/verifier
  feedback is folded in.** Yogi already has that verifier — the M2 bench pass/fail. Wire it in.
- **Local RAG specifics (`nomic-embed-text`):** 768-dim, cosine on L2-normalized vectors; hybrid
  dense+BM25 via reciprocal-rank fusion (+15–30% recall, esp. code/identifiers); recency prior
  `score = α·cos + (1−α)·0.5^(age/half_life)`, α≈0.7, half-life≈14d (α≥0.9 surfaces stale facts).
- **Failure modes to encode as tests:** stale-but-similar (require the temporal prior; never ship pure
  cosine), retrieval drift as memory grows (no naive FIFO eviction), fact supersession (test
  knowledge-update queries explicitly).

**M3 build order (loop-safe; retrieval-first):** (1) semantic-retrieval core — cosine + recency-prior
vector index, pure + tested; (2) generic `Embedder` (live `nomic-embed-text` behind a feature,
foreground) + hybrid BM25/RRF; (3) `Consolidator` (episodic→semantic; deterministic core, model
variant behind a feature); (4) skill-learning loop fed by the M2 bench verifier; (5) wire retrieval
into the turn + a Day-N bench demo vs. a no-memory baseline. Distillation = a later optional foreground
tool (D-M3-2).

**Citations.** LongMemEval (arXiv 2410.10813); Letta Skill Learning + Continual Learning in Token
Space; Mem0/LoCoMo/BEAM (mem0.ai); A-MEM (arXiv 2502.12110); nomic-embed-text (HF; arXiv 2402.01613);
recency prior + drift limits (arXiv 2509.19376).

---

## D-M3-2 — Weight distillation: small-student MLX QLoRA, foreground & gated

**Decision.** When distillation is warranted (a domain proven valuable on the bench), it is a **rare,
foreground, user-initiated** operation: teacher `qwen3:8b` (Ollama) generates quality-filtered traces
→ `ollama stop` → train a **small student via QLoRA in MLX-LM** → fuse → **serve via MLX directly**.
Default student **Qwen3 1.7B** (~6 GB peak, ~8 min/domain LoRA); 0.6B for cheap domains; 4B as a tight
upper bound (~11 GB). Never train the 8B; teacher and student are **never co-resident** (16 GB).

**Why.** Real Mac LoRA peaks: 0.8B ≈ 3.9 GB, 2B ≈ 5.9 GB, 4B ≈ 11.1 GB; 8B QLoRA only "fits" with
almost no headroom — fragile for a loop. MLX-LM is the only first-class Apple-Silicon LoRA path
(unsloth is CUDA-only; PyTorch-MPS / candle / llama.cpp-finetune not viable). Qwen3 is **not** in MLX's
GGUF export, and HF→GGUF conversion of merged Qwen adapters has reported correctness regressions — so
serve the fused student **via MLX**, not an Ollama GGUF, unless a post-conversion behavioral validation
passes. Distillation is **latency/cost compression of already-proven token-space skills**, never the
source of new capability.

**Citations.** sciences44/mlx-lora-finetune (real Mac numbers); MLX-LM LoRA.md; InsiderLLM 16 GB table;
MLX-LM #353 / #1058 (GGUF export limits + Qwen conversion regressions); Agentic KD (arXiv 2602.10869).

---

## D-M4-1 — Bounded self-modification: Improver proposes, the bench alone judges

**Decision.** M4 adds an `Improver` that **only proposes** a closed-surface `MutationKind`; the
acceptance path is **pure deterministic machinery over recorded bench results — no model inference in
the loop** (this is both the CLAUDE.md rule and the bias-free configuration the self-preference
literature endorses). A proposed edit is committed only if it passes BOTH gates, then is audited and
reversible:
- **Validation Gate** (the 2510.04399 rule, literally): run the bench on incumbent vs. candidate over
  the *same* cases; accept iff candidate beats incumbent by **more than `2·ε_V + τ`**, where
  `ε_V ≈ √((K + ln(1/δ))/n)`. Use a **paired per-case (McNemar) test** on pass/fail (sensitive at
  small n), optionally an always-valid sequential bound to stop early without p-hacking.
- **Sealed holdout + adaptive-query budget:** the Improver sees a working bench split; a **sealed
  split it never sees** re-checks accepted edits before final commit (defeats the documented
  reusable-holdout overfitting). Sealed-set regression ⇒ rollback.
- **Capacity Gate** (the proxy the paper leaves open, made concrete for a declarative genome):
  `B[genome] = weighted Σ(prompt length, #skills, routing rows, decomposition depth, retrieval-k,
  per-edit edit-distance)`; accept iff `B[new] ≤ K[m]` with `K[m]` a **monotone schedule keyed to
  accumulated bench evidence**. The closed, non-`#[non_exhaustive]` `MutationKind` is the hard outer
  capacity bound (already a compile-time invariant).
- **Bias mitigation by construction:** the verifier is the pass/fail bench, never an LLM judge, in the
  acceptance loop. Any qualitative score is a *different* model family, foreground, rubric-based, and
  advisory — never gating.
- **Rollback + audit + post-accept safety re-check:** append-only log of every proposed/accepted edit
  (genome diff + bench evidence + B-value), a frozen last-known-good genome, one-call revert, and an
  automatic re-check (misevolution erodes safety *quietly* even through legitimate prompt/policy edits).

**Human-reviewed constants (⚠):** `τ` (stability margin), the capacity schedule `K[m]`, and the
holdout/refresh policy are surfaced for human review, not chosen silently.

**Why.** 2510.04399 proves PAC-learnability is preserved under self-modification **iff** reachable
hypothesis capacity stays bounded; the Validation+Capacity gate pair is its computable guardrail. The
dominant empirical risk is **not** an over-strict gate — it is a **gamed verifier / inflated
self-evaluation** (STOP's >1000%-accuracy hack; Shao et al. misevolution: ~55% refusal-rate erosion
via memory, >76% vulnerable tools). Yogi's closed surface + pass/fail bench already neutralize the
worst pathways; M4's job is the margin, the capacity proxy, the sealed holdout, and free rollback.

**Citations.** Two-Gate theory (arXiv 2510.04399); STOP (2310.02304); Darwin-Gödel Machine
(2505.22954); Gödel Agent (2410.04444); Misevolution — *Your Agent May Misevolve* (2509.26354);
self-preference bias (2410.21819); reusable holdout (1506.02629); OPRO (2309.03409) / EvoPrompt
(2309.08532) / PromptBreeder.

---

## D-M4-2 — Isolation timing: defer process-split + WASM to the executor boundary (amends D-M1-3)

**Decision.** **Do NOT do the process-split + Wasmtime upgrade before M4.** The in-process reference
monitor stays *honest* through M4's **bounded, declarative** self-modification, because none of the
reference-monitor properties (tamper-proof / always-invoked / verifiable) are violated by code that
only mutates declarative data via a pure `apply()` and never executes attacker/model-influenced
instructions in the supervisor's address space. The real trigger is **in-address-space execution of
untrusted code**, which arrives at the *executor* (running tools), not at declarative self-mod.

This **amends the D-M1-3 hard gate**, which conflated two events. The precise trigger:
> Upgrade to (separate-process supervisor + Wasmtime-sandboxed executor, epoch-interrupt limits)
> **the moment the executor is about to run any model-generated, externally-fetched, or
> not-operator-reviewed code — before the first such execution, never after, and not before.**
Encode it as a hard gate: the executor's "run code" path must be unconstructable while the supervisor
is in-process (pair `Executor` with `OutOfProcessSupervisor + WasmSandbox` at the type/startup level).

**To stay honest in-process, M4 MUST NOT:** (1) compile/`eval`/link/`dlopen`/jump to any model-produced
bytes; (2) add a `MutationKind` whose `apply` executes/spawns/shells genome-derived content (`apply`
stays pure + total, no `_ =>`); (3) let any genome field become a code/command/plugin path; (4)
introduce the executor running tools/native/fetched code under the in-process supervisor (that *is*
the trigger); (5) relabel any model-influenced mutation as `DirectUserIntent` (existing no-launder).

**Why.** Anderson/NIST "tamper-proof" presupposes an actor that can issue instructions to overwrite
the monitor; OpenSSH privsep's whole value is *containing a code-execution compromise* — both are
absent when the supervised code cannot execute arbitrary instructions. Paying for that isolation now
buys a defense against an absent threat; deferring it to the executor boundary schedules it at the
exact event that makes in-process dishonest.

**Citations.** Reference monitor / NEAT (NIST CSRC; Anderson); OpenSSH privsep (Provos; README.privsep);
Wasmtime security + epoch-vs-fuel (docs.wasmtime.dev); in-process vs process isolation (arXiv 2306.08127).

---

## D-M5-1 — Value source: operator-as-customer payer, inflow-bounded, efficiency-only

**Decision.** M5 ships **one concrete payer — operator-as-customer (v0)**: a published per-task-class
**Tariff** + an operator-owned **Grader** (grades against held-out ground truth the being cannot
observe at decision time; a deterministic substring grader is the v0 stand-in) + an **inflow-bounded
Treasury** (total credits can never exceed the committed external inflow — budget-conservation). On a
graded-accepted delivery the operator credits the **survival Account** (`supervisor.credit`). An
`ExternalPayer` trait is the **hook** so a genuinely exogenous payer (one the operator cannot
reprice) drops in later. Until that exists, **value-capture claims are labeled EFFICIENCY-ONLY**.

**Why.** On a local 16 GB build there is no real marketplace, and the prior economic analysis is
explicit: under operator-as-customer the value gradient is operator-internal, so selection results
are *void as economics* until a genuinely exogenous payer exists — which is exactly the precondition
the M6 anti-theater/economic gate depends on. The **grader is the load-bearing Goodhart surface**, so
it is operator-owned, held-out, and non-stationary-capable; revenue (not savings) defines surplus so
a being cannot grow its budget by burning slower. This makes the economic gate **live as methodology**;
the §13.1 anti-theater + compounding gates only *fire* on real foreground runs.

**Status.** Engineering call grounded in the prior economic analysis (D-M1-2 + the architecture's
exogenous-payer derivation); no new web research. The exogenous-payer commitment remains the step-0
that turns the economic gate from methodology into a live, derived threshold.
