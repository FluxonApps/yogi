# Yogi

An independent, self-evolving being — built as a **trust-native, self-distilling agent runtime with
a falsification bench that decides whether the "being" claims actually hold.**

The honest one-paragraph version: an LLM lacks three capacities a living thing has — **metabolism**
(maintaining itself against a resource gradient), **intrinsic goals** (wanting because wanting was
selected for), and **heredity-with-selection** (changing across generations by copies / variation /
fitness / death). Yogi supplies the plumbing for all three around a swappable model, and — crucially
— ships the instruments to *test whether that plumbing does causal work* rather than asserting it.
In the first build, the deliverable is the substrate plus the bench; the "metabolism / evolution /
being" language stays suspended until the bench's anti-theater gate fires.

## Status

**Pre-alpha. No code yet.** This repository currently holds the specifications. Building starts at
milestone M0 (see the build spec).

## What is actually being built first (v0)

A buildable substrate, not a guaranteed digital lifeform:

- A Samkhya-framed runtime: perception → **propose / commit / attest** seam → memory → executor,
  every step signed and journaled.
- A microdollar **Account** with an operator-owned **reaper** (insolvency ⇒ terminal bind).
- Per-domain **self-distillation** (LoRA on a frozen base) — the "earned intelligence" engine.
- A **falsification bench**: does capability compound (model held constant)? does the harness do
  causal work, or is it an accounting wrapper?

The across-generation arm (population + selection) is built as substrate but **selection stays off**
until the bench shows compounding. That ordering is the whole point.

## The two master gates

1. **Anti-theater gate** — until it fires, Yogi is honestly "an LLM + accounting + a safety harness,"
   and the research arm stays off.
2. **Exogenous-payer step-0** — until a payer the operator cannot reprice exists, economic and
   value-capture claims are labeled **efficiency-only**.

## Repository map

| Path | What |
|---|---|
| [`docs/architecture.md`](docs/architecture.md) | **Canonical architecture & research spec.** The full design, including its own falsification targets and the open trade-space (nothing pre-rejected). |
| [`docs/build-spec.md`](docs/build-spec.md) | **Buildable v0 spec.** Committed decisions, crate DAG, interface contracts, SQLite schema, the per-step state machine, milestones M0–M6 with acceptance tests, and the gates that unlock the research arm. |
| [`docs/architecture-annotated.md`](docs/architecture-annotated.md) | *Internal.* The earlier annotated draft, kept only for its source citations and provenance map. Superseded by `architecture.md`. |

## Build order (from the build spec)

- **M0** Substrate skeleton (identity, journal, memory, the typed seam; echo proposer)
- **M1** Metabolism plumbing (Account, reserve/settle, reaper, the state machine)
- **M2** Real proposer + the bench (the first milestone where "does it compound?" is measurable)
- **M3** Learning layer (consolidation, distillation flywheel, forgetting gate)
- **M4** Self-modification (closed mutation surface + Two-Gate)
- **M5** Value source (one concrete payer + acceptance grader; makes the economic gate live)
- **M6** Research arm — *gated*: selection turns on only after the bench shows compounding **and** the
  anti-theater gate fires.

## Stack

Rust workspace (type-level closure of the mutation surface; Wasmtime sandbox; Ed25519 + hash-chain
signing; SQLite journal/memory). Local model via candle/llama.cpp; cloud via OpenAI-compatible HTTP.
See build spec §0 for the committed decisions (all revisable; each points to an architecture §15
trade).

## License

TBD.
