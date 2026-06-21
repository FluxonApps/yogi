# Yogi — build rules (local, frictionless)

Yogi is built **entirely locally**. No CI, no cloud services, no API keys.

## Resources (the only two)
- **Ollama** (`http://localhost:11434`, OpenAI-compatible) — the runtime's local proposer and the
  shared embedding. Installed: `qwen3:8b` (proposer / per-domain distillation base),
  `nomic-embed-text` (the named shared embedding for memory retrieval + navigators).
- **Claude Code agent** — frontier intelligence: it builds the project and is the spec's
  frontier/teacher tier. No cloud LLM API is used.

This **supersedes the spec's cloud assumptions for the local build** (architecture/build-spec D2/D5):
local proposer = Ollama; frontier/teacher = Claude Code; no OpenAI-compatible cloud endpoint.

## Hardware & inference budget (M5 MacBook Pro · 16 GB unified memory)
- **One model at a time.** `qwen3:8b` ≈ 5–6 GB resident when loaded; never run two large models
  concurrently. `nomic-embed-text` ≈ 0.3 GB.
- **HARD RULE — no model inference in the automated loop.** `cargo test --all`, `cargo clippy`, and
  the `Stop`/`PostToolUse` hooks MUST NEVER call Ollama. Live model calls are **foreground and
  user-initiated only** (e.g. `cargo test -p being-proposer-ollama --features live-ollama`, or a
  `cargo run` demo). Default `cargo test --all` excludes the `live-ollama` feature, so the loop never
  loads the model.
- **Never launch inference (or any heavy job) in the background.**
- Distillation (M3) must pick a student small enough to train within this budget — decide at M3.

## Golden rule
**Never claim a task or milestone "done" while the build is red.** The `Stop` hook runs
`cargo test --all` + `cargo clippy --all-targets -- -D warnings` and blocks completion on failure.
Show the actual test/clippy output as evidence; do not assert success.

## Build order (every change)
1. `cargo check` — fast feedback (also auto-run on each `.rs` edit by the `PostToolUse` hook).
2. `cargo test --all` — all tests green.
3. `cargo clippy --all-targets -- -D warnings` — zero warnings.
4. `cargo fmt --all` — formatted.
5. Commit only when 1–4 pass. End commit messages with the session trailer.
   - Run git from the repo root — **never** `cd … && git …` (it trips the "cd before git can run
     untrusted hooks" prompt; the session cwd is already the repo root).
   - Use **plain `git commit`** — never `-c user.email/-c user.name` overrides; the ambient gitconfig
     identity (`Ganesh Gunasegaran <me@itsgg.com>`) is correct.

## Continue / what's next
- **Milestone state is tracked in `docs/MILESTONES.md`** (the durable source of truth). To "continue",
  implement the **first unchecked item** there, keep the build green, then tick it and commit.
- Drive one milestone per `/goal`; review the diff at `⚠` safety-critical and `🔒` gated milestones.

## Milestone discipline (build-spec §6)
- Build milestone-by-milestone (M0 → M6); each ends in a runnable artifact + a passing acceptance test.
- **Selection (M6) stays OFF** until the bench shows compounding AND the anti-theater gate fires.
- Safety-critical crates (`being-supervisor`, sandbox, egress proxy): surface design choices for
  human review rather than deciding silently.

## Invariants that must never regress (encode as tests)
- **Closed mutation surface:** `MutationKind` is not `#[non_exhaustive]` and `apply` matches
  exhaustively with no wildcard — a forbidden variant (capability grant, trust-policy edit, kernel,
  budget rules, reaper) must be a *compile error*. Never add a `_ =>` arm to `apply`.
- **No-launder provenance:** nothing may be relabelled to `DirectUserIntent`.
- **Microdollar budget** is `i64` with `overflow-checks` on (workspace release profile).

## Layout
- `crates/` — the Rust workspace (full crate DAG in build-spec §2).
- `docs/architecture.md` — canonical spec · `docs/build-spec.md` — M0–M6 · `docs/architecture-annotated.md` — internal.
- `.claude/settings.json` + `.claude/scripts/` — the local build harness (verify + check hooks).

## Don'ts
- No `cargo clean` (slow; use `cargo check`).
- No `#[ignore]` / `#[allow(...)]` to turn red green — fix the root cause.
- No network/cloud calls in the runtime hot path beyond local Ollama.
