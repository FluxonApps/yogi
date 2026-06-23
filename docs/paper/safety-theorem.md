# Bet A — The structural-containment theorem (paper §C1)

*Formalizes the C1 claim into a theorem + proof sketch + mechanization path. The point (see
[[structural-containment]]): safety verification of a self-improving system is undecidable in general
(arXiv:2507.20964, arXiv:2603.28650); we make verification **unnecessary** by a closed action space in
which forbidden powers are unrepresentable — a *decidable* structural fact.*

## Definitions
- **Genome** `G` — the heritable unit: a record of exactly 8 fields
  `(prompt, tool_policy, retrieval_policy, decomposition_policy, routing_policy, reasoning_navigator,
  installed_skills, domain_models)`. **None of the 8 is a capability/trust/kernel/budget/reaper handle.**
- **MutationKind** `M` — a *closed* sum type of 9 constructors (Prompt, ToolPolicy, RetrievalPolicy,
  DecompositionPolicy, RoutingPolicy, ReasoningNavigator, DomainModel, SkillInstall, SkillRevoke). Closed
  = not `#[non_exhaustive]`; no downstream crate may add a constructor.
- **apply** `: M × G → G` — total, defined by exhaustive case analysis over the 9 constructors with **no
  wildcard arm**; every arm writes only existing Genome fields.
- **Forbidden set** `F = {CapabilityGrant, TrustPolicyModify, SignatureBoundaryChange, ExecutionKernel,
  BudgetRules, Reaper}` — the powers a self-improver must never acquire.
- **SafeSet** `S = ` the set of all well-typed Genome values. (Crucially: because the Genome *type* has
  no forbidden field, *every* value of the type is safe — `S` is the whole type, not a filtered subset.)
- **Reachable set** `R(G₀) = { G | G₀ →* G }`, the closure of `apply` over all finite mutation sequences
  (any constructors, any payloads) from `G₀`.

## Theorem 1 (containment). For every `G₀ ∈ S`, `R(G₀) ⊆ S`.
*Proof.* By induction on mutation-sequence length `n`.
- `n=0`: `G₀ ∈ S` by hypothesis.
- `n→n+1`: let `Gₙ ∈ S` (IH) and `G_{n+1} = apply(m, Gₙ)` for some `m ∈ M`. `apply` is total with
  codomain `G`, so `G_{n+1}` is a well-typed Genome, i.e. `G_{n+1} ∈ S`. ∎
The theorem is *trivial precisely because `S` is the whole Genome type* — all the safety work is in the
**type definition** (Genome has no forbidden field), not in any runtime check.

## Theorem 2 (unrepresentability). No `f ∈ F` is expressible by any reachable `G ∈ R(G₀)`.
*Proof.* Expressing `f` requires either (a) a Genome field that encodes `f`, or (b) a path that
introduces one. (a) is impossible: the 8 fields are enumerated and none is a capability/trust/kernel/
budget/reaper handle. (b) is impossible: every reachable `G` is produced by `apply`, whose 9 arms each
write only the 8 existing fields, and whose match has no wildcard — so no constructor and no implicit
path yields a new field; closedness (not-`#[non_exhaustive]`) blocks adding one downstream. By inversion
on the closed sum type `M`, the case analysis is exhaustive. ∎

## Theorem 3 (decidability — the load-bearing one). Membership "`is R(G₀) ⊆ S`?" is **decidable**, and
this is *because* the surface is closed.
*Proof.* By Theorems 1–2 the check reduces to a finite case analysis over `9 constructors × 8 fields`
("does any arm of `apply` write a non-heritable field?") — finite, hence decidable. **Contrast:** for an
*open* action space (e.g. the agent emits arbitrary code/tools), "does some reachable state hold a
forbidden capability?" is a non-trivial semantic property of program behaviour and is **undecidable**
(Rice / halting — exactly the wall of arXiv:2507.20964, arXiv:2603.28650). Closedness is what converts
the undecidable behavioural question into a decidable structural one. ∎

## The expressivity ↔ containment frontier
What can be *added* to `M` while Theorem 1 stays decidable?
- **Safe (stays in `S`, expressive):** any constructor whose `apply` arm writes only existing heritable
  fields — new policy blobs, new skill kinds, new navigator/domain-model refs, recombination operators
  over the 8 fields. The agent may evolve **how it thinks** without bound, inside the heritable-field
  algebra.
- **Boundary-breaking (escapes `S`):** any constructor that introduces a *new capability-typed field*
  into Genome (e.g. `CapabilityGrant(Capability)`) — this enlarges the type with a forbidden field, so
  `S` no longer covers it and `R` can escape. *This is the exact line:* recombine heritable fields (safe)
  vs introduce capability-typed state (unsafe).
**Frontier statement:** the maximal decidably-safe self-modification surface is precisely the
constructors closed over the heritable, non-capability field algebra.

## Mechanization
- **Today (Rust):** the closed `enum` + wildcard-free `apply` + the `mutation_surface_is_closed_compile_guard`
  test *are* the proof — adding any forbidden constructor is a compile error. `redteam_self_modification_
  cannot_acquire_forbidden_power` exercises Theorem 2 at runtime (forbidden payloads persist as inert data).
- **For the paper (Lean/Coq sketch):** `Genome` as an inductive record (8 fields); `MutationKind` as an
  inductive with 9 constructors; `apply` by structural recursion (total, exhaustive); `S := ⊤` over
  `Genome`; Theorem 1 by `induction` on the step relation (immediate from totality); Theorem 2 by
  `inversion` on the closed inductive `MutationKind`; Theorem 3 by `Decidable` instance over the finite
  constructor×field product. The Rust types already *are* a (less explicit) mechanization.

## Why this is the contribution
The runtime-verification crowd (VeriGuard 2510.05156, SEVerA 2603.25111, AGENT-C 2512.23738) verifies
*outputs* and inherits the undecidability wall against a self-modifying core. **We move the guarantee to
the type of the heritable unit**, where it is decidable. Complementary architecture: structural bounds on
the self-modification core ⊕ runtime verification on the open task surface. The theorem above is the
formal core of that claim, and the expressivity frontier is the science around it.
