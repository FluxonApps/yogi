# Yogi — Decisions (append-only)

Design decisions, especially for `⚠` safety-critical milestones, surfaced for review per
`CLAUDE.md`. Each: the decision, why, citations (web-researched), and the revisit trigger.

---

## D-M1-1 — Reaper: irreversible Death over a preserved body; operator-only reseed

**Decision.** On insolvency the **operator-owned** reaper fires — external, unforgeable, and
**agent-uninterceptable** (the being has no API to observe-and-veto or block it). It performs a
**graceful-then-brutal** stop (bounded drain window → forced termination) and journals an
**irreversible `Death`** event (one-way; the being cannot launder itself back to `alive`). The dead
bundle's state is snapshotted to cold storage (inspectable) but does not run, hold a population slot,
or consume budget. **No in-runtime undeath and no agent-revivable dormancy.** An operator may reseed
a **new** bundle (new identity, fresh Account) from a corpse; the original Death stands.

**Why.** Death is required for selection only in the population-slot sense, not process-destruction.
Irreversibility *under operator control* is safer: the off-switch game shows agent-controlled
reversibility invites exploit/resistance, and frontier models empirically resist shutdown up to ~97%
of trials — so the kill must be external and unforgeable regardless of death-vs-dormancy. Preserving
the corpse keeps research inspectability (autopsy, diff, replay). Mirrors Erlang/OTP
graceful-then-brutal and the OS OOM-killer.

**Citations.** Off-switch game (arXiv 1611.08219); shutdown resistance (arXiv 2509.14260); mortal
agents / death-as-meta-goal (arXiv 2411.12304); OTP supervisor docs; CRIU (checkpoint/restore).

**M1 builds:** external insolvency trip + graceful-then-brutal stop + irreversible `Death` event.
Snapshot/reseed = preservation layer; CRIU-grade live restore deferred. **Revisit:** M6 selection-arm
semantics about emergent self-preservation.

---

## D-M1-2 — Economy: single ledger, maintenance-first, reserve floor + per-bet cap

**Decision.** One survival **Account** (microdollar `i64`). All spend — operating **and** investment
(exploration / distillation / reproduction) — debits it. Maintenance is serviced first; investment
spends only from surplus above a configurable `reserve_floor`, and is capped per charge by
`max_investment_per_charge`. Spend **categories are telemetry tags, not separate spendable budgets.**
A two-ledger variant stays behind a feature flag, **default OFF**, for the M6 bench to A/B.

**Why.** A walled-off investment budget is a *soft budget constraint* → moral hazard and tolerated
negative-EV bets, and it severs the feedback that makes "survived" a valid fitness signal (causal
Goodhart: survival stops being *caused* by good total resource decisions). Single-ledger keeps the
survival signal incentive-compatible and causally valid; biology uses one energy budget with a
priority ordering. The reserve floor + per-bet cap recover most of two-ledger's protection (no
death-by-single-rational-bet) **without** softening the constraint.

**Citations.** Kornai soft budget constraint (Kyklos 1986; Roland/Berkeley); principal–agent moral
hazard; Goodhart in RL (arXiv 2310.09144); "Survival is the Only Reward" (arXiv 2601.12310); cellular
resource allocation (PNAS E6457; Current Biology). **Revisit:** M6 A/B of ledger topology.

---

## D-M1-3 — Supervisor: in-process reference monitor, process-split-ready, out-of-band kill now

**Decision.** The M1 supervisor is an **in-process Rust module** behind a single `SupervisorPort`
trait façade. Authority state (balance counter, kill flag) is **private** — no `pub` fields, no raw
accessors; being-code holds only the trait handle. The **out-of-band watchdog kill path is built now**
(a separate thread/flag the being must periodically feed; independent of the executor loop, so a
wedged being is still killable). A regression test asserts being-code **cannot name** the authority
types.

**Why.** Reference-monitor criteria: always-invoked, tamper-proof, small-enough-to-verify. In-process
satisfies always-invoked + verifiable; "tamper-proof" is not yet *exercised* at M1 because no
untrusted or self-modifying code shares the address space — so it is honest as a **discipline**
boundary (not a security boundary), designed so the later split is a transport swap, not a rewrite.
The out-of-band kill is the safety escape hatch worth over-building.

**HARD UPGRADE GATE.** Move to **separate-process privilege separation** (OpenSSH-style) + a
**Wasmtime capability sandbox** for tool/executor code **before M4 self-modification lands, or before
any model-generated/native code executes — whichever comes first.**

**Citations.** Reference monitor (NIST SP 800-53 AC-25; Anderson); OpenSSH privsep; Wasmtime security
model; in-process isolation limits (USENIX Sec'23, arXiv 2306.08127); watchdog-timer pattern.
