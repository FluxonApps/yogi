# Yogi — Handover & Self-Improvement Runbook

This is the operating manual for continuing Yogi **autonomously**. It is written so a fresh agent
session (with no prior context) can pick up the project and keep improving it without hand-holding.
Read this, then `CLAUDE.md`, then `docs/MILESTONES.md`.

## What Yogi is

An independent, self-evolving being built as a **trust-native, self-distilling agent runtime with a
falsification bench**. Remove the model and a coherent skeleton remains: it types inputs, remembers,
enforces policy, **maintains its budget, and lives or dies by a fitness function** — that invariant is
the whole point. The honest framing: the headline "metabolism / evolution" claims stay **suspended**
until the bench's anti-theater gate fires (see Gates).

## Current state (M0–M5 built, M6 gated)

15 crates, ~79 tests, clippy clean, all committed under the operator's gitconfig.

| Milestone | crate(s) | what it gives the being |
|---|---|---|
| **M0** substrate | `being-core-types`, `-id`, `-journal`, `-memory`, `being-runtime` | typed percepts, Ed25519 identity, signed hash-chained journal, episodic/semantic/procedural memory, the propose→commit→attest seam + end-to-end `turn()` |
| **M1** metabolism `⚠` | `being-core-economy`, `being-supervisor` | single-ledger microdollar Account, reaper (insolvency/timeout/operator-kill) + out-of-band watchdog, wired so it **lives and dies by its budget** |
| **M2** mind + measure | `being-proposer-openai`, `being-bench` | generic OpenAI-compatible proposer (Ollama/qwen3 preset), falsification harness (bootstrap CI, anti-theater arms) |
| **M3** compounding | `being-embed-openai` + memory/runtime | semantic retrieval (cosine + recency), consolidation, verifier-fed skill-learning; retrieval wired into the turn so memory accumulates |
| **M4** self-modification `⚠` | `being-loop` | Two-Gate (Validation `2·ε_V+τ` + Capacity proxy) + epsilon-greedy Improver + `self_improve_round` (commit-or-rollback + audit) |
| **M5** value source `⚠` | `being-value` | operator-as-customer payer: tariff + held-out grader + inflow-bounded treasury + `ExternalPayer` hook |
| **M6** population/selection | `being-lineage` | **GATED.** Heredity substrate only (`Lineage` + `fork`, child inherits genome, gen+1); **selection/fitness/death OFF** until the entry gate. See Gates. |

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

## The Gates (what must hold before the risky steps)

1. **Anti-theater gate (master).** Until a real bench run shows the harness does causal work (not just
   a budget cap), the "metabolism/evolution/being" language stays suspended (build-spec §7, arch §13.1).
2. **M6 selection stays OFF** until the compounding bench shows accumulation **AND** the anti-theater
   gate fires. Both need real foreground runs — so M6 is correctly *not built*. When the gate opens,
   build fork/lineage substrate first, selection second, behind the simulation-gate discipline.
3. **Isolation upgrade (D-M4-2):** move the supervisor to a separate process + Wasmtime sandbox
   **before the executor runs any model-generated / external / tool code — never before.** Encode it
   as a hard gate pairing `Executor` with `OutOfProcessSupervisor + WasmSandbox`.
4. **Exogenous-payer step-0 (D-M5-1):** value-capture claims are **efficiency-only** until a payer the
   operator cannot reprice is committed. That commitment is what makes the economic gate *fire*.

## Two senses of "self-improvement" (don't conflate)

- **The project improves** via the autonomous build loop above (an agent builds the next milestone).
- **The being improves itself** via `being-loop::self_improve_round` (foreground): it proposes
  closed-surface genome edits, the bench verifies, the Two-Gate accepts only real gains, else rolls
  back — all audited. This is bounded, reversible, and operator-gated by construction.

## Immediate next steps (in order)

1. **Run the foreground bench/compound/selfimprove** to get the first *real* anti-theater + compounding
   numbers (the build loop can't — they need the model). Record results; they decide whether the
   metabolism/evolution language un-suspends and whether M6's gate can open.
2. If compounding + anti-theater hold → build M6 substrate (fork/lineage), selection still gated behind
   the simulation-gate discipline.
3. When the executor / tool-running lands → do the isolation upgrade (D-M4-2) first.
4. If a real exogenous payer becomes available → wire it via `ExternalPayer` (D-M5-1 step-0).
