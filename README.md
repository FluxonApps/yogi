# Yogi

An independent, self-evolving being — built as a **trust-native, self-distilling agent runtime with
a falsification bench that decides whether the "being" claims actually hold.**

The honest one-paragraph version: an LLM lacks three capacities a living thing has — **metabolism**
(maintaining itself against a resource gradient), **intrinsic goals** (wanting because wanting was
selected for), and **heredity-with-selection** (changing across generations by copies / variation /
fitness / death). Yogi supplies the plumbing for all three around a swappable model, and — crucially
— ships the instruments to *test whether that plumbing does causal work* rather than asserting it.
In this build the substrate, the bench, and the full evolutionary arm are all built; the "metabolism /
evolution / being" *language* stays calibrated to what the bench certifies — demonstrated where the
evidence is live (compounding, open-ended recombination), and held to "efficiency-only" where it isn't
yet (selection-vs-drift returned a principled null; value capture awaits a deployed external payer).

## Status

**M0–M6 all built** (22 crates, 206 tests, clippy clean). The operator lifted the M6 selection gate,
so the open-ended-search arm is **on**: the substrate, metabolism (it lives and dies by its budget),
real-model proposer + falsification bench, semantic-memory compounding, both distillation routes,
bounded self-modification (closed mutation surface), the §3.9 dynamic trust model, capability isolation
(a real `wasm32` executor guest), durable crash-recoverable persistence, and a live economic population
with reproduction + death all exist and are tested. Run `yogi status` for the live summary.

**Calibrated claims (the honest part):** the live open-ended-search result is real — on a transfer
corpus, recombination assembles an all-3-skill solver (cold 0.00 → recombinant 1.00, reproduced), and
a being earns its keep only by genuinely-verified success against a being-exogenous payer. The formal
**selection-vs-drift** acceptance ran live and returned a *principled null* (selection ≡ drift where
the niche determines fitness — the anti-theater gate correctly declined to claim a win it didn't earn).
Value capture stays labeled **efficiency-only** until a genuinely external (operator-unrepriceable)
payer is deployed — that, and OS-keystore key storage, are deployment, not code.

**To continue the project autonomously, read [`docs/HANDOVER.md`](docs/HANDOVER.md)** (the runbook),
then [`CLAUDE.md`](CLAUDE.md) and [`docs/MILESTONES.md`](docs/MILESTONES.md).

## Run it

```
yogi status        # what's built (no model, instant)
yogi run [turns]   # drive a durable, capability-sandboxed being on qwen3:8b (foreground)
```

Foreground demonstrations (each loads the local model): `cargo run -p being-bench --bin full_being`
(one being that is durable + sandboxed + model-backed, recovering its signed journal across a restart),
`--bin population_live` (economic natural selection on the live model), `--bin evolve_transfer`
(MAP-Elites open-ended search), `--bin compound` (Day-0 vs Day-N compounding).

## What is actually being built first (v0)

A buildable substrate, not a guaranteed digital lifeform:

- A Samkhya-framed runtime: perception → **propose / commit / attest** seam → memory → executor,
  every step signed and journaled.
- A microdollar **Account** with an operator-owned **reaper** (insolvency ⇒ terminal bind).
- Per-domain **self-distillation** (LoRA on a frozen base) — the "earned intelligence" engine.
- A **falsification bench**: does capability compound (model held constant)? does the harness do
  causal work, or is it an accounting wrapper?

The across-generation arm (population + selection) was originally gated off until the bench showed
compounding; **the operator lifted that gate**, and it's now built and on — a live economic population
with the full operator set (mutation + crossover + selection + death) over durable signed heredity. The
closed mutation surface still bounds every child, so no forbidden power is representable regardless.

## Compounding layers (how the being gets better, model held constant)

"Compounding" = does the being improve over time *without* changing the underlying model? Yogi stacks
several layers, cheapest/safest first. **Token-space layers run with no weight updates; the navigator
and routing do no model inference at all** (legal inside the automated test loop).

| Layer | What compounds | Status |
|---|---|---|
| Episodic memory | retrieval of prior turns (hybrid: embedding + IDF-lexical, so rare/exact tokens hit) | **live, wired** |
| Verifier-fed skills | generalized lessons, gated on the bench verdict, retrieved into future turns | **live, wired** |
| Navigator / routing | Think vs NoThink per task — heuristic, plus an outcome-learned router (pass/fail per task-class) | **live** (`being-router`) |
| Per-domain distillation | token-space (rule-in-prompt skills) **and** weight-space (LoRA/QLoRA on a frozen base) | **both live** — token-space in-loop; weight LoRA promotes 0→8/12 held-out (foreground, [`scripts/distill_lora.sh`](scripts/distill_lora.sh)) |
| Population + selection | heredity across generations, selected economically (earn → reproduce, insolvent → reaped) | **live** (`being-colony::Population`: mutation + crossover + selection + death; gate lifted by operator) |

**Certified ([`docs/FINDINGS.md`](docs/FINDINGS.md)):** on a cold-failing made-up-operation *transfer*
corpus, a learned rule lifts the being **0.000 → 1.000** (CI=[1,1], `compounds=true`) — genuine
transfer (new operands; the answer is never stored), end-to-end through the being's own
retrieve→apply loop. The load-bearing lesson: reasoning tasks must run with **thinking on** (`/no_think`
strangles rule application).

## The two master gates

1. **Anti-theater gate** — the operator lifted the "research arm stays off" gate, so selection is
   built and on; but the *claims* stay calibrated by the bench. The live selection-vs-drift acceptance
   returned a **principled null** (selection ≡ drift where the niche determines fitness), so the
   open-ended-search result rests on the demonstrated recombination/illumination, not on a
   selection-beats-drift claim the gate didn't certify.
2. **Exogenous-payer step-0** — the payer is wired to metabolism (a being earns its keep only by
   verified success against a payer *structurally exogenous to it*), but until a payer the operator
   cannot reprice is *deployed*, value-capture stays labeled **efficiency-only**. That step is
   deployment, not code.

## Repository map

| Path | What |
|---|---|
| [`docs/architecture.md`](docs/architecture.md) | **Canonical architecture & research spec.** The full design, including its own falsification targets and the open trade-space (nothing pre-rejected). |
| [`docs/build-spec.md`](docs/build-spec.md) | **Buildable v0 spec.** Committed decisions, crate DAG, interface contracts, SQLite schema, the per-step state machine, milestones M0–M6 with acceptance tests, and the gates that unlock the research arm. |
| [`docs/HANDOVER.md`](docs/HANDOVER.md) | **The runbook** — how to continue the project autonomously: current state, the loop, the hard rules, the gates, foreground commands. Read this first. |
| [`docs/MILESTONES.md`](docs/MILESTONES.md) | Durable milestone tracker (M0–M6), the source of truth for "what's next". |
| [`docs/decisions.md`](docs/decisions.md) | Append-only decision log (D-M1…D-M5), web-researched, with citations + revisit triggers. |
| [`docs/architecture-annotated.md`](docs/architecture-annotated.md) | *Internal.* The earlier annotated draft, kept only for its source citations and provenance map. Superseded by `architecture.md`. |

## Build order (from the build spec)

- **M0** Substrate skeleton (identity, journal, memory, the typed seam; echo proposer)
- **M1** Metabolism plumbing (Account, reserve/settle, reaper, the state machine)
- **M2** Real proposer + the bench (the first milestone where "does it compound?" is measurable)
- **M3** Learning layer (consolidation, distillation flywheel, forgetting gate)
- **M4** Self-modification (closed mutation surface + Two-Gate)
- **M5** Value source (one concrete payer + acceptance grader; makes the economic gate live)
- **M6** Research arm — *built and on* (gate lifted by the operator): MAP-Elites illumination + a live
  economic population with mutation + crossover + selection + death, over durable signed heredity.

## Stack

Rust workspace (type-level closure of the mutation surface; a real `wasm32` executor guest under a
Wasmtime/broker sandbox; Ed25519 + W3C `did:key` identity; blake3 hash-chain signing; a durable,
crash-recoverable append-log for the journal/ledgers; a `Beta`-per-effect-class trust model). **Built
and run entirely locally — no CI, no cloud, no API keys:**
the runtime's local proposer + shared embedding are served by **Ollama** (`qwen3:8b`,
`nomic-embed-text`), and the frontier/teacher tier is the **Claude Code agent**. See `CLAUDE.md` for
the local build harness and build spec §0 for the committed decisions (all revisable).

## License

TBD.
