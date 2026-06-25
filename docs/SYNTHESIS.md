# Yogi — synthesis: making a sub-frontier local model usable, safely, for free

*A reader's map of the whole project. One worked system on commodity hardware (4-bit Qwen3-8B, MLX, 16 GB
Apple Silicon), free deterministic verifiers, zero frontier-API cost. Numbers are a calibrated example; the
laws and the recipe are the transferable part. Full detail: `paper/local-model-laws.md`, `paper/draft.md`,
`local-model-eval-report.md` (+ PDF), `local-vs-frontier-guide.md`, `FINDINGS.md`.*

## Thesis (one paragraph)

A sub-frontier model on a laptop can be made genuinely usable on verifiable tasks — for free and provably
safely — by wrapping it in **verifier-gated inference scaffolding** rather than fine-tuning it. The achievable
gain is bounded by what the model can already *reach* (its pass@k spread), not by its raw error rate; within
that bound a free verifier lets the model self-certify the majority of its work as provably correct and flag a
small tail to escalate. Self-modification is kept safe by construction — every change is drawn from a closed,
typed mutation surface in which forbidden capabilities are not even representable. What scaffolding cannot do is
raise the model's reachability ceiling; that still requires scale or RL on a bigger machine.

## The lever-map (what works, what doesn't, and why)

- **Inference-time scaffolding is the robust lever.** An agent loop (solve → run → retry-on-fail), decomposition,
  and embedding-retrieved examples lift a hard task with no training and no forgetting (BIRD SQL 37 → 53).
- **Its payoff is inverse to base accuracy — and bounded by *reachable* headroom.** Scaffolding converts
  *fixable* headroom = pass@k oracle − one-shot (cheap to measure), not raw headroom. SQL had +20 reachable
  (realized +11); ASCII had raw headroom but ~0 reachable (errors systematic) → no lever helps. Confirmed across
  tasks, levers, within a task, and in composition (sub-additive).
- **The agent loop reduces to verified resampling.** It's the *retry*, not the feedback content (rich error
  prose adds nothing, can even anchor); it saturates fast where per-sample success is high (SQL, rounds=2) and
  needs independent best-of-N where it's low (MBPP). Feedback-rich "fix the broken code" ≈ plain "try again".
- **Weight-update self-improvement is fragile locally.** Distillation, TTT, and iterated reward-gated
  fine-tuning were flat or *regressed* via catastrophic forgetting — even on reachable tasks. It compounds only
  in a carefully engineered regime (teacher + heavy replay on a homogeneous skill: the F-series, 0 → ~98%).
- **The verifier is the moat, and verified selection is the discipline.** Keep a tool/lever only if it raises
  held-out accuracy (gold-free); unverified self-invented tools *hurt*. A *correctness* verifier additionally
  lets the model self-certify (no gold) → safe cost-optimal routing.

## The deployable recipe

1. **Profile first.** Take ~8 temperature samples + verify. The spread (oracle − one-shot) is the achievable
   inference gain. If it's ~0, no inference lever will help — route or scale.
2. **Pick the cheapest lever that captures the spread.** retry@2 if it saturates the spread; verifier-selected
   best-of-N if the spread is large but retry stalls; one-shot if already strong.
3. **Self-certify and route.** With a correctness verifier, accept any verifier-passing output (provably
   correct, no gold, no frontier) and escalate only the residual. Measured local tier: MBPP self-certifies ~75%,
   HumanEval higher, at ~2–3 generations/item — near-frontier accuracy on the verifiable majority at a fraction
   of frontier cost.
4. **Stay inside the closed mutation surface.** Any self-modification is selection-gated and drawn from a set
   that cannot express an unsafe action (PAC-safe-by-construction).

## The boundaries (state them honestly)

- **Reachability ceiling.** No local lever produces an answer outside the model's reachable distribution;
  generation-bound tasks cap at their oracle. Raising the ceiling needs scale (~900k traces) or RL.
- **Verifier taxonomy.** Self-certification and safe routing need a *correctness* verifier (unit tests,
  expected outputs). An execution-only signal ("it ran") can't certify correctness.
- **Weight-update fragility.** Local fine-tuning forgets; treat it as a careful, replay-protected last resort,
  not a default.

## What it answers

From "can a sub-frontier local model be made usable, safely, and for free?" — yes, on verifiable tasks, via
verifier-gated scaffolding and self-certified routing, all inside a provably bounded mutation surface — with a
precise, measurable account of exactly how far that goes and where scale takes over.
