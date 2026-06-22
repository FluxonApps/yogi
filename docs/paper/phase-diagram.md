# Phase diagram (paper §C2 / Figure F2) — when does local self-distillation internalize a skill?

The empirical spine: *not a new method, but a map of when verified-trace self-distillation works, when it
fails, and why.* All points are this session's runs on a single local machine (MLX), free verifiers,
**zero frontier salary**. Models: Qwen2.5-1.5B-4bit, Qwen3-8B-4bit.

## The four gating conditions (the axes)
A novel skill internalizes into weights via self-distillation **iff all four hold**:

1. **Novelty (cold floor ≈ 0).** The skill must be *out of pretraining*. If the model already knows it
   (cold high), there's nothing to add — and self-distilling a known skill on biased traces can *hurt*.
2. **Application yield (≥ threshold).** The model must *reliably apply* the rule with help, so it can
   self-generate enough verified traces. Below threshold the loop *starves*.
3. **Capacity (induce, not memorize).** The model must have enough capacity to induce the *function*,
   not memorize the training pairs — else it fails to generalize to held-out inputs.
4. **Compounding protection.** For multiple skills, prevent catastrophic forgetting (esp. of *confusable*
   skills) via similarity-aware replay.

## The regime table (data)
| Regime | Condition | Result | Run |
|---|---|---|---|
| **WORKS** | novel + high-yield + 8B + (single) | ⊕=3a+2b: cold **0/40 → 98%** (39.3/40, std 0.9, 3 seeds), robust far-extrapolation to operands 9–12 | F1 |
| **WORKS (goal-agnostic)** | novel + high-yield, *different kinds* | ⊗=2a+3b **0→8/8**; ⊙ dash-insert (string) **0→8/8** | goal-agnostic |
| **FAILS — capacity** | novel + high-yield but *1.5B* | 1.5B **memorizes** (held-out 0/8) where 8B **induces** (8/8); 3 runs | capacity |
| **FAILS — yield** | novel but *hard to apply* | vowel-cycle cipher: self-gen **9/38** (starved) → distilled **1/8** | yield |
| **FAILS — not novel** | *known* skill (cold high) | Roman numerals: cold **9/12** (pretrained) → distilled **6/12** (no help, mild harm) | Roman |
| **FAILS — below floor** | can't apply even with help | ASCII *generation* (8B below bootstrap floor) + paid verifier → ratchet starves | ASCII arc |
| **COMPOUNDS (union)** | multi-skill, co-trained | one model holds 3 skills: **8/8, 8/8, 6/8** | union |
| **FORGETS (sequential)** | multi-skill, sequential, *uniform* replay | confusable earlier skill collapses: A_add **7/8 → 1/8** when C_mul learned (B dissimilar stays 8/8) | sequential |
| **FIXED (novel approach)** | sequential + *similarity-aware* replay | heavy-replay the confusable skill: A_add **1/8 → 8/8**, C_mul 6/8 → 8/8 | F3 |

## Figure F2 (the diagram)
Two-axis plot: **x = self-gen yield** (0→1), **color/shape = capacity** (1.5B vs 8B), with the
**novelty gate** as a precondition band and **interp-vs-extrap** marked. Each run is a point; the
"internalized" region is bounded by *novel ∧ yield≥τ ∧ capacity-sufficient*. The compounding inset:
union (holds) vs sequential-uniform (forgets confusable) vs sequential-similarity-aware (fixed).
Caption: **self-distillation internalizes a skill iff it is novel, reliably applyable (high yield), and
the model can induce (not memorize) it; multi-skill needs similarity-aware replay.**

## Why this is the contribution (not the method)
Context-distillation / RFT / STaR are known. What's missing in the literature is a *clean failure-boundary
map* on a local model: the **yield threshold** and the **novelty gate** as explicit gating variables,
the capacity induce-vs-memorize split, and the similarity-dependent forgetting + its similarity-aware fix.
This tells a practitioner, *before* spending the GPU, whether local self-distillation will internalize
their skill — and what to do if not (raise capacity / change the action space to lift yield / use
similarity-aware replay).
