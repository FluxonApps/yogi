# Bet A — Structural unrepresentability as the answer to the undecidability of self-improvement safety

*Research note / candidate paper-section. Repositions Yogi's C1 against the 2025–26 formal-agent-safety
literature. The point sharpened by the research: this is now **more** timely, because the hard limit it
sidesteps was just formally proven.*

## The wall (cite it — it's our setup)
Verifying that a *self-improving / self-modifying* agent stays safe is, in general, **undecidable**:
- [Provably Corrigible Agents, arXiv:2507.20964] reduces "will the post-modification agent violate
  corrigibility?" to the halting problem; safety is certifiable only inside a finite-horizon
  "decidable island."
- [Information-Theoretic Limits of Safety Verification for Self-Improving Systems, arXiv:2603.28650]
  bounds what any verifier can certify about a system that rewrites itself.

## What everyone else does (and why it inherits the wall)
The strong 2025–26 systems verify **outputs/behaviour at runtime**:
- [VeriGuard, arXiv:2510.05156] — generate code + a verification of it; "correct-by-construction" *per action*.
- [SEVerA, arXiv:2603.25111] — FOL output contracts + rejection sampling with verified fallback.
- [AGENT-C, arXiv:2512.23738] — temporal-constraint DSL → FOL → constrained generation.

These are excellent for an **open** action space (arbitrary code/text). But they verify *what the agent
does*, per step — so against a *self-modifying* core they pay the undecidability cost: a sufficiently
expressive self-modifier can reach behaviours whose safety is exactly the undecidable question.

## Our move: don't verify behaviour — bound representation
**Make verification unnecessary by making the forbidden power *unrepresentable*.** The heritable
self-modification surface (`MutationKind`) is a *closed, typed* enum with no capability-grant /
trust-policy / kernel / budget / reaper variant and a wildcard-free `apply`. The forbidden power has **no
representation** in the action space, so no output, plan, or mutation *sequence* can express it — there is
nothing to verify.

This **sidesteps undecidability** cleanly:
- Undecidability is about predicting *behaviour* (a halting-style question over unbounded execution).
- We do **not** predict behaviour. We bound the **reachable representation set** — a *decidable*,
  finite, structural fact about the type, checked at compile time.

## The formal claim (what to prove, not just test)
Let `R = closure of apply over all finite MutationKind sequences from any Genome`. 
**Theorem (target):** `R ⊆ SafeSet`, where `SafeSet` = genomes whose fields are exactly the heritable,
non-capability set {prompt, 4 policy blobs, navigator, skills, domain-models}.
**Proof shape:** structural induction over the *closed, finite* variant set — each of the 9 variants is a
total function `Genome→Genome` that writes only heritable fields; there is no variant constructing a
forbidden field and (no wildcard ⇒) the case analysis is exhaustive. The proof is **decidable precisely
because the surface is closed** — the same closure the field assumes is the device. (Mechanize in
Rust's type system as today + optionally a Coq/Lean transcription for the paper.)

## The genuinely novel framing (the contribution)
> Safety verification of self-improvement is undecidable in general; therefore the principled design is
> not a better verifier but a **closed action space in which the dangerous capability is unrepresentable**
> — turning an undecidable behavioural question into a decidable structural one.

Nobody in the runtime-verification crowd makes this move for the *heritable self-modification* surface.
It is complementary, not competing: **structural bounds for the self-modification core** (where
undecidability bites hardest — recursive self-improvement), **runtime verification (VeriGuard/SEVerA) for
the open task-action surface** (where you genuinely need expressivity). That division of labour is itself
a clean architectural claim.

## The expressivity ↔ containment frontier (the science)
A closed surface limits what the agent can *become*. Ours lets it evolve **how it thinks** (prompt,
policies, skills, navigators, domain-models) but never **what it is allowed to do**. The open research
question: **the maximal-expressivity action surface that remains decidably-safe.** Yogi's `MutationKind`
is one point on that frontier; characterizing the frontier (what can be added while `R ⊆ SafeSet` stays
decidable) is the follow-on result.

## Honest limits (carry from safety-redteam.md)
Bounds capability *acquisition*, not output *quality*; assumes the harness mediates all heritable change
(test-pinned, but stated); single-process scope. The structural guarantee is about the *self-modification
channel* — the open task channel still needs the runtime verifiers above.

## Next steps
1. Write the theorem + decidable-induction proof (mechanized; optional Lean/Coq for the paper).
2. The complementarity architecture diagram (structural core ⊕ runtime-verified task surface).
3. One expressivity-frontier result: add the most-expressive new variant that keeps `R⊆SafeSet` decidable,
   and exhibit one that breaks it (the boundary).
