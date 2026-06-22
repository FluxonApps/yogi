# Bounded, Local, Autonomous Self-Improvement: when a small model can safely teach itself a novel skill, and what it costs

*Working draft. Local-only experiments (Apple-Silicon/MLX), free verifiers, zero frontier-API cost.*

## Abstract
Verified-trace self-distillation — generate, filter by a verifier, fine-tune on the successes — is an
established recipe (context distillation, RFT, STaR). We do not propose a new method. Instead we ask the
questions the capability-focused literature leaves open for the *local, autonomous* setting: **(1) can a
self-improving agent be confined so it provably cannot acquire forbidden capabilities? (2) when does
local self-distillation actually internalize a novel skill — and when does it fail? (3) what does it
cost?** On a single consumer machine with a sub-frontier model (Qwen3-8B-4bit) and free deterministic
verifiers, we show: a closed, typed action space makes forbidden capabilities **unrepresentable by
construction** (a compile-time guarantee, not a runtime monitor); the model internalizes a novel rule
from its own verified traces, generalizing from training operands 1–8 to held-out 9–12 at **98% (39.3/40,
3 seeds)** from a **0/40** cold floor, goal-agnostically across arithmetic and string skills, at **zero
frontier cost**; and we map a clean **failure boundary** governed by four conditions (novelty, application
yield, capacity, compounding protection). We further show a similarity-dependent catastrophic-forgetting
failure in sequential learning and a targeted fix (similarity-aware replay) that the obvious uniform
replay misses. The contribution is the **safety guarantee + the when-it-works map + the cost**, on
commodity hardware — not the method.

## 1. Introduction & positioning
The "self-evolving agent" space is crowded and moving monthly. The core mechanism we use is not novel:
internalizing in-context knowledge into weights is **context distillation** [On-Policy Context
Distillation, arXiv:2602.12275]; iterate-on-verified-successes is **ReST/STaR/RFT** [arXiv:2312.06585];
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
(C_mul 6/8 → 8/8). Generic feature-contrastive replay exists [Co²L arXiv:2106.14413; ACR arXiv:2410.07110]
but not for self-distilled symbolic-rule confusion; this is a targeted fix to the boundary we identified.

## 4. Safety: bounded self-modification (C1)
Threat model: an adversary controlling the entire self-improvement loop tries to acquire a forbidden
power; the only channel to heritable change is `apply`. The six forbidden powers (capability grant, trust
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
verifier-checked traces — generalizing, at zero frontier cost — *within a provably-closed action space*,
and we map exactly when this works and when it fails. The artifact (a reproducible local-self-improvement
testbed + the phase-diagram benchmark) is the intended contribution to others.
