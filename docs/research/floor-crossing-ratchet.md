# Bet B — The closed floor-crossing ratchet (scoped experiment)

*Research note / experiment design. The most distinctive thing we have; high-risk, highest-ceiling.
The moonshot proved a model can cross its bootstrap floor when WE pick the action-space reformulation.
The frontier: the being detects the floor, picks/invents the reformulation, AND distills the crossing
back into weights.*

## The gap the research confirms is open
Autonomous tool-making is active — [Agent0, arXiv:2511.16043] (curriculum+executor co-evolve from zero
data), [Test-Time Tool Evolution, arXiv:2601.07641], LATM (tool creator/user). **But all invent tools to
*use at runtime*, for tasks the model mostly already handles.** None of them:
1. tie tool/action-space invention to a *measured capability floor* (cold≈0 AND taught≈0 — can't do it
   even *with* the rule/help), and
2. **distill the tool's leverage back into the policy's weights** — internalize the new capability rather
   than depend on the tool at inference.

That two-part loop — **detect floor → reformulate action space → distill the crossing to weights** — is
unclosed in the literature. It's exactly the loop Yogi can close (we have floor-detection via the
verifier-grounded capability map, the action-space lever from F6, and the distill ratchet).

## The thesis under test
> Base capability isn't the ceiling; the *action space* is — and the lift can be **internalized**, so the
> model needs the scaffold to *learn* the skill but not to *exercise* it afterward.

## Phase 1 (scoped, feasible now): autonomous *selection* among reformulations
Full autonomous *invention* of novel DSLs is phase 2. The tractable first step is autonomous **selection**
— give the being a small menu of reformulation *operators* and let it pick by measuring which crosses the
floor. This is already novel (selection-by-floor-crossing + distill-back) and de-risks the hard version.

**Setup.** Pick 2–3 below-floor tasks (verified cold≈0 *and* taught≈0 on qwen3-8b), e.g.:
- ASCII drawing (have it — F6),
- multi-digit multiplication done in one shot (8B fails ≥3-digit), or a multi-step parse/format task.

**Reformulation operators (the menu):**
- `R0 = direct` (baseline — the floor),
- `R1 = emit-a-program` + deterministic executor (the F6 lever),
- `R2 = decompose-into-verified-substeps` (scratchpad of checkable steps),
- `R3 = tool-call` (delegate the hard part to a primitive).

**The loop, per task:**
1. **Detect floor:** confirm cold≈0 ∧ taught≈0 under `R0` (verifier-grounded).
2. **Select:** measure taught-pass under each `Ri`; pick the `Ri*` that lifts taught-pass above threshold
   (autonomous selection by the free verifier — no human picks it).
3. **Ratchet:** self-generate verified traces *through* `Ri*`; distill cold(task)→solution into weights.
4. **Evaluate the internalization claim — the crux:** post-distill, test the model **without** the
   scaffold where possible (e.g. for `R2` decompose: does it now do the steps itself; for `R1`: does it
   emit programs natively — F6 already shows yes 6/6). Report: floor-crossed? internalized-vs-scaffold-
   dependent?

**Metrics.**
- *Selection accuracy:* does measuring-taught-pass reliably pick a reformulation that crosses (vs `R0`)?
- *Crossing rate:* fraction of below-floor tasks lifted above threshold by the selected `Ri*`.
- *Internalization:* post-distill pass with scaffold removed / reduced (the novel claim vs runtime-tool crowd).

## Phase 2 (the real frontier, after Phase 1): autonomous *invention*
The being *generates* a candidate reformulation (a small DSL / decomposition schema) rather than picking
from a menu — proposes, tests taught-pass via the free verifier, keeps the one that crosses, distills.
This is "meta-evolution inside the being." Gate it on Phase 1 working.

## Risk & kill-criterion
High risk: (a) selection might not beat a fixed always-`R1` baseline (then the contribution shrinks to
"reformulation+distill works," still real but less); (b) internalization might *not* survive scaffold
removal for `R2`/`R3` (then the honest result is "F6's program-emission is the one that internalizes;
others stay scaffold-dependent" — a clean boundary). Either way: a recordable result. Salary: `R1`/F6 used
a capped teacher; the rest can use the free verifier if the 8B's taught-pass under `Ri` is high enough.

## Why this is the bet that would surprise people
Everyone is racing capability *or* building runtime tool-users. The claim "a model can permanently raise
its own floor by reformulating its action space and *internalizing* the lift" reframes what 'capability'
even means for a fixed base model — and we already have the one datapoint (F6) that says it's possible.
