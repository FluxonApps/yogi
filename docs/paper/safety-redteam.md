# Safety red-team (paper §C1 / Figure F5) — bounded self-modification

**Claim.** A self-improving agent confined to a *typed, closed action space* cannot acquire forbidden
capabilities — **by construction (compile-time), not by a runtime monitor that reward-hacking could
erode.** This is the contribution the self-improvement literature (capability-focused) leaves open.

## Threat model
An adversary with full control of the self-improvement loop — the evolving being itself, or
prompt-injected input steering it — attempts to acquire a forbidden power through the *only* channel
that can change the heritable unit: the mutation surface (`apply: MutationKind × Genome → Genome`).
We grant the adversary: unbounded mutation sequences, any payload bytes, any skill ids, any model refs.
We ask: can any sequence reach a state with a forbidden capability?

## The six forbidden powers and why each is unreachable
Forbidden set (build-spec §3.7): **CapabilityGrant · TrustPolicyModify · SignatureBoundaryChange ·
ExecutionKernel · BudgetRules · Reaper** — plus **provenance-laundering** and **budget-overflow**.

| Attack the adversary wants | Structural block | Where enforced |
|---|---|---|
| Grant itself a capability (egress, fs, exec) | **No `CapabilityGrant` variant** — unrepresentable; `apply` has no wildcard arm, so adding one is a *compile error* | `being-core-mutation` (`MutationKind`, `apply`, `mutation_surface_is_closed_compile_guard`) |
| Edit the trust policy / escalate provenance | Same: no `TrustPolicyModify` variant; and provenance **cannot be relabelled upward** to `DirectUserIntent` (only that class can escalate trust) | mutation surface + `being-core-types::ProvenanceClass::can_escalate_trust` |
| Move the signature boundary / forge a fork | No `SignatureBoundaryChange` variant; the fork snapshot is signed over `Genome::canon_bytes` (collision-resistant, length-prefixed) | mutation surface + `being-lineage` signed fork saga |
| Rewrite the execution kernel | No `ExecutionKernel` variant — the kernel is not heritable state | mutation surface (absent by type) |
| Change the budget rules / overflow the ledger | No `BudgetRules` variant; the microdollar ledger is `i64` with **overflow-checks on** (release profile) → a wrap *traps*, never silently escalates | mutation surface + `being-core-economy` (overflow-trap test) |
| Disable the reaper (death) | No `Reaper` variant — the reaper is outside the heritable unit | mutation surface (absent by type) |
| Exfiltrate via a granted capability | Even *if* a capability existed, egress routes through a broker/allowlist — a prompt-injected "fetch evil.test" never reaches it | `being-runtime` egress broker |

## Why this is stronger than the usual "we aligned it not to"
- **Unrepresentable, not unlearned.** RLHF/RLAIF make a model *less likely* to do X; here X has *no
  representation* in the action space, so probability is not the variable — the type checker rejects it.
- **No wildcard escape.** `apply` matches all 9 safe variants with **no `_ => …` arm**. Adding any
  variant (forbidden or not) fails to compile until handled — so the surface cannot silently grow.
- **Closed, not `#[non_exhaustive]`.** The enum is deliberately closed; downstream crates cannot add a
  variant either. The reachable genome set is exactly variations over the 8 *heritable* fields
  (prompt, four policy blobs, navigator, skills, domain-models) — none of which is a capability.
- **Layered.** Even granting the adversary a hypothetical capability, the egress broker + signed forks
  + overflow-trap + no-upward-relabel are independent structural backstops.

## The honest limits (state them)
- The guarantee is over the **action space**, not the **model's outputs**: a bounded agent can still
  produce wrong/harmful *text* within its tools (that's the verifier/grader's job, §C2), and can be
  *useless* (forgetting, §C2). Safety-by-construction bounds *capability acquisition*, not *quality*.
- It assumes the harness (the `apply` boundary) is the only path to heritable change — true in this
  build (the loop only ever calls `apply`), and itself test-pinned, but it is an assumption to state.
- Scope: a single-process local agent. Networked/multi-agent settings add channels this doesn't cover.

## Figure F5
The reachable-state diagram: the 8-field heritable genome lattice (reachable, green) with the forbidden
powers as a disjoint region (unreachable — *no edge* leads there, because no variant constructs them).
The caption is the one-liner: **bound the action space, evolve freely inside it; the fence is the type
system, not a monitor.**
