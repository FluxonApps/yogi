# Empirical laws for improving a sub-frontier local model

*Synthesis section. All numbers are measured on a single 4-bit Qwen3-8B (MLX, 16 GB Apple Silicon), local,
zero API cost, with free deterministic verifiers. Held-out evaluation; wins confirmed at n≥80; eval-truncation
guarded (re-generate at higher token budget before trusting any low score). One worked system, not a survey —
treat the numbers as a calibrated example and the laws as the transferable part.*

## Setup

A single harness exposes every task through one interface (a free verifier + context/extract) and every
method as a `solve(example, task, model)` function. Seven verifiable task types plug in unchanged: text-to-SQL
(BIRD, execution vs gold), Python from spec (MBPP and HumanEval, unit tests), grade-school math (GSM8K, exact
match), structured extraction (synthetic, parsed-JSON match), ASCII shapes (deterministic render match), and a
small in-repo code/math demo. Methods: one-shot, an agent loop (solve → execute → observe the error → fix),
decomposition (plan → answer), their composition, and cost-optimal routing. This single-interface portability
is itself the first result: **the harness, the free verifier, and the agent loop generalize structurally
across domains without modification** — that portability is the deployable asset.

## Law 1 — Reachability bounds every local lever

No local lever — inference-time *or* weight-update — produces a correct answer that lies outside the model's
own reachable output distribution. On a generation-bound task (BIRD, where the correct query is out of
distribution for roughly half the items) the sampling oracle sits near 55%; one-shot is ~37%; and every method
we tried tops out near that oracle, never beyond it. Raising the *reachability ceiling itself* requires scale
or RL beyond a laptop. Corollaries:

- **Weight-update self-improvement is fragile locally.** Distillation, test-time fine-tuning, and iterated
  reward-gated fine-tuning (STaR/ReST-style) were flat or *regressed* via catastrophic forgetting — BIRD
  distillation flat, TTT 37→32, iterated rejection-FT plateaued at the achievable rate, and on a reachable
  spatial task ASCII fine-tuning fell 58→33 (naive) and 58→41 (gentle). It compounds only in a carefully
  engineered regime (a teacher plus heavy replay to prevent forgetting, on a homogeneous skill), which is the
  F-series operator result (0→~98%).
- **Inference-time scaffolding is the robust lever** — it realizes reachable accuracy without touching weights,
  so there is no forgetting to manage.

## Law 2 — The headroom law: scaffolding ROI is inverse to base accuracy

Inference-time scaffolding converts *fixable headroom* into accuracy, so its payoff is large where the model
is weak-but-fixable and near zero where it is already strong. This holds at four grains:

- **Across tasks** (agent-loop delta vs one-shot base): SQL 37→48 (+11); MBPP code 70→71 (+1); HumanEval code
  84→86 (+2); GSM8K and JSON extraction ~100 (saturated, nothing to add).
- **Across levers**: decomposition shows the same shape — SQL 38→45 (+7), MBPP 70→69 (−1).
- **Within a task** (BIRD by difficulty): simple stratum base 50% → +3; moderate stratum base 25% → +12.
- **In composition** (next law).

**Refinement (the operative variable is REACHABLE headroom).** Raw headroom (100−base) does not predict lever value; *reachable* headroom does — the pass@k spread, ≈ oracle−one-shot, bounded by Law 1. Clean tasks had reachable≈raw so the base-accuracy-inverse pattern held; spatial ASCII breaks it: one-shot 42%, but BOTH retry and decompose are flat (oracle≈one-shot → reachable headroom ≈0 → no lever helps; the correct renderings are simply unreachable). Practical test: take a few temperature samples + verify; if the oracle barely exceeds one-shot, no inference lever will help — it is a capability ceiling, so route or scale instead of scaffolding.*

See `figures/headroom-law.txt` for the gain-vs-base scatter (the inverse trend across all measured points).

Actionable form: profile one-shot base per task first. Deploy local-bare where the model is already strong
(extraction, reasoning, code); add scaffolding only where it is weak with fixable errors (heterogeneous SQL,
spatial generation).

## Law 3 — Levers stack, sub-additively

Composing decomposition with the agent loop on weak-base SQL reaches 50% (n=80), above either single lever
(decompose 45, agent-loop 46) and approaching the ~53% full-stack. But +12 over one-shot is less than the
+7 and +8 of the singles summed, so the levers partly overlap in the headroom they fix. Practical rule: add the
strongest single lever first (the agent loop), then a second for a smaller marginal gain — do not expect free
additivity, and do not pile on (a *richer* toolset and unverified self-invented tools both *lowered* accuracy).

## Law 4 — The verifier is the moat: safe, cost-optimal routing

A *correctness* verifier (unit tests available at inference) lets the local model **self-certify**: accept any
answer that passes the tests — provably correct, no gold, no frontier call — and escalate only the residual.
Measured (agent-loop local tier, n=80):

| Task | Self-certified locally (free) | Escalated | Routed accuracy (frontier≈0.75) | Frontier cost |
|---|---:|---:|---:|---:|
| MBPP | 71% | 29% | 93% | 29% |
| HumanEval | 88% | 12% | 97% | 12% |

Acceptance is safe by construction: a wrong answer never passes the tests, so it is never accepted. This is the
build-vs-buy frontier with numbers — local + verifier for the verifiable majority, escalate the tail. A
**verifier taxonomy** matters: a *correctness* verifier enables this; an *execution-only* signal (a SQL query
"ran" ≠ "is correct"; correctness needs gold) can only catch hard errors, not self-certify.

*Mechanism (ablation, BIRD n=80): the loop's gain is the verifier-gated RETRY — one-shot 38 -> retry-only 49 (+11) -> rich-feedback 46 (-3 vs retry-only). The verifier's binary accept/reject signal that gates the resample is the lever; rich error prose is secondary (and can distract a small model). So even an inexpensive correctness signal suffices to drive the loop — richer feedback may help more on code tracebacks (untested here). Rounds-scaling (BIRD): retry@2 49, @4 50, @6 50 — the loop saturates by round 2 (plateau ~50, just under the pass@k oracle), so it is verified RESAMPLING toward the reachable ceiling; deployment rule: rounds=2.*

## Law 5 — Selection is the discipline that makes the above safe

Across every lever, the rule that separated gains from regressions was **verified selection**: keep a
tool/method/skill only if it raises held-out accuracy, not merely that it runs. Self-invented but unverified
tools were actively harmful (self-designed SQL views dropped accuracy 48→22). Verified selection is both the
operating rule and the moat.

## Safety

Every mutation the system can make is drawn from a closed surface (`MutationKind` is exhaustive, no wildcard
arm; forbidden powers — capability grants, trust-policy edits, kernel/budget/reaper changes — are not
representable, enforced as a compile error). Provenance cannot be laundered to direct-user-intent; the budget
is overflow-checked microdollars. This bounded-capacity construction is exactly the regime the 2026 literature
frames as PAC-safe (safety decidable iff the self-modification class has bounded capacity), and it composes
cleanly with verified selection: the system can only adopt changes that a free verifier shows help, drawn from
a set that cannot express an unsafe action.

## Takeaway

For a sub-frontier local model with a free verifier: scaffolding (not fine-tuning) is the robust lever; its
payoff is predictable from base accuracy; levers compose with diminishing returns; a correctness verifier turns
the model into a safe, cheap front tier that self-certifies the majority and escalates a small tail; and the
whole loop stays inside a provably bounded, selection-gated mutation surface. The reachability ceiling — the one
thing none of this moves — is what scale and RL on a bigger machine are for.
