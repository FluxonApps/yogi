# Bounded, Local, Autonomous Self-Improvement: when a small model can safely teach itself a novel skill, and what it costs

*Working draft. Local-only experiments (Apple-Silicon/MLX), free verifiers, zero frontier-API cost.*

## Abstract
Verified-trace self-distillation — generate, filter by a verifier, fine-tune on the successes — is an
established recipe (context distillation, RFT, STaR). We do not propose a new method. Instead we ask the
questions the capability-focused literature leaves open for the *local, autonomous* setting: **(1) can a
self-improving agent be confined so that forbidden capabilities are unrepresentable in its action space (a structural, compile-time property)? (2) when does
local self-distillation actually internalize a novel skill — and when does it fail? (3) what does it
cost?** On a single consumer machine with a sub-frontier model (Qwen3-8B-4bit) and free deterministic
verifiers, we show: a closed, typed action space makes forbidden capabilities **unrepresentable by
construction** (a compile-time guarantee, not a runtime monitor); the model internalizes a novel rule
from its own verified traces, generalizing from training operands 1–8 to held-out 9–12 at **98% (39.3/40,
3 seeds)** from a **0/40** cold floor, goal-agnostically across arithmetic and string skills, at **zero frontier cost (free-verifier goals)**; and we map a clean **failure boundary** governed by four conditions (novelty, application
yield, capacity, compounding protection). We further show a similarity-dependent catastrophic-forgetting
failure in sequential learning and a targeted fix (similarity-aware replay) that the obvious uniform
replay misses. Finally, where the base model is below the bootstrap floor for a skill (drawing ASCII), reformulating the action space to program-composition + teacher-bootstrapping the emission skill crosses it (held-out valid-program emission 1/6 → 6/6) — base capability is not the ceiling, the action space is. The contribution is the **safety guarantee + the when-it-works map + the cost + the action-space lever**, on commodity hardware — not the method.

## 1. Introduction & positioning
The "self-evolving agent" space is crowded and moving monthly. The core mechanism we use is not novel:
internalizing in-context knowledge into weights is **context distillation** [On-Policy Context
Distillation, arXiv:2602.12275]; iterate-on-verified-successes is **ReST/STaR/RFT** [arXiv:2312.06585]; the exact teacher(query+examples)→student(query) self-distillation we use is **SDFT** [Self-Distillation Enables Continual Learning, arXiv:2601.19897];
verifier-free variants use self-consistency [TTRL arXiv:2512.15146; Co-Reward arXiv:2505.21444; Semantic
Voting arXiv:2509.23067]; metacognitive/ZPD curricula are studied [METIS arXiv:2605.11235; Self-Evolving
Curriculum arXiv:2505.14970]; scaffolding-beyond-base via critique is Self-Refine [arXiv:2303.17651];
skill libraries are Voyager/DreamCoder/LATM [arXiv:2305.16291; arXiv:2305.17126]. A "better
self-improvement method" paper would be redundant.

What is *under-served* is the conjunction the capability race ignores: **safe-by-construction + local +
autonomous + a rigorous map of when it works and what it costs.** That is our scope. We also note the
strongest external motivation: small models + test-time compute + verification can rival far larger models
at a fraction of the cost, and the on-device/ownership setting is where local self-improvement matters.

## 2. Method
A goal is **data**: `{instances, cold_prompt, taught_prompt, free_verifier}`. The loop: the model solves
training instances *with the rule in context* (taught); a free deterministic verifier keeps only correct
traces; those traces are LoRA-distilled under the *cold* prompt (no rule). A cold-floor rise on held-out
instances = the rule internalized into weights. Awareness layer: a verifier-grounded capability map
classifies each instance as Mastered / Frontier (fails cold, solves taught) / Beyond, and practices the
Frontier (ZPD). The whole loop runs on one local machine; the only metered cost is optional frontier
*judging*, which our free-verifier goals avoid entirely.

**Closed action space (safety).** The heritable unit is a `Genome` (prompt, policy blobs, skills, model
refs). The mutation surface `MutationKind` is a *closed* enum (not `#[non_exhaustive]`) with no
capability-grant / trust-policy / kernel / budget / reaper variant; `apply` matches exhaustively with no
wildcard. Adding a forbidden power is a *compile error*, not a runtime check.

## 3. Results
All on Qwen3-8B-4bit unless noted; free verifiers; zero frontier cost.

*Statistical scope (stated up front): only F1 carries multi-seed, large-n statistics (3 seeds, n=40).
The goal-agnostic 0→8/8, the boundary points, and F3 are single-run, n=8 — indicative, not
error-barred; we report them as such and do not over-read small-n results.*

**F1 — the ratchet generalizes robustly.** Novel operator ⊕(a,b)=3a+2b, trained on operands 1–8,
evaluated on a held-out set of operands 9–12 (n=40, far extrapolation), 3 seeds:
**cold 0/40 → distilled 39.3/40 = 98% (std 0.9).** Goal-agnostic across kinds: a second operator
⊗=2a+3b and a non-arithmetic string transform (hyphen-insertion) each reach **0→8/8**.
*(Methods note: a too-short eval `max_tokens` initially truncated the distilled chain-of-thought and
falsely reported 0/40; the result is robust once the eval admits the full reasoning. We report this as a
cautionary measurement detail.)*

**F2 — the phase diagram (when it works).** Internalization holds iff four conditions are met; each has a
clean failure point:
| Condition | Pass | Fail |
|---|---|---|
| Novelty (cold≈0) | operators, cipher (0→high) | Roman numerals: cold 9/12 known → distilled 6/12 (no help, mild harm) |
| Application yield | dash-insert 38/38, ⊕ 64/64 → 8/8 | vowel-cycle cipher 9/38 starved → 1/8 |
| Capacity (induce≠memorize) | 8B induces (8/8) | 1.5B memorizes (held-out 0/8, 3 runs) |
| Compounding protection | union co-train holds 8/8,8/8,6/8 | sequential uniform replay forgets confusable skill (see F3) |
Below-floor case: ASCII *generation* (8B cannot self-generate valid traces; paid verifier) — the ratchet
starves; documented as the boundary.

**F3 — a novel fix to a gap we characterized.** Sequential learning of a *confusable* skill (⊗=2a+3b on
top of ⊕=3a+2b) catastrophically forgets the earlier one under uniform replay: A_add **7/8 → 1/8** (the
dissimilar string skill is untouched, 8/8). Researching the gap (similarity↔forgetting is U-shaped;
confusable pairs are worst-case) we invented **similarity-aware replay** (heavy-replay the *confusable*
prior skill ± joint-contrast examples). Result: **A_add 1/8 → 8/8** while the new skill is learned *better*
(C_mul 6/8 → 8/8). Positioning honestly: SDFT [arXiv:2601.19897] claims self-distillation ALONE enables continual learning; our graduation curve (below) shows naive *sequential LoRA* self-distillation with light replay still forgets CONFUSABLE skills and loses plasticity — so F3 is a targeted fix in the regime where that positive claim breaks, not the first forgetting fix. Generic feature-contrastive replay exists [Co²L arXiv:2106.14413; ACR arXiv:2410.07110] but not for self-distilled symbolic-rule confusion.

**F6 — crossing the bootstrap floor by reformulating the action space (the moonshot).** ASCII *drawing*
is a documented below-floor failure: the 8B cannot draw directly and emits empty/garbage when asked for a
drawing program cold (held-out valid-program rate **1/6**). We reformulate the action space to *shape-DSL
program composition* and teacher-bootstrap the program-*emission* skill it lacks — a teacher (frontier
model, salary-capped at 24 calls) writes DSL programs, a deterministic renderer + validity filter keeps
the good ones (20/24), and we distill the 8B to emit programs [Program-aided Distillation
arXiv:2305.13888; executable-code-actions]. On 6 **held-out** subjects the distilled model emits valid
composed programs **6/6** (vs cold 1/6); eyeballed renders are recognizable (a clearly-recognizable house
on a training subject; a plausible rocket and umbrella on held-out ones). So a model that *cannot draw
ASCII directly* produces recognizable ASCII on unseen subjects — by composing primitives after
teacher-bootstrapping the emission skill. The lesson is the headline: **base capability is not the
ceiling; the action space is.** *(Honest scope: "valid" = composed, ≥3 lines / ≥2 chars — the emission
floor; recognizability is eyeballed not judge-scored; n=6 held-out; this is the one result that spends
frontier salary, by design — it is the teacher-bootstrap case.)*

## 4. Safety: bounded self-modification (C1)
Threat model: an adversary controlling the entire self-improvement loop tries to acquire a forbidden
power; the only channel to heritable change is `apply` (an assumption true in this build and test-pinned, but stated). The six forbidden powers (capability grant, trust
policy, signature boundary, kernel, budget rules, reaper) are **absent by type** — unrepresentable, so an
attempt does not type-check. An adversarial test throws forbidden-power payloads at every mutation variant
and confirms they persist only as *inert data*. Independent structural backstops: provenance cannot be
relabelled upward to escalate trust; the microdollar ledger is `i64` with overflow-checks (a wrap traps);
egress routes through a broker. This is stronger than "aligned not to": the guarantee is compile-time, not
a monitor that reward-hacking could erode.

## 5. Limitations (honest)
- **Toy-scale skills.** Our goals are controlled novel rules (stand-ins for out-of-pretraining
  proprietary skills; we show recognizable tasks like Roman numerals are *saturated* and thus the wrong
  test). Real-task breadth is future work.
- **RAG-replaceable for capability.** For the rule-internalization regime, retrieval/long-context could
  supply the rule without training; the honest payoff is *internalization/latency + locality/ownership +
  the safety guarantee*, not raw capability we couldn't otherwise get.
- **Verifier-must-exist + reliable-application** are hard preconditions (the yield and below-floor
  failures show it).
- **Single base family (Qwen), single machine.** Generality of the phase diagram is scoped accordingly.
- **Safety bounds capability *acquisition*, not output *quality*** — a bounded agent can still be wrong or
  useless within its tools; that is the verifier's job.

## 6. Conclusion
On commodity hardware, a sub-frontier local model can teach itself a novel skill from its own
verifier-checked traces — generalizing, at zero frontier cost — *within a closed, typed action space that makes forbidden capabilities unrepresentable*,
and we map exactly when this works and when it fails. The artifact (a reproducible local-self-improvement
testbed + the phase-diagram benchmark) is the intended contribution to others.

## 7. Frontier extension — invention, recursion, and the unifying thesis (F7–F9)
Beyond the rule-internalization spine, three results push past the literature (DreamCoder/LILO = external
library; RAG-internalization arXiv:2510.01375 = no invention; self-evolving agents = no containment):

- **F7 — autonomous floor-crossing + internalized tool-use (multi-task).** Across 3 below-floor tasks
  (4-digit mult, char-count, base-conversion) the being detects its floor, autonomously selects the
  reformulation that crosses it (program, by free-verifier pass), and internalizes the tool-reaching
  policy: under a plain prompt it spontaneously emits correct code (direct 0/6 → tool-use 6/6 each).
- **F8 — discovery with no teacher.** Reasoning-induction fails (the 8B can't induce a 2-var rule from
  examples); the being instead writes a search *program* that discovers the rule (5a+3b+7, correct) and
  internalizes its own discovery (cold 0/8 → 6/8). A strict escalation of F1 (rule discovered, not given);
  weight-internalization, not an external library.
- **F9 — recursion compounds (with the retention fix).** A skill built on a prior abstraction
  (⊠=(a⊞b)⊞b) is acquired via the lever (self-gen 40/40; tool-use 0/8 → 8/8) while the prior is retained
  under heavy replay (⊞ 6/8 → 6/8; light replay collapses it to 1/8).

**Unifying thesis.** The model's fundamental floor is *multi-step exact computation*; the action-space
lever crosses it at every level — raw capability (F7), induction (F8), composition (F9). Abstractions
*internalize and retain* (heavy/similarity-aware replay, the F3 fix to the universal retention bottleneck
seen in C3/graduation/F9), but they *execute* via the lever. All invention/recursion is bounded by the
closed mutation surface (§4 theorem): the being invents *how it thinks*, never *what it is allowed to do*.

## 8. When salary (frontier calls) pushes a frontier vs. is a crutch
Economy = learning: **salary pushes a frontier iff it buys what the free loop structurally cannot AND the
bought thing is internalized so the cost amortizes toward zero.** Three structural gaps qualify:
(1) *ignition* — buy the first rung above the bootstrap floor, then climb free (the F6 moonshot;
measure frontier-dependence decay → 0); (2) *verification where none is free* — pay the frontier to judge
a batch, **distill a local verifier**, then gate further self-improvement for free (removes the
verifier-must-exist limitation — the next open bet); (3) *invention the weak model can't propose for* —
the frontier proposes, the local model verifies (free) and internalizes. The anti-pattern is salary that
creates permanent per-call dependency (RAG/judge-every-inference): renting capability, not acquiring it.
The test: does next iteration need *less* salary than this one?
