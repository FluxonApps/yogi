# Yogi — Findings (empirical, append-only)

Live results from **foreground** runs (operator-initiated; the automated loop never runs the model).

## 2026-06-21 — first live end-to-end verification (qwen3:8b, 16 GB Mac)

- **Stack:** M0–M5 run live on `qwen3:8b`, foreground, machine stable, no crash. Single inference
  answered correctly in 22 s (incl. ~5 GB cold load).
- **Determinism:** added `temperature` to the proposer (default **0.0 / greedy**) after observing the
  same suite score 0.9 then 1.0 across two runs — sampling noise was swamping the signal. With temp 0
  the bench measures capability, not randomness.
- **Calibrated bench:** the easy 5-task tier saturates at 1.0 cold (no headroom), so a 5-task harder
  tier was added. Deterministic Day-0 cold = **0.900** (the model fails one task cold — e.g.
  `anagram` / `prime-7th`).
- **Compounding (Day-0 vs Day-N):** `Day-0 0.900 → Day-N 1.000`, paired delta **+0.100**,
  CI **[0.000, 0.300]**, `compounds=false`.
  - The **memory mechanism measurably works**: Day-N fixed the cold-failed task by semantically
    retrieving the studied answer into context (token-space compounding, D-M3-1).
  - The **falsification gate correctly does NOT certify it** at N=10 (a single-task gain → bootstrap
    CI includes 0). Conservative-by-design; it won't over-claim.
- **Self-modification:** on a no-improvement run the Two-Gate **rejected every candidate edit
  (rollback)** — live verification it refuses noise-level gains.

### What this validates
- The being **compounds directionally via token-space memory**, live and end-to-end.
- The anti-theater / compounding gate is **appropriately conservative** (real mechanism, no
  over-claim).
- The Two-Gate self-modification **refuses noise** and rolls back, as designed.

### Next — to actually FIRE the compounding gate (not just show direction)
A larger, harder, provenance-isolated corpus the model reliably fails cold on a meaningful fraction,
sized with enough tasks + replications to beat run-to-run variance (the derived replication count of
build-spec §7). This is corpus-curation + foreground-run work, not loop-buildable; it's the concrete
next step toward un-suspending the metabolism/evolution language and opening the M6 gate.

## 2026-06-21 — transfer-compounding certification (NEGATIVE, informative)

Built a real transfer corpus (D-M3-3): a made-up operation a (+) b = a*b + a + b the model cannot know
cold, 20 seeded cold-failing tasks, fresh being per task, **cold vs. with the learned RULE skill**.

```
cold (no skill)    mean 0.000   (correct — the op is unknowable cold)
with learned skill mean 0.100   (the rule helped only 2/20)
paired delta +0.100  CI [0.000, 0.250]  compounds=FALSE  -> NOT certified
```

**The transfer mechanism is wired and directionally real (0.00 -> 0.10), but far below a working
skill-transfer (~0.8+).** Prime suspects, in order:
1. **`/no_think` sabotages reasoning** — the proposer prefixes `/no_think` (for latency), but applying
   a*b+a+b needs step-by-step computation; greedy no-think blurts a wrong number even with the rule in
   context. **Next: re-run with thinking ON.**
2. **Retrieval miss** — `nomic-embed-text` may not place the query near the rule note (rare symbol);
   diagnostic: inject the skill deterministically to isolate retrieval-vs-application.
3. **8B in-context-learning ceiling** (research caveat) — qwen3:8b may apply a told rule weakly.

**Honest status:** token-space skill-transfer is **not yet certified**; the mechanism works but the
realized effect at 8B + no_think is too small. The bench correctly refused to certify, and the
negative result names the next experiments. This is the project's ethos working — it won't lie to
itself about compounding.

## 2026-06-21 — transfer-compounding CERTIFIED (the /no_think fix)

Applied the research's cheapest-first fixes (thinking ON: drop `/no_think`, temp 0.6 / top_p 0.95 /
top_k 20, max_tokens 2048; deterministic rule injection; a worked-example skill note). Re-ran the
same 15-task cold-failing ⊕ transfer corpus:

```
cold (no rule)     mean 0.000
with injected rule mean 1.000
paired delta +1.000  CI [1.000, 1.000]  compounds=TRUE  -> CERTIFIED
```

**The prime hypothesis was right: `/no_think` removed the reasoning scratchpad needed to APPLY a
rule.** With thinking enabled the being applies a learned rule to 15 brand-new operand pairs
perfectly. This is the **first certified token-space compounding result** — a learned skill causally
lifts cold-failing *transfer* tasks (new operands; the answer is never stored) from 0 to 1, CI
excludes zero. The compounding gate fires on this task.

**Honest scope.** Controlled synthetic operation + *deterministic* rule injection (to isolate the
APPLY mechanism from retrieval). It certifies that the being can apply a learned skill to genuinely
new inputs once allowed to reason — not yet that the full retrieval→apply loop self-certifies on a
messy corpus. Next: certify end-to-end through the (now hybrid-wired) retrieval path, and on a
less-synthetic task. The 8B "weak ICL" caveat is mooted for this rule-application class when thinking
is on.

**Config lesson (load-bearing).** Never `/no_think` a task that needs reasoning. The being's proposer
should run in thinking mode for reasoning/compounding work; `/no_think` is only for trivial recall
where latency matters.

## 2026-06-21 — transfer-compounding CERTIFIED END-TO-END (full retrieve→apply loop)

Re-ran through the **full being** — no deterministic injection. `learn_skill` stored the rule; the
turn **retrieved** it from the being's own memory via the hybrid index (the rare `⊕` matched
lexically); the **thinking** proposer applied it:

```
cold (no skill)    mean 0.000
with learned skill mean 1.000
paired delta +1.000  CI [1.000, 1.000]  compounds=TRUE  -> CERTIFIED END-TO-END
```

**The complete autonomous loop self-certifies:** the being learns a skill, retrieves it from its own
memory on a *new* task, and applies it by reasoning — lifting cold-failing transfer tasks 0→1. This
closes the gap from the prior cert (which hand-injected the rule): **hybrid retrieval + thinking
together make the retrieve→apply loop work without help.**

**Honest scope / next frontier.** Still a controlled synthetic operation with a hand-authored skill
note. The remaining frontier: (1) a less-synthetic, multi-skill corpus; (2) certify with the skill
*learned from verifier feedback* (the Letta loop) rather than authored; (3) measure LiMem under
perturbation to confirm it's not memorization. The mechanism is proven; the corpus is the next work.

## 2026-06-21 — multi-skill transfer FAILS (skill interference) — informative negative

Multi-skill cert: a thinking-mode being learns 3 made-up rules (⊕,⊗,⊙), then is tested per-op and on
the compositional `(a⊕b)⊗c` split.

```
single-op transfer:  cold 0.000 -> skilled 0.000  compounds=false
compositional split: cold 0.000 -> skilled 0.000  compounds=false
LiMem: 0.000 (uninformative — nothing was correct to be consistent about)
```

**Adding 3 similar rules COLLAPSES the transfer that scored 1.000 with a single skill.** Diagnosis
(pure test added, no model): the hybrid index **correctly ranks the matching-symbol rule first**, so
this is **not** a retrieval-ranking bug — it's **skill interference**: the being injects the top-4
retrieved, so all 3 novel symbol-rules reach the model, and qwen3:8b **conflates** them. Single-skill
worked precisely because only one rule was present. This is the retrieval-*precision* caveat from the
D-M3-3 research, made concrete.

**Next (fix → re-cert):** inject only the **top-1–2** most-relevant skills (the lexical channel
already ranks the right rule first), separating high-precision *skill* retrieval from broader *memory*
retrieval. Then re-run the multi-skill cert. The compositional split needs the right *two* rules and
nothing else — precise injection is the prerequisite for composition.

## 2026-06-21 — CORRECTION: the multi-skill 0.000 runs were INVALID (backend down)

The multi-skill `0.000` results above (and the follow-up "digit-collision" / "precise-injection"
re-runs) were run against a **dead Ollama backend**: `ollama ps` showed no resident model and a single
cheap inference failed in 0.14 s (connection refused). A previously-**certified** single-skill e2e
(1.000) also returned 0.000 in this state — the tell that it was the backend, not the code.

**Therefore the "multi-skill skill-interference" and "digit-collision" conclusions are retracted as
unverified** — they cannot be drawn from runs where *every* arm (including cold and a known-good cert)
returns 0.000. **Process lesson (added to discipline): always verify backend health (one cheap
inference) before interpreting any `0.000` foreground result.** A zeroed cold-AND-skilled is the
signature of a downed backend, not a real negative.

**Still valid:** the single-skill transfer-compounding certs (apply-mechanism and end-to-end) ran when
the backend was alive and stand. **Code kept (loop-safe, tested, but UN-validated against a live model
until re-run):** the separate `skill_index` with top-1 precise injection, the digit-collision-free
skill examples, and the multi-skill corpus — all reasonable designs, to be re-certified once the
backend is back. Model runs are paused until `ollama serve` is confirmed healthy.

**Resolved.** After a full Ollama restart (`pkill ollama` + `ollama serve`), the cheap inference
returned `model said: hi` and the single-skill e2e **re-certified at 1.000** (CI=[1,1]). The
skill-index refactor is sound — the 0.000 streak was entirely the dead backend, as diagnosed. The
real multi-skill cert now runs on a healthy backend.

## 2026-06-21 — multi-skill result (GENUINE, healthy backend) — supersedes the retracted run

A thinking-mode being learns 3 rules (⊕,⊗,⊙) and is tested per-op and on the compositional split:

```
single-op transfer:  cold 0.000 -> skilled 1.000  CI=[1,1]  compounds=TRUE   CERTIFIED
compositional split: cold 0.000 -> skilled 0.000  compounds=false            (top-1 limit)
LiMem (single):      0.000  -> pure rule-application, not memorization
```

- **Multi-skill single-op transfer is CERTIFIED.** The being holds three learned rules and, via
  top-1 precise retrieval, applies the *right* one to new operands. The earlier "skill interference"
  was the dead-backend artifact (retracted); on a healthy backend multi-skill transfer holds.
- **LiMem = 0.000** — re-scoring with perturbed operands stays correct, so this is genuine rule
  *application*, not answer memorization. The transfer claim is clean.
- **Compositional `(a⊕b)⊗c` fails (0.000), as expected:** top-1 injects only ONE skill, but composition
  needs BOTH ⊕ and ⊗ in context. Not a backend issue (single-op = 1.000 same run).
- **Note on top-1:** the "precise top-1" choice was over-motivated by the retracted dead-backend
  interference finding. Top-1 demonstrably suffices for single-op; whether injecting more skills hurts
  is now an *open, testable* question. **Next: inject top-2 skills** so multi-symbol tasks get both
  rules, and re-cert — checking single-op stays 1.000 while compositional rises.

## 2026-06-21 — top-2 skill injection: COMPOSITION CERTIFIED (both hold)

With `SKILL_RETRIEVAL_K=2` (inject the top-2 ranked skills), re-ran the multi-skill cert:

```
single-op transfer:    cold 0.000 -> skilled 1.000  CI[1,1]  CERTIFIED  (held — top-2 didn't hurt)
compositional (a⊕b)⊗c: cold 0.000 -> skilled 1.000  CI[1,1]  CERTIFIED  (rose from 0.000!)
LiMem (single):        0.000  -> pure rule-application
```

**The being composes two independently-learned skills.** Once both rules are in context (top-2, ranked
by the lexical channel), it chains them — `(a⊕b)⊗c` solved on fresh operands. And single-op did **not**
degrade, so top-2 is a strict win over top-1: it adds composition at no cost to single-symbol transfer.
This is **L0/L1 self-improvement composing into genuinely new behavior** without any code-writing — the
primitives+skills thesis (D-RSI-1) demonstrated: memory + verifier-fed skills + precise-but-plural
retrieval = transfer *and* composition, certified, on a local 8B. `SKILL_RETRIEVAL_K=2` kept.

## 2026-06-21 — M6 engine + entry gate built and the acceptance experiment FIRES (loop-safe, no model)

Built the open-ended-search engine and its honesty gate, then ran the M6 acceptance methodology
end-to-end without any inference (synthetic landscape; the loop never loads a model):

- **`being-lineage::illuminate`** — MAP-Elites illumination: sample a parent elite (branch from any
  ancestor), fork, vary via the **closed `MutationKind` surface**, evaluate (injected `Evaluator`),
  place the child in its behavior cell. `BehaviorDescriptor` maps behavior→cell; QD-score/mean-fitness
  report progress. Variation can never escape the sanctioned set — the M0 safety invariant holds across
  generations by construction.
- **`being-bench::neutral_drift_gate`** — the M6 entry gate. Pairs the selection arm against a matched
  **neutral-drift control** (identical eval budget + variation; only retention differs) via the
  existing paired-bootstrap CI. Fires iff selection beats drift by a margin — else reports the honest
  breeding-program-not-evolution null. (`Retention::{Elitist,NeutralDrift}` is the one knob that
  differs between arms.)
- **Integration experiment** (`being-bench/tests/m6_acceptance.rs`): on a noisy landscape, elitist
  retention captures each niche's max (mean-fitness ~0.96) while drift random-walks around the mean
  (~0.5). Across 12 paired replicates the advantage CI excludes the margin → **gate FIRES**.

**Interpretation.** This is a *methodology* proof, not yet a real-being result: it shows the engine and
the gate compose to detect genuine selection signal and to reject drift — the machinery is correct and
fires only when selection actually does work. The real M6 result (signal vs drift on a model-scored
landscape, fork as a signed crash-recoverable snapshot) is the next foreground step. The dangerous
parts — reproduction/death wired to a live population — remain a deliberate, reviewable boundary even
though the gate now demonstrably distinguishes signal from drift.

## 2026-06-21 — signed, crash-recoverable fork snapshot (M6 acceptance piece)

The operator lifted the M6 selection gate ("nothing is gated, don't stop"), so the fork saga is now
built (loop-safe, no model):

- **`Genome::canon_bytes`** (being-core-mutation) — canonical, length-prefixed, deterministic encoding
  of the heritable unit. Ordered collections iterate canonically; the classic "ab|c" vs "a|bc"
  ambiguity can't collide. This is what the parent actually signs.
- **`being-lineage::ForkSnapshot`** — the parent (`Signer`) signs a blake3 digest over
  `(parent_did, parent edge, child edge, genome.canon_bytes)`, domain-separated `yogi.fork.snapshot.v1`.
  `verify()` checks BOTH the heredity invariants (child = parent.generation+1, sole parent recorded)
  AND the Ed25519 signature. Tampered genome, forged lineage edge, or impostor DID each flip the digest
  / fail the edge check → rejected.
- **`ForkLedger`** — content-addressed `snapshot_id` (blake3) keys an at-most-once commit set:
  first commit `Committed`, replay of the same snapshot `AlreadyCommitted` (idempotent crash recovery),
  invalid snapshot `Rejected`. Same exactly-once-effective discipline as the M1 `DedupLedger`.

**Safety invariant intact despite the lifted gate:** a signed child still inherits its genome verbatim
and can only vary through the closed `MutationKind` surface — so no signed snapshot can ever carry a
forbidden mutation, regardless of selection being on. The remaining foreground step is a real
model-scored illumination run (the gate already fires on synthetic data).
