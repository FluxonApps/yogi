# Yogi — Milestone Tracker

**The durable source of truth for milestone state** (git-tracked; survives sessions and context
compaction). Build order and acceptance tests come from [`build-spec.md`](build-spec.md) §6.

## How to continue (the rule)

- **One `/goal` per milestone.** The `Stop` hook (`.claude/scripts/verify.sh`) enforces
  `cargo test --all` + `cargo clippy -- -D warnings` green before any turn can end.
- **To continue at any point:** implement the **first unchecked item below**, keep the build green,
  then tick it here and commit. This is what "continue" resolves to regardless of context window.
- **Review gates:** pause autonomous building and review the diff at **⚠ safety-critical** and
  **🔒 gated** milestones before proceeding. Do not autopilot anything that can end or fork a being.

Legend: `[x]` done · `[~]` in progress · `[ ]` todo · `⚠` safety-critical (human review) ·
`🔒` gated (entry condition must hold).

---

## M0 — Substrate skeleton  `[x] complete`
*Goal: a turn runs end-to-end with an echo proposer; every step signed + journaled; replay of the
committed tail is deterministic.* — **met** (26 tests, clippy clean; commits c0e59c8…<runtime>).

- [x] `being-core-types` — provenance no-launder ladder, microdollar/Did/Hash/Sig types · commit `0471f48`
- [x] `being-core-mutation` — the **closed `MutationKind` surface** + `apply` (no wildcard) · commit `0471f48`
- [x] `being-core-id` — DID + Ed25519 signer + `verify` (ed25519-dalek), deterministic-from-seed · 5 tests
- [x] `being-core-journal` — single-writer-per-DID append, blake3 hash-chain, signed entries, replay, `verify_chain` · 7 tests *(in-memory; SQLite+fsync at M1/§5)*
- [x] `being-core-memory` — episodic (bitemporal, no-launder by construction) / semantic (consolidated=ModelInference) / procedural (population variants) · 4 tests *(in-memory; SQLite at M2)*
- [x] `being-runtime` seam — `Proposer`/`Committer`/`Executor` + control loop, **echo proposer**; end-to-end `turn()` (signed commitment+attestation, deterministic committed tail) · 5 tests
- **Acceptance:** turn completes; all steps signed+journaled; committed-tail replay deterministic;
  property tests — 7th `MutationKind` variant won't compile, no-launder holds, single-head-per-DID.

## M1 — Metabolism plumbing  `[~] ⚠`  · decisions [D-M1-1/2/3](decisions.md)
*Account + supervisor `reserve`/`settle` + the per-step state machine + the **reaper**.*

- [x] `being-core-economy` — single-ledger Account: maintenance-first, `reserve_floor` + per-bet cap, category telemetry, credit-only inflow (D-M1-2) · 6 tests
- [x] `being-supervisor` — `SupervisorPort` façade, private authority, out-of-band watchdog thread, irreversible `Death`/reaper (insolvency · timeout · operator kill), first-cause-wins (D-M1-1, D-M1-3) · 6 tests
- [ ] per-step state machine + crash recovery
- [x] wire into `being-runtime` turn — heartbeat → reserve operating cost → commit → (if affordable) execute → attest; insolvency mid-turn trips the reaper (Death journaled), dead/killed beings refuse all turns · 7 tests
- [ ] per-step state machine + crash recovery (reserve→dispatch→attest→settle; build-spec §5/App. A) — refinement

- **Acceptance:** `reserve` rejects over-cap (budget binds); reaper fires on sustained insolvency and
  journals a Death event; out-of-band kill meets a measured latency bound; in-flight egress ≤
  `min(B_inflight, per-turn effect-count cap)` under stale-replica fuzzing.

## M2 — Real proposer (Ollama) + the bench  `[x]`
*`being-proposer-ollama` (`qwen3:8b` @ localhost:11434) + the falsification bench. Inference is
foreground/user-run only (16 GB budget); the automated loop never loads a model.*

- [x] `being-proposer-openai` — **generic** OpenAI-compatible chat `Proposer` (Ollama/vLLM/llama.cpp/…);
  backend specifics in `OpenAiChatConfig` with an `ollama_qwen3()` preset; request-build + parse +
  `<think>` strip (6 unit tests, no network); live call behind `live-model` feature, foreground-only
- [x] `being-bench` — pure machinery (frozen suite, scoring, **deterministic paired-bootstrap CI**
  + `improves_monotonically` gate, anti-theater arm comparison) · 6 unit tests, no model; foreground
  `bench` binary scores a live being (`cargo run -p being-bench --bin bench --release`). Longitudinal
  Day-N compounding signal is exercised at M3 once learning accrues.
- **Acceptance:** bench runs Day-0 vs Day-N with the model held constant and emits a paired-bootstrap
  CI; the anti-theater harness runs all three arms and produces a report (null result is valid).

## M3 — Learning layer  `[x]`  · decisions [D-M3-1/2](decisions.md) (retrieval-first; distillation gated)
*Token-space compounding first (retrieval + consolidation + verifier-fed skills); per-domain
distillation is an optional foreground arm. Build order: retrieval → embedder → consolidation →
skill-learning(verifier) → wire + Day-N bench.*

- [x] semantic-retrieval core — `cosine_similarity` + `SemanticIndex` (score = α·cos + (1−α)·0.5^(age/h)); stale-but-similar guard tested · 4 tests
- [x] generic `Embedder` trait (in `being-core-memory`) + `being-embed-openai` (live `nomic-embed-text`
  behind `live-model`, foreground; build/parse unit-tested, no network) · 4 tests · hybrid BM25/RRF deferred
- [x] `Consolidator` (episodic→semantic; deterministic `FrequencyConsolidator`, idempotent) +
  verifier-fed skill-learning (`ProceduralStore::learn_from`/`best_for`: branching `[ok]`/`[fail]`
  variants keyed by task class; latest passing wins) · 3 tests
- [x] wired semantic retrieval into `Being::turn` (optional embedder; embed input → cosine+recency
  search → accumulate into the index → memory compounds; episodic fallback). Test proves turn-2
  surfaces turn-1 memory (stub embedder, no model). Foreground `compound` bin runs the Day-0 vs Day-N
  paired-bootstrap demo (`cargo run -p being-bench --bin compound --release`).
- **Acceptance:** distillation closes the gap on `(teacher-success ∩ student-weak)` for ≥1 domain by
  the pre-registered per-domain margin; every `DomainModel` promotion re-clears the mixed-set
  non-inferiority floor; compounding bench detects accumulation or reports saturation.

## M4 — Self-modification  `[ ] ⚠`
*`Improver` + closed surface + **Two-Gate** (Validation + Capacity) + self-judgment-bias mitigation.*
- **Acceptance:** a genome mutation passes both gates, is signed/journaled/reversible; Capacity-Gate
  false-admit rate ≤ the Validation Gate's false-discovery budget.

## M5 — Value source (makes the economic gate live)  `[ ] ⚠`
*One concrete payer (operator-as-customer: tariff + arrival + held-out anti-Goodhart grader) +
exogenous-payer hook.*
- **Acceptance:** external-revenue ledger live and bounded by inflow; the anti-theater gate moves
  from methodology to a live derived threshold; value-capture claims labeled efficiency-only until an
  exogenous payer is committed.

## M6 — Research arm (population + selection)  `[ ] 🔒`
*Fork saga + lineage built as substrate; **selection stays OFF** until the entry gate holds.*
- **Entry gate:** the compounding bench shows accumulation **AND** the anti-theater gate fires.
- **Acceptance (when entered):** a fork is a signed, crash-recoverable distributed snapshot; the
  post-exhaustion fitness-variance gate distinguishes signal from a neutral-drift control at stated
  power, or the "breeding-program-not-evolution" result is reported.

---

## Ready-to-paste `/goal` conditions

**M0** (non-safety — fine to run unattended):
```
/goal Yogi M0 complete: being-core-id (DID + Ed25519 signer/verify), being-core-journal
(single-writer-per-DID append + hash-chain + replay), being-core-memory (episodic/semantic/
procedural), and being-runtime (Proposer/Committer/Executor + control loop with an echo proposer)
all exist and wire into one end-to-end turn; cargo test --all and cargo clippy --all-targets
-- -D warnings clean (show the output); MILESTONES.md M0 ticked; work committed.
```

**M1** ⚠ (review the diff before continuing past it):
```
/goal Yogi M1 complete: being-core-economy Account + being-supervisor reserve/settle authority +
the per-step state machine + the reaper; reserve rejects over-cap, the reaper fires on insolvency
and journals a Death event, and the in-flight-egress bound holds under stale-replica tests;
cargo test --all + clippy clean (show output); MILESTONES.md M1 ticked; committed.
```

(M2–M6 goal strings to be written as each is reached; M6 stays gated.)
