# Yogi — Handover & Self-Improvement Runbook

This is the operating manual for continuing Yogi **autonomously**. It is written so a fresh agent
session (with no prior context) can pick up the project and keep improving it without hand-holding.
Read this, then `CLAUDE.md`, then `docs/MILESTONES.md`.

## What Yogi is

An independent, self-evolving being built as a **trust-native, self-distilling agent runtime with a
falsification bench**. Remove the model and a coherent skeleton remains: it types inputs, remembers,
enforces policy, **maintains its budget, and lives or dies by a fitness function** — that invariant is
the whole point. The honest framing: the headline "metabolism / evolution" *language* stays calibrated
to what the bench certifies — demonstrated where evidence is live, "efficiency-only" where it isn't yet
(see Gates).

## Current state (M0–M6 all built; selection live)

22 crates, 206 tests, clippy clean, all committed under the operator's gitconfig. The operator lifted
the M6 selection gate, so the open-ended-search arm is on. Beyond the M0–M5 spine, this session added:
- **M6 live** (`being-lineage`, `being-colony`): MAP-Elites illumination + a live economic `Population`
  with the full operator set — mutation + crossover (sexual `fork2`) + selection (by solvency) + death
  (reaper) — over durable signed heredity.
- **Both distillation routes** (`being-distill` + `scripts/distill_lora.sh`): token-space (in-loop) and
  weight/LoRA (foreground; 0→8/12 held-out), each through a promotion gate.
- **M4 isolation** (`being-sandbox`, `being-sandbox-wasm`): capability broker + a **real `wasm32`
  executor guest** (zero ambient authority) + `Being::from_seed_sandboxed` on the live turn path.
- **Durable persistence** (`being-persist`, `being-colony`): crash-safe append-log; durable journal /
  fork-ledger / dedup-ledger; a live **crash-recoverable** being (`durable_being`).
- **§3.9 trust model** (`being-core-policy`): `Beta`-per-`EffectClass`, earn-slow/lose-fast, statrs.
- **Policy gate** (`being-runtime::RiskPolicyCommitter`): static risk-ceiling self-restraint.
- **Real W3C `did:key`** identity; the **`yogi` CLI** (`being-bin`); `being-router` outcome-learned routing.

| Milestone | crate(s) | what it gives the being |
|---|---|---|
| **M0** substrate | `being-core-types`, `-id`, `-journal`, `-memory`, `being-runtime` | typed percepts, Ed25519 identity, signed hash-chained journal, episodic/semantic/procedural memory, the propose→commit→attest seam + end-to-end `turn()` |
| **M1** metabolism `⚠` | `being-core-economy`, `being-supervisor` | single-ledger microdollar Account, reaper (insolvency/timeout/operator-kill) + out-of-band watchdog, wired so it **lives and dies by its budget** |
| **M2** mind + measure | `being-proposer-openai`, `being-bench` | generic OpenAI-compatible proposer (Ollama/qwen3 preset), falsification harness (bootstrap CI, anti-theater arms) |
| **M3** compounding | `being-embed-openai` + memory/runtime | semantic retrieval (cosine + recency), consolidation, verifier-fed skill-learning; retrieval wired into the turn so memory accumulates |
| **M4** self-modification `⚠` | `being-loop` | Two-Gate (Validation `2·ε_V+τ` + Capacity proxy) + epsilon-greedy Improver + `self_improve_round` (commit-or-rollback + audit) |
| **M5** value source `⚠` | `being-value` | operator-as-customer payer: tariff + held-out grader + inflow-bounded treasury + `ExternalPayer` hook |
| **M6** population/selection | `being-lineage`, `being-colony` | **LIVE** (gate lifted). Heredity (`Lineage`/`fork`/`fork2`) + MAP-Elites illumination + a live economic `Population`: mutation + crossover + selection (solvency) + death (reaper), over durable signed heredity. Selection-vs-drift acceptance = principled null (see Gates). |

Post-M5 hardening (all green): **hybrid IDF-lexical + embedding retrieval** wired into the turn (rare
symbols like `⊕` retrieve reliably); `ollama_qwen3_thinking()` proposer preset.

**Live-verified** (`docs/FINDINGS.md`, 2026-06-21):
- M0–M5 run on `qwen3:8b` foreground; the being **compounds directionally** (Day-0 0.900 → Day-N
  1.000 via memory), with the gate correctly conservative (`compounds=false` at N=10 — no over-claim).
- **Token-space compounding CERTIFIED**: on a cold-failing made-up-operation *transfer* corpus, a
  learned rule lifts the being **0.000 → 1.000** (CI=[1,1], `compounds=true`) — genuine transfer (new
  operands; answer never stored), once thinking is on. Self-mod refuses noise (rollback).
- **Load-bearing config lesson:** never `/no_think` a reasoning task — it strangles rule application.

## How to continue autonomously (the loop)

1. **`docs/MILESTONES.md` is the source of truth.** To continue: implement the **first unchecked
   item**, keep the build green, then tick it and commit.
2. **Decisions go in `docs/decisions.md`** (append-only, web-researched where it matters, with
   citations + revisit triggers). At `⚠` safety-critical milestones, record the decision and surface
   it for review rather than deciding silently.
3. **Every change:** `cargo check` → `cargo test --all` → `cargo clippy --all-targets -- -D warnings`
   → `cargo fmt --all` → commit. The `Stop` hook enforces green; never claim done while red.
4. **Commit** from the repo root with plain `git commit` (the ambient gitconfig identity is correct;
   never `cd … && git …`, never `-c` identity overrides). End messages with the session trailer.

## The hard rules (from CLAUDE.md — do not violate)

- **No model inference in the automated loop.** `cargo test`/clippy/hooks must never call Ollama. The
  two live models (`qwen3:8b`, `nomic-embed-text`) load **only on foreground commands** (below).
- **One model at a time** (16 GB unified memory). Never run inference in the background.
- **Closed mutation surface:** `MutationKind` is closed by the type system; a forbidden variant
  (capability grant, trust-policy edit, kernel, budget rules, reaper) must be a *compile error*.
- **No-launder provenance:** nothing relabels to `DirectUserIntent`.
- **Operator owns the reaper + kill-switch + budget rules.** The being never does.

## Foreground commands (you run these; each loads a model, once)

```bash
cargo test -p being-proposer-openai --features live-model -- --nocapture   # watch it think (qwen3:8b)
cargo test -p being-embed-openai   --features live-model                    # live embedding (nomic-embed-text)
cargo run  -p being-bench --bin bench        --release                      # score a live being
cargo run  -p being-bench --bin compound     --release                      # Day-0 vs Day-N compounding demo
cargo run  -p being-bench --bin selfimprove  --release                      # bounded self-modification demo
```

## The Gates (status — operator lifted the build gates; claims stay calibrated)

1. **Anti-theater gate (master).** The operator lifted the "research arm stays off" build gate, so
   selection is built; but the *claims* stay bench-calibrated. Live: compounding (0→1 transfer) and
   open-ended recombination (all-3-skill solver) are demonstrated. The selection-vs-drift acceptance
   ran live and returned a **principled null** (selection ≡ drift where the niche determines fitness),
   so don't claim selection-beats-drift — the open-ended-search result rests on illumination/recombination.
2. **M6 selection: LIVE** (`being-colony::Population`) — gate lifted. The closed mutation surface still
   bounds every child, so no forbidden power is representable regardless of selection being on.
3. **Isolation upgrade (D-M4-2): DONE.** `being-sandbox` (broker) + `being-sandbox-wasm` (a real wasm32
   executor guest, zero ambient authority) + `Being::from_seed_sandboxed` gate effects on the live turn
   path. (Out-of-process supervisor remains a deployment hardening, not v0 code.)
4. **Exogenous-payer step-0 (D-M5-1):** the payer is **wired to metabolism** (`being_value::earn` →
   `supervisor.credit`; a being earns its keep only by verified success). Value-capture stays
   **efficiency-only** until a payer the operator *cannot reprice* is **deployed** — that's deployment.

## Two senses of "self-improvement" (don't conflate)

- **The project improves** via the autonomous build loop above (an agent builds the next milestone).
- **The being improves itself** via `being-loop::self_improve_round` (foreground): it proposes
  closed-surface genome edits, the bench verifies, the Two-Gate accepts only real gains, else rolls
  back — all audited. This is bounded, reversible, and operator-gated by construction.

## Immediate next steps (the v0 build is complete; remainder is not v0 code)

The substantive v0 spec is built (every named crate, all M0–M6 milestones, all evolutionary operators,
all three safety invariants test-encoded — verified by a workspace-vs-spec diff). What remains:

1. **Foreground validation (run-on-demand, the build loop can't — needs the model):** `yogi run`, and
   `cargo run -p being-bench --bin {full_being,population_live,evolve_transfer,compound}`. These produce
   live evidence; they're not run in the automated loop (they load the model).
2. **Deployment, not code:** a genuinely external (operator-unrepriceable) payer to fire the economic
   gate; OS-keystore key storage. Until then value-capture stays efficiency-only.
3. **Post-v0 integrations:** trust→lane gating (v0 fixes `lane_count=1`, so the §3.9 trust ledger is
   built as an isolated leaf — wiring it into the core would spread `nalgebra` workspace-wide for no v0
   benefit; do it when lanes>1 is actually needed).
4. **If continuing autonomously:** re-scan for genuine gaps (placeholders/`lands later` markers, spec
   acceptance criteria, self-review of fast-built code) before concluding there's nothing to do — that
   discipline repeatedly surfaced real work this session (see `docs/FINDINGS.md`).
