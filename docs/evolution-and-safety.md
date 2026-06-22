# Evolution and Safety — Yogi's central tension, stated plainly

> Does introducing safety restrict evolution? **Yes — necessarily.** The whole design is about
> *where* that restriction sits and *how* it moves. This note states the thesis without hedging.

## 1. Safety *is* a bounded mutation space

Yogi's self-modification is the **closed `MutationKind` surface**: `Prompt`, `Skill` install/revoke,
`ToolPolicy` / `RetrievalPolicy` / `DecompositionPolicy` / `RoutingPolicy`, `DomainModel`,
recombination, distillation. It has **no** `CapabilityGrant`, no kernel/budget/reaper edit, no
provenance relabel, no "add yourself a tool." `apply` matches exhaustively with no wildcard — a
forbidden variant is a *compile error*.

So safety and mutation-freedom trade off directly:

- **Provable safety ⟺ bounded evolution.**
- **Unbounded open-ended evolution ⟺ no safety guarantee.**

You cannot have both fully. A self-evolving being that *didn't* restrict mutation wouldn't be
"safer and freer" — it'd just be unbounded, which is the exact thing the project exists to avoid.

## 2. The restriction is on the *space*, not the *search*

The design bounds the space so the **entire reachable closure is safe** — no sequence of allowed
mutations can reach a forbidden state. Then evolution runs **completely free inside** that enclosure.
Safety does not slow mutation or selection; it defines the fence. Inside the fence, search is as
vigorous as you like.

## 3. The cost: meta-moves live outside the fence

The safe enclosure excludes the highest-leverage moves — **expanding your own action space, acquiring
new tools, rewriting your own pipeline, researching and applying new methods**. Those are where the
biggest leaps live, and they are exactly capability-expansion, so they are exactly what the closed
surface forbids. Therefore **safety caps the self-achievable ceiling.**

### Worked example — the ASCII arc (2026-06-22)
The being plateaued because `qwen3:8b` can't draw ASCII one-shot. The breakthrough was to *change the
action space* — compose shape **primitives** on a deterministic canvas (program synthesis), and add a
draw→critique→redraw **refine** loop. The being could not have invented those: they are meta-moves
outside its surface. **The operator (a human / the frontier agent) invented them.** That is the safety
boundary working as intended — and it exposed the honest split below.

## 4. Invention vs. selection — and "nothing human"

Split the meta-layer in two:

- **Selection / composition → give it to the being.** Register the new strategies as a
  **capability-bounded toolspace** (`DrawTool::{Direct, Program, Refine}`), and let the being's
  *existing* closed-surface evolution (`ToolPolicy` mutation) **discover which tool/composition wins**,
  selected by fitness. The being didn't invent the tool, but it *learns to reach for it* — and learns
  to route around the ones that fail (e.g. program-synthesis returns empty on qwen; the being evolves
  away from it on its own, no human deciding). **Strategy-discovery now happens in Yogi.**
- **Invention → the capability-expansion frontier.** Inventing genuinely new tools / researching /
  rewriting the loop is what the closed surface guards.

The operator directive **"nothing should be human, I'm OK with the toolspace"** (2026-06-22) resolves
this cleanly: provide a **rich, capability-bounded toolspace once**, then the being evolves over it
fully autonomously. **Safety then comes from the toolspace *boundary* — every tool sandboxed, judged,
incapable of self-granting power — not from a human in the loop.** This is the right place for full
autonomy: *maximal evolution inside a bounded enclosure.* Autonomy and safety stop fighting because
the fence, not a person, is the safety.

## 5. The fence rises — "trust-native"

The boundary is not frozen. It moves outward through **human-vetted, verified, capability-bounded
additions**: a new primitive is invented and sandbox-checked once; thereafter the being may *compose*
it. The reach of evolution grows **as fast as verification and earned trust allow — and no faster.**
That last clause *is* the safety. The rest of Yogi — signed journal, reaper, capability broker,
Two-Gate, no-launder — exists to *earn* the trust that lets the fence move.

## One-line statement of the thesis

> **Safety restricts *what* can evolve (the reachable set), not *how hard* evolution searches it; the
> price is that capability-expanding meta-moves sit outside the fence, supplied by the operator as a
> bounded toolspace — and the fence rises only on earned, verified trust.**
