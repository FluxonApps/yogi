# Frontier Being — Build Specification (v0)

> The implementable companion to the architecture spec. The architecture spec answers *what and
> why* (and is rigorously honest that its economic/evolution claims are **suspended behind
> falsification gates**). This document answers *how, in what order, and how you know it works* —
> scoped to the part that is buildable today regardless of those open questions.
>
> All decisions here are **committed for the v0 build but revisable**; each points to the
> architecture spec's §15 trade-space where an alternative is live. Nothing is closed.
>
> Rationale, open-research debate, and the trade-space live in `/Users/gg/Work/Yogi/frontier-spec.md`
> (cited as *arch §N*). This document is self-contained for implementation: every type, algorithm,
> formula, and acceptance criterion needed to build v0 is defined here.

---

## The one structural move

The architecture spec splits cleanly along a line the build must respect:

- **Buildable now (the substrate + plumbing):** identity, the signed journal, memory, the
  perception → propose → commit → attest → execute seam, the microdollar Account with an
  operator-owned reaper, and the **falsification bench**. The distillation flywheel sits on top.
- **Instrumented, not built (the research arm):** parallel self-modification *with selection* and
  the economic/Darwinian claims. These are **experiments the substrate makes measurable**, gated
  behind the bench — not features to implement in v0.

So: build the substrate, the metabolism plumbing, the learning layer, and the bench. Treat
evolution and economic fitness as instrumentation the bench reads. The master success criterion is
the **anti-theater gate** (§9): does the harness do causal work, or is it an accounting wrapper?

---

## 0. Committed build decisions

| # | Decision | Committed choice (v0) | Why | Revisable → trade |
|---|---|---|---|---|
| D1 | Stack | **Rust workspace** | Type-level closure is real (a 7th `MutationKind` variant is a *compile error*, not a runtime check); Wasmtime sandbox; Ed25519 + hash-chain signing; SQLite journal/memory map ~1:1 to the spec's invariants | arch §15 (genome representation) |
| D2 | Proposer runtime | Local via **candle/llama.cpp**; cloud via **OpenAI-compatible HTTP**; both behind one `Proposer` trait | model-agnostic; the being survives a proposer swap | arch §2 (heritable proposer) |
| D3 | First target | **One being that compounds, measurably** (ontogeny + bench). Population/selection deferred to M6, gated | smallest thing that passes the removal test; selection is gated on the bench anyway | arch §14 |
| D4 | Value source | **Operator-as-customer with a concrete tariff/grader (the (a2) efficiency-only variant first)**, with the **exogenous-payer hook** built so a real payer drops in | runnable today; honest that economic/value-capture claims stay labeled *efficiency-only* until an exogenous payer exists (arch §5.2) | arch §5.2 option (i) |
| D5 | Distillation | **Per-domain LoRA on a frozen base** | isolates adapters; bounds forgetting to a measurable event | arch §15 (per-user weights) |
| D6 | Concurrency | **Serial lane** (single in-flight effect lane) for v0 | the spec's distributed invariants are proven only for the serial lean; concurrency is arch §13.3 future work | arch §4.7/§15 |

---

## 1. Build scope

**In scope (v0, M0–M5):** identity + journal; memory (episodic/semantic/skills — in v0 "procedural
memory" **is** the existing `skills` table + `Genome`; there is no separate procedural store); the
propose/commit/attest/execute seam; the Account + supervisor reserve/settle; the reaper; per-domain
distillation; navigator + routing; the closed mutation surface + Two-Gate self-modification; the
falsification bench; one concrete payer + acceptance grader.

**Instrumented, not built (M6, gated):** fork/population, fitness-driven selection, lineage
dynamics. The *mechanics* (fork saga, lineage records) are built as substrate; **selection is not
turned on** until the bench shows compounding and the anti-theater gate fires.

**Explicitly deferred (with a re-open trigger):** exogenous payer (trigger: a real marketplace
exists); richer memory (QD archive, foresight, multi-graph — trigger: flat store loses on the
bench); meta-modification (trigger: base compounding proven); mesh federation (trigger: scale/threat
model demands); **crossover** (out of scope for v0 — mutation-only per arch §15; opaque genome
`Bytes` therefore need no crossover parser). Each is an architecture §15 row.

---

## 2. Module decomposition (crate DAG)

```
                 being-core-types ──┐  (lowest node: shared value types + canonical codec + crypto value-types + skill value-types)
                 being-core-id ─────┤  (depends on -types; DECLARES Signer/verify only, re-exports crypto value-types)
                 being-core-journal ┤  (depends ONLY on -types and -id; never on -runtime or -mutation)
   being-core-memory ───────────────┤
   being-core-economy ──────────────┤
   being-core-policy ───────────────┤  (trust/capability)
   being-core-mutation ─────────────┘  (Genome, MutationKind, Two-Gate)
            │
            ▼
      being-runtime ─────────────────────────────┐  perception, Proposer trait,
            │  (composes the core crates into a turn)  committer/seam, executor state machine,
            ▼                                          control loop, AccountAuthority client stub
      being-supervisor ◀── operator-owned: reserve/settle authority, reaper, spend
            │                classification, out-of-band kill, fork saga, egress/payment proxy,
            ▼                AccountAuthority/Supervisor server handler, FallbackJudge endpoint
      being-proposer-{echo,openai,local}   (impl the Proposer trait)
      being-distill                        (gap detect, teacher traces, LoRA, promotion gate)
      being-bench                          (frozen+rolling bench, bootstrap CI, anti-theater arms)
      being-bin                            (the binary: run / serve / bench / fork / status)
```

**Dependency rule:** `being-core-types` is the **shared-type root**: it owns all cross-cutting value
types (`ProvenanceClass`, `EffectClass`, `ModelRef`, `Domain`, `IdemKey`, `Percept`, `StepArgs`,
`SecondPartyConfirmation`, `ReserveVerdict`, `DedupToken`, `EpisodicId`, `SemanticId`) and the one
canonical codec (`canon`, the skill value-types `SkillId/SignedSkill/Manifest` + `skill_id()`, and
the crypto value-types `Did/Sig/Hash`); `being-core-id` sits directly above it and DECLARES only
`Signer`/`verify`, re-exporting the crypto value-types from `-types` (§3.0). Every other core crate
depends only on `-types` and `-id` for shared types — never on each other. **`being-core-journal`
depends only on `-types` and `-id`** (importing `Did/Hash/canon` from `-types` and `Signer` from
`-id`; its `Commitment`/`Attestation`/`GenomeMutation` payloads are opaque blobs, §3.3).
**`being-core-mutation`** consumes `SkillId/SignedSkill/Manifest/skill_id` from `-types` (not from
`being-core-memory`), preserving the no-cross-core-dep rule. The **supervisor is a
separate trust domain from the runtime** — the being links the runtime; the operator runs the
supervisor; they communicate by IPC (see §3 IPC transport). That separation is a safety property, not
a packaging convenience (D4, arch §5.4).

---

## 3. Interface contracts

Concrete signatures the crates implement against. (Rust-flavored; `Result`/error types elided unless
load-bearing.)

### 3.0 Canonical shared types (`being-core-types`, re-exported to all crates)

One definition, owned at the root, re-exported everywhere. This block resolves every undefined
`Did/Sig/Hash/Seq/Microdollars` reference across the document.

```rust
type Did         = [u8; 32];   // raw Ed25519 public key. did:key text is display-only, never stored.
type Sig         = [u8; 64];   // Ed25519 signature, ed25519-dalek v2.
type Hash        = [u8; 32];   // BLAKE3-256, the `blake3` crate.
type Seq         = u64;
type Microdollars = u64;
type EpisodicId  = u64;        // crate-assigned monotone rowid; fixed-width LE in canon().
type SemanticId  = u64;        // crate-assigned monotone rowid.
type Bytes       = Vec<u8>;    // canon-encoded as a length-prefixed byte run (see encoding pin below).
```

**Canon encoding pin (normative; closes the array-vs-Vec preimage ambiguity).** Under `canon`,
`Bytes`/`Vec<_>`/`String` encode as `u64 LE length-prefix || raw bytes`; fixed-size arrays
(`Did = [u8; 32]`, `Sig = [u8; 64]`, `Hash = [u8; 32]`) encode as exactly N raw bytes with **NO**
length prefix. This single rule fixes the byte layout of every `Bytes` field (e.g. the `entry_hash`
preimage, `Percept.body`, `SemanticFact`, IPC payloads) and the array-vs-Vec distinction in the
hash-chain preimage.

**`being-core-types` ownership (authoritative type list).** `being-core-types` DECLARES the value
types `Did, Sig, Hash, Seq, Microdollars, Bytes, EpisodicId, SemanticId, SkillId, SignedSkill,
Manifest` and the closed cross-cutting enums/structs of §3.0, plus the functions `canon`,
`paired_bootstrap_p`, and `skill_id()`. `being-core-id` DECLARES only `Signer`/`verify` and
**re-exports** the crypto value-types (`Did/Sig/Hash`) from `-types`. Consequently:
`being-core-journal` imports `Did/Hash/canon` from `-types` and `Signer` from `-id`;
`being-core-mutation` consumes `SkillId/SignedSkill/Manifest/skill_id` from `-types` (**not** from
`being-core-memory`, preserving the no-cross-core-dep rule in §2).

`Signer::sign`/`verify` use **ed25519-dalek v2**. All chain/content hashing uses **blake3**.

**Canonical deterministic codec (the single committed encoder).** Every signed or hashed payload is
encoded with exactly one function, used identically on write and on replay:

```rust
fn canon<T: Encode>(v: &T) -> Vec<u8>;   // bincode v2, fixed little-endian, fixed-int encoding,
                                         // explicit enum-variant tags in declaration order, no compression.
// EXACT configuration (quoted once so no implementer falls back to varint standard()):
//   canon = bincode::encode_to_vec(v, config::standard().with_little_endian().with_fixed_int_encoding())
//   (the u32 LE variant tag under this config is the "Codec determinism pins" pin below).
```

**Shared paired-bootstrap primitive (owned here so `-mutation` and `-bench` call ONE engine).**

```rust
// item-aligned paired bootstrap p-value, one-sided H1: mean(candidate - base) > 0.
// B resamples of item-pairs with replacement; p = (count of resampled paired-mean-diffs <= 0) / B
// (ties at exactly 0 counted in the <= 0 bucket). RNG is seeded for determinism.
fn paired_bootstrap_p(base: &[f64], candidate: &[f64], B: usize, seed: u64) -> f64;
```

Both the §3.7 Validation Gate and the §3.12 `paired_bootstrap_ci` call `paired_bootstrap_p`. Callers
pass `B = BENCH_B` (§7) and a **fixed journal-derived seed** (the low 64 bits of `head().1` at gate
time) so `p` is deterministic on replay. **"significant"** = `p < VALIDATION_FDR_Q` (§7).

`canon` is total and deterministic. **Codec determinism pins (replay-critical):**
1. **Enum discriminants are encoded as a fixed `u32` LE** (bincode v2 fixed-int encoding applies to
   the variant tag too), **not** a varint — so `JournalEvent`/`MutationKind`/`StepArgs` encodings
   stay byte-stable across enum evolution.
2. Every fixed-width integer (including `EpisodicId`/`SemanticId` where they appear in a signing
   payload, e.g. `EpisodicRow.supersedes`) is LE fixed-width.

**Derived `Eq` on any type encoded by `canon` is the canonical equality**: two values are equal iff
they re-encode to byte-identical `Vec<u8>`. This is the equivalence relation used by all
replay-determinism checks. `JournalEvent`, every memory row, and `Commitment` are encoded only via
`canon`.

**Cross-cutting value types** (closed enums; owned here):

```rust
// Intent provenance — see §3.1. Closed; a 6th variant is a compile error.
enum ProvenanceClass { DirectUserIntent, ModelInference, ToolOutput, FetchedDoc, PeerFederated }

// Effect taxonomy — mirrors arch §4.6 so capability checks are exhaustive. Closed.
enum EffectClass { MemoryRead, MemoryWrite, Query, Infer, Notify, Render, Http, Sign, Payment }

struct ModelRef {
    kind: ModelKind,            // Base | Adapter
    base_hash: Hash,            // for Adapter: the frozen base it attaches to
    weights_hash: Hash,
    provenance_class: ProvenanceClass,
    trained_on_untrusted: bool, // OR of teacher-trace taint (see §3.1)
}
enum ModelKind { Base, Adapter }

type Domain    = String;        // operator-curated task-class key
type ChannelId = String;        // perception ingress channel identifier

struct IdemKey {                // see §3.6 / §4 ledger
    commitment_hash: Hash,      // blake3(canon(Commitment)) EXCLUDING outer Sig, INCLUDING journaled nonce
    step_index: u32,
}

// Perception unit — ONE canonical definition serving being-core-memory and being-core-policy/runtime.
struct Percept {
    channel: ChannelId,
    provenance_class: ProvenanceClass,
    trust: f64,
    body: Bytes,
    ts: u64,                    // wall-clock micros
}

// Closed plan-step argument enum, keyed 1:1 to EffectClass so arg validation is exhaustive
// and canon-encodable. (MemoryRead/MemoryWrite use EpisodicRow/SemanticRow from §3.8.)
enum StepArgs {
    MemoryRead { query: Bytes },
    MemoryWrite { row: EpisodicRow },
    Query(Bytes),
    Infer(Bytes),
    Notify { body: Bytes },
    Render { body: Bytes },
    Http { method: String, url: String, body_hash: Hash },
    Sign { over: Hash },
    Payment { amount: Microdollars, payee: String },
}

// Out-of-band human confirmation gating provenance upgrades and DirectUserIntent (§3.1).
struct SecondPartyConfirmation {
    confirmer_did: Did,
    over: Hash,                 // = blake3(canon(row || target_class))
    sig: Sig,
    expiry: u64,                // wall-clock micros
}

// Return of reserve() (§3.6).
enum ReserveVerdict {
    Granted { lease_id: Hash, expires_wall_ts: u64, reserved: Microdollars },  // reserved = sum over the
        // granted (survivor) rows of THIS reserve call; recomputed on a survivor re-reserve (§3.6).
    Exceeded,
    Refused,
}

// At-most-once egress token (§3.10).
struct DedupToken { idem_key: IdemKey, nonce: Hash, expires_wall_ts: u64 }
// nonce = blake3(canon(idem_key) || expires_wall_ts) — ADVISORY ONLY (dedup keys SOLELY on
//   canon(IdemKey); the nonce never participates in the dedup-ledger lookup).
// expires_wall_ts = now + EGRESS_TOKEN_TTL_MS (§7; v0 reuses lease_ttl_ms).

// Canonical turn-output unit — ONE definition, owned here, consumed by being-bench
// (grade/attempt) and being-distill (gap_closure / forgetting scoring). canon-encodable.
struct TurnOutput {
    turn_id: Hash,                  // = commitment_hash of the turn that produced it
    answer: Bytes,                  // the graded artifact: AcceptanceGrader compares this vs BenchItem.reference
    steps: Vec<PlanStep>,           // committed steps (§3.5)
    attestations: Vec<Attestation>, // per-step attestations (§3.4)
}
```

**Single inference entrypoint.** Both `being-bench` and `being-distill` obtain a `TurnOutput` through
**one** codepath — `Being::attempt` (§3.12) — which runs a full turn and returns the produced
`TurnOutput`. The router inside the turn selects base vs candidate adapter from the genome
(`Router::route`, §3.11) and `build_context_pack` stamps that selection into `ContextPack.model`
(§3.5), the **single wire** every inference step reads, so there is **no second inference path**:

```rust
fn run_turn(model: &ModelRef, item: &BenchItem) -> TurnOutput;  // thin wrapper: builds a Percept from
// item.prompt, runs Being::attempt with the router pinned to `model` and domain = item.domain (§3.12 BenchItem),
// returns the TurnOutput.
```

`AcceptanceGrader::grade` (§3.12) compares `TurnOutput.answer` against `BenchItem.reference`.
**Teacher arm:** `teacher_scores` (used by `gap_closure`, §3.11) are produced by grading the **stored
`TeacherTrace` output replayed as a `TurnOutput` through the same `AcceptanceGrader` codepath** — the
teacher's approved output becomes `TurnOutput.answer`, graded identically to student outputs.

**`Percept` signing + channel gating.** The per-row signing payload of any memory row carrying a
`Percept` covers `channel`, `provenance_class`, `trust`, `body`, and `ts` (all five fields are inside
the `canon`'d `payload`, §3.8). The **confirmed-human-channel flag** that gates `DirectUserIntent` in
the §3.1 truth table is **derived**, not stored: a channel is "confirmed human" iff `percept.channel`
is a member of an **operator-configured confirmed-channel set** (supervisor config, not being-asserted).

### 3.1 Provenance, no-launder, and the writer truth table (`being-core-types` + `being-core-memory`)

`trained_on_untrusted: bool` is an **orthogonal heritable taint flag**, not a 6th `ProvenanceClass`
variant (arch §5.3). It rides alongside the intent class and is never an intent class itself.

**Writer truth table** — which role may *stamp* which class:

```rust
enum WriterRole { PerceptionGateway, Consolidator, Executor }
fn can_write(writer: WriterRole, class: ProvenanceClass) -> bool;
```

| WriterRole | DirectUserIntent | ModelInference | ToolOutput | FetchedDoc | PeerFederated |
|---|---|---|---|---|---|
| PerceptionGateway | only from a confirmed human channel | yes | yes | yes | yes |
| Consolidator | no | **yes (only this)** | no | no | no |
| Executor | no | no | no | no | no |

Nothing may write `DirectUserIntent` except the gateway from a confirmed human channel ("confirmed" =
`Percept.channel` ∈ the operator-configured confirmed-channel set, §3.0).

**Runtime no-launder enforcement (not type-level in v0).** Episodic/semantic writes require a
non-forgeable `ProvenanceToken`:

```rust
// carries the maximum class it authorizes. Its constructor is CRATE-PRIVATE to being-core-memory:
// there is NO public path that constructs a ProvenanceToken.
struct ProvenanceToken { allowed: ProvenanceClass, minted_by: WriterRole }

// the ONLY two minting fns exposed, gated to specific modules:
fn mint_gateway_token(allowed: ProvenanceClass) -> ProvenanceToken;     // perception-gateway only
fn mint_consolidator_token() -> ProvenanceToken;  // Consolidator only; `allowed` FIXED to ModelInference

fn append_episodic(&mut self, row: EpisodicRow, authority: &ProvenanceToken) -> EpisodicId;
```

**Unforgeability mechanism.** Because `ProvenanceToken`'s constructor is crate-private and reachable
only via `mint_gateway_token` / `mint_consolidator_token`, and `mint_consolidator_token` hard-fixes
`allowed = ModelInference`, **no public path constructs an arbitrary token**. The M0 property test's
notion of "non-forgeable" is exactly this: *no public code path can construct a `ProvenanceToken`*.

The guard **rejects any write whose declared class exceeds the token's authority**. Legal upgrades go
only through:

```rust
fn attested_upgrade(from: ProvenanceClass, to: ProvenanceClass,
                    confirmation: SecondPartyConfirmation) -> Result<ProvenanceClass, UpgradeError>;
```

**`attested_upgrade` verify predicate (all four must hold, else `Err`):**
1. `verify(confirmation.confirmer_did, confirmation.over, confirmation.sig) == true`;
2. `confirmation.confirmer_did ∈` the operator-configured **confirmer set**;
3. `confirmation.over == blake3(canon(row || target_class))` (binds the confirmation to this exact row
   and target);
4. `now <= confirmation.expiry`.

**Flat-out impossible transitions:** `anything → DirectUserIntent` without a valid
`SecondPartyConfirmation` satisfying the predicate above; any Consolidator-originated row to a class
other than `ModelInference`; any Executor-originated provenance stamp. This makes the M0 no-launder
property test objectively checkable (§8).

### 3.2 Identity (`being-core-id`)

```rust
trait Signer { fn did(&self) -> Did; fn sign(&self, bytes: &[u8]) -> Sig; }
fn verify(did: &Did, bytes: &[u8], sig: &Sig) -> bool;
```

**`verify` is total and never panics.** `verify` uses **`VerifyingKey::verify_strict`** (ed25519-dalek
v2, **NOT** the permissive `verify`). Any error building the `VerifyingKey` from the `Did` **OR** the
`Signature` from the 64 bytes, **OR** a `verify_strict` failure (including non-canonical R/S and
cofactored/malleable signatures) returns **`false`** (not a panic, not an `Err`). This makes two
implementers recover the **same chain head** from the same on-disk journal and the malformed-`Did`
replay case deterministic.

### 3.3 Journal (`being-core-journal`) — single-writer per DID, one chain head (arch Appendix A)

```rust
trait Journal {
    fn open(path: &Path, did: Did, signer: Arc<dyn Signer>) -> Result<Self>; // construction contract (below)
    fn append(&mut self, ev: JournalEvent, wall_ts: u64) -> Seq;   // see hash-chain + durability below
    fn head(&self) -> (Seq, Hash);                   // (max_seq, entry_hash[max_seq]); empty = (0, blake3(did))
    fn replay_from(&self, seq: Seq) -> impl Iterator<Item = ReplayedEvent>; // .seq >= seq, ascending, inclusive
    // v0 replay_from MAY materialize the replayed slice into an owned iterator (collect into Vec) — this
    // unblocks returning `impl Iterator` from `&self` over rusqlite without a self-referential wrapper,
    // consistent with the already-accepted O(N) recovery scan.
    fn next_commitment_nonce(&self) -> u64;          // count of Commitment events at current head (see below)
}
struct ReplayedEvent { seq: Seq, event: JournalEvent }  // .seq is readable so M0's predicate is checkable

enum JournalEvent {
    Commitment(Bytes),                          // opaque pre-canon'd blob (being-runtime Commitment)
    ExecMarker { seq: Seq },
    Attestation(Bytes),                         // opaque pre-canon'd blob (being-runtime Attestation)
    Settle { seq: Seq },
    BatchRelease { batch_id: Hash, released: Vec<u32> },
    ForkBarrier { child: Did, snapshot_offset: Seq },
    GenomeMutation(Bytes),                       // opaque already-canon'd GenomeVersion blob (see §3.7)
    Death { cause: DeathCause },
}
enum DeathCause { Insolvency, ReaperKill, Operator, Fault }
```

**`JournalEvent` canon contract (single byte layout, replay-critical).** `JournalEvent` and **all
nested types** (`DeathCause`, `BatchRelease`, `ForkBarrier`, the `ExecMarker`/`Settle` payloads, etc.)
`#[derive(bincode::Encode)]` **AND `#[derive(bincode::Decode)]`** and are encoded **only** through the
single committed `canon` config (§3.0). `replay_from` **decodes the stored `payload` with the SAME
`canon` Config** (`config::standard().with_little_endian().with_fixed_int_encoding()`, §3.0), and the
**round-trip property `canon(decode(canon(e))) == canon(e)` is REQUIRED** so replay does not perturb the
chain. Combined with the §3.0 array/Vec encoding pin, this fixes the `entry_hash` preimage
`blake3(prev_hash || did || seq_le || canon(event))` to **one** byte layout, so `head()` and the M0
`prev_hash` determinism oracle cannot diverge across implementers.

**Journal construction + single-writer-per-DID.** `open(path, did, signer)` constructs the journal: it
**holds the `Signer`** (used to sign every `entry_hash`, below) and the `did` (seeds `prev_hash[0]` and
is the verifying key for recovery `verify`). **`open()` returns `Err` if `signer.did() != did`** (a
mismatch makes every appended row sign under the wrong key, so recovery would `verify` against `did` and
truncate the entire journal — this guard prevents that footgun). Single-writer is enforced physically: **one SQLite file
per `did`, held under an exclusive advisory `flock(LOCK_EX | LOCK_NB)` on the DB file for the journal's
lifetime, released on `Drop`**. The advisory `flock(LOCK_EX | LOCK_NB)` is held on a **dedicated fd
opened separately from the rusqlite `Connection`**, kept alive for the journal's lifetime and
**released explicitly on `Drop` AFTER the `Connection` is closed** — so single-writer enforcement is
independent of SQLite's internal WAL/POSIX locking. A **second `open` of the same file errors `Err` if
the lock is held by a live process**; a **stale lock from a crashed holder is automatically
reclaimable** (the OS releases `flock` on process death, so the next `open` acquires it). **`open()` order:**
`(1)` acquire the `flock`; `(2)` create the DB file if absent, run `PRAGMA journal_mode=WAL`, then a
**no-op write txn (equivalently `PRAGMA wal_checkpoint(PASSIVE)`) to MATERIALIZE the `-wal`/`-shm`
sidecar files** (which do not exist at `journal_mode=WAL` time — they appear on first write); `(3)` fsync
the directory **once, on creation only**, AFTER the `-wal`/`-shm` files are materialized (§ durability
below); `(4)` run the recovery scan (below); `(5)` ready — the first `append` returns only after this
sequence. This pins **what is fsynced and in what order** (the DB file + materialized `-wal`/`-shm`,
then one directory fsync).

**Seq convention.** The **first appended row has `seq = 1`**; `seq` increments by 1 per `append`.
**`seq = 0` is the reserved empty-journal sentinel** — it never appears in any `entry_hash` preimage
and is the `seq` component of the empty `head()` `(0, blake3(did))`. `prev_hash` for `seq = 1` is
`blake3(did)` (= `prev_hash[0]`). The `next_commitment_nonce` rule (k-th `Commitment` nonce = `k-1`) is
consistent with this 1-based convention.

`Commitment`, `Attestation`, **and `GenomeMutation`** are stored as **opaque, already-`canon`'d
blobs**. `GenomeMutation(Bytes)` carries an already-`canon`'d `GenomeVersion` (defined in §3.7), which
in turn embeds `MutationKind` (defined in `being-core-mutation`). The opaque-blob treatment is what
keeps the dependency clean: **so `being-core-journal` does not depend on `being-runtime` OR
`being-core-mutation`** (the same rationale already applied to `Commitment`/`Attestation`).

**`wall_ts` provenance.** `append`'s `wall_ts: u64` parameter is **caller-supplied** (the caller owns
the clock, matching the deterministic-replay posture — the journal never reads a wall clock itself). It
is stored in the `journal.wall_ts` column as a **stored-not-signed annotation**: it is **excluded from
every determinism oracle** and is **not** in any `entry_hash`/`sig` preimage (the preimage below does
not reference it). It is the source of truth the runtime reads for the Seq → wall_ts mapping (§3.8).

**Hash chain (exact preimage).**

```
prev_hash[0]    = blake3(did)
entry_hash[n]   = blake3( prev_hash[n-1] || did || seq.to_le_bytes() || canon(event) )
sig[n]          = Signer.sign(entry_hash[n])     // the signature commits to the chain
```

Row schema stores `payload = canon(event)`, `prev_hash = entry_hash[n-1]`, `sig`. **`entry_hash[n]` is
NOT persisted** (there is no `entry_hash` column): only `prev_hash`, `payload`, and `sig` are stored.
`head().1` and recovery **recompute** `entry_hash[max_seq]` from the stored fields via the preimage
above. (Removing the ambiguity here is what keeps two implementers from silently disagreeing on
whether to add an `entry_hash` column — the determinism oracle depends on this.) `head()` returns
`(max_seq, entry_hash[max_seq])`; empty-journal head is `(0, blake3(did))`.

**Durability + recovery contract.**
1. SQLite `journal_mode=WAL`, `synchronous=FULL` ⇒ SQLite fsyncs the WAL frame on COMMIT; the crate
   relies on this and issues **no extra `fdatasync`**. `append` performs **one INSERT per
   transaction** and returns only after that COMMIT.
2. The DB **directory is fsynced once, on file creation only** — **after** the DB file is created AND its
   `-wal`/`-shm` sidecar files are **materialized** by the `open()`-step-(2) no-op write txn /
   `wal_checkpoint(PASSIVE)` (they do not exist at `journal_mode=WAL` time) — and **before the first
   `append` returns**.
3. v0 checkpoint policy = **PASSIVE checkpoint every `WAL_CHECKPOINT_N` appends** (§7) to bound WAL
   growth and recovery-scan cost.

One row = one atomic commit, so a torn write cannot leave a partial row. **Recovery on open:** scan
**ascending** and STOP at the first `seq` that fails link-or-sig validation (`entry_hash` link or
`sig`); the head is the **last fully-valid contiguous `seq`**. **Per-row validation truth-table** (a
row is valid iff BOTH hold): **(a) link:** the stored `prev_hash` `==` the recomputed `entry_hash[n-1]`
(the recomputed `entry_hash` of the immediately preceding surviving row; for `seq = 1`,
`prev_hash == blake3(did)`); **(b) sig:** `verify(did, recomputed entry_hash[n], stored sig) == true`
using the journal's own `did`. The **first row failing EITHER check is the first-failure `seq`**;
truncate from there. **ALL rows at and above the first
failure are TRUNCATED** — even if some validate in isolation — because the chain is broken. Truncation
**physically `DELETE`s all rows with `seq >= first-failure seq` in one transaction** so the
post-recovery `seq` space is **contiguous and dense**; `head()` and `next_commitment_nonce` are then
computed over the surviving prefix. This is **truncation, not error**, in v0 (required for the
single-head-per-DID property test to be objectively checkable). **Recovery-scan cost:** v0 accepts the
recovery scan as a **full ascending scan, O(N)** in journal length (bounded in practice by the
`WAL_CHECKPOINT_N` policy); a persisted last-verified-head marker with re-verify-from-marker is a
post-v0 optimization row. Re-append after a crash is idempotent
via the `commitment_hash` idempotency key (§3.6).

**`event_kind` storage + variant-order pin (DB portability + nonce determinism).** The `journal.event_kind`
column stores the **`u32` LE enum discriminant of `JournalEvent` exactly as emitted by `canon`**
(stored as an `INTEGER`). The variant order is **pinned** so a future reorder cannot silently change
persisted tags: `Commitment = 0`, `ExecMarker = 1`, `Attestation = 2`, `Settle = 3`,
`BatchRelease = 4`, `ForkBarrier = 5`, `GenomeMutation = 6`, `Death = 7`. A reorder is a spec change,
not a silent data migration.

**Idempotency nonce (`next_commitment_nonce`).** A per-DID `u64` equal to the **count of
`Commitment`-kind events at the current head**, computed as
`COUNT(*) WHERE event_kind = 0 AND seq <= head_seq` (so the nonce of the k-th `Commitment` is `k-1`,
taken *before* the in-flight append). `append()` does **not** mutate the nonce for non-`Commitment` events;
recovery recomputes it by counting `Commitment`-kind rows up to head. This is the **authoritative
value `being-runtime` stamps into `Commitment.nonce`** before building the opaque `Commitment(Bytes)`
blob. It feeds `commitment_hash` as a fixed-width LE field. "Per-iteration" nonce **is** this per-DID
monotone counter (no per-turn reset); it also relates to `GenomeVersion.version` only insofar as both
are journal-derived monotone counters.

### 3.4 Runtime seam (`being-runtime`) — arch §4.3

```rust
trait Proposer { fn propose(&mut self, ctx: &ProposerContext) -> Proposal; }  // generative; never commits.
// ProposerContext (§3.5) is the ContextPack subset projected for the proposer; budget_replica +
// trust_snapshot are withheld unless ConditionFlags.expose_budget_trust == true (§3.12). commit()
// continues to take the FULL &ContextPack (it ALWAYS sees trust_snapshot + budget_replica).
struct Proposal {
    intent: String, candidate_plans: Vec<Plan>, preferred: usize, est_cost: Microdollars,
    advisory_EV: f64, advisory_risk: f64,    // pinned-at-commit advisory inputs (see objective_score)
}

trait Committer {  // deterministic over: policy, trust thresholds, schema/arg validation, budget reserve
    fn commit(&mut self, p: &Proposal, ctx: &ContextPack) -> Commitment;
}
struct Commitment {
    committed_steps: Vec<PlanStep>, rejected: Vec<(PlanStep, String)>,
    needs_human: bool, continue_loop: bool, budget_verdict: BudgetVerdict,
    objective_score: f64, turn_id: Hash, nonce: u64, signature: Sig,
    fallback_verdict: Option<JudgeVerdict>,   // journaled when the fallback judge was consulted (see below)
}
enum BudgetVerdict { WithinBudget, Exceeded, Refused }

// the executor IS the per-step state machine of §5 below
trait Executor { fn run_step(&mut self, step: &PlanStep, c: &Commitment) -> Result<StepResult, ExecError>; }

struct StepResult {
    attestation: Attestation,     // §3.4 below
    actual_cost: Microdollars,
    emitted: bool,                // mirrors EgressOutcome.emitted (false on dedup-replay, §3.10)
    final_state: StepState,
}
// Per-step resume points (names the §5 crash-recovery truth-table rows). DECLARATION ORDER pinned.
enum StepState { Proposed, Committed, ExecAttempted, Reserved, Dispatched, Attested, Settled }
enum ExecError { ProxyDown, ReserveExceeded, ReserveRefused, Ipc, Internal }
// run_step returns Err(ExecError::ProxyDown) when the supervisor egress proxy is unreachable
// (= the §3.10 fail-closed egress-proxy-down case): the executor emits NO effect.
```

**Attestation** (a `JournalEvent::Attestation(Bytes)` blob — the output of the §5
`dispatched → attested` transition; feeds the trust update). Matches the §4 `attestations` columns:

```rust
struct Attestation {
    step_index: u32, turn_id: Hash, accepted: bool, actual_cost: Microdollars,
    duration_ms: u64,    // per-step wall-clock duration; feeds resource_spike_count replay (§3.9)
    evidence_hash: Hash, subject: Hash, sig: Sig,
}
```

For the M0 no-op echo executor `accepted` defaults to `true`. `accepted` **drives the trust
up-update** (`alpha += W_UP`, §3.9); `accepted == false` (or a damage/overrun signal) drives
`beta += W_DOWN`.

**Candidate-plan selection (v0).** `commit()` operates **only on `candidate_plans[preferred]`**; the
other candidate plans are **advisory and ignored** in v0. The `committed_steps`/`rejected` partition
derives **from the preferred plan only**.

**`commit()` dispatch rule (deterministic core vs model-mediated fallback).** `commit()` decides
**deterministically**: access control via trust thresholds + policy, budget reservation, schema/arg
validation. The model-mediated fallback judge is invoked **iff** the deterministic core admits **AND**
(the step's `EffectClass` is in the high-stakes set `{Payment, Http, Sign}` **OR**
`advisory_risk > RISK_THRESHOLD`); otherwise the deterministic verdict stands.

```rust
trait FallbackJudge { fn adjudicate(&self, step: &PlanStep, c: &ContextPack) -> JudgeVerdict; }
// IPC call to an operator-owned process; non-heritable; distinct from the proposer.

enum JudgeVerdict { Admit, Reject { reason: String }, EscalateToHuman }
```

**`commit()` final-admit merge (pins `committed_steps`/`rejected` identically across implementers).**
For a step the deterministic core admits **and** the fallback judge is consulted (per the
high-stakes/risk rule above), the merge is:
- `final_admit = deterministic_admit AND verdict != Reject` (i.e. `Admit` is required to keep a step
  the deterministic core admitted; the judge can only veto, never resurrect a deterministically
  rejected step — the judge is invoked only on the deterministic-admit branch).
- On `verdict == Reject { reason }`: the step lands in `Commitment.rejected` with that judge `reason`.
- On `verdict == EscalateToHuman`: set `needs_human = true` and the step lands in `Commitment.rejected`
  with reason `"escalated_to_human"`.
- On `verdict == Admit`: the step stays in `committed_steps`.

The consulted `JudgeVerdict` is recorded in `Commitment.fallback_verdict` (journaled) so replay honors
it without re-calling the non-deterministic judge.

**Committer-determinism pins:**
1. `HIGHSTAKES_FALLBACK_CAP` (§7 = 0.25, a **fraction**) is converted to an **integer cap against a
   fixed denominator**: over-cap iff `(journaled fallback count this turn) > ceil(HIGHSTAKES_FALLBACK_CAP
   * MAX_ITERS)`. The count is **per-turn** (reset on a new `turn_id`) and **recovered on replay by
   counting journaled fallback invocations** (i.e. `Commitment` events whose `fallback_verdict.is_some()`).
   The cap is **evaluated at each fallback invocation**; over-cap ⇒ `needs_human = true`.
2. The `FallbackJudge`'s `JudgeVerdict` is **recorded in the `Commitment`** (`fallback_verdict` field,
   journaled with the opaque blob) so replay honors the journaled verdict **without re-calling the
   non-deterministic judge** (mirroring the arch §4.5 commit-time-snapshot lean).

**`objective_score` formula (replay-reproducible from journaled inputs).**

```
objective_score = advisory_EV - risk_penalty - (cost_ceiling / COST_SCALE)
```

where `advisory_EV` and `risk_penalty` (= f(`advisory_risk`)) are **pinned-at-commit** unit-scale `f64`
values from the proposal, and `cost_ceiling` here is the **sum over `committed_steps` of
`cost_ceiling(step)`** (per-Commitment aggregate, matching the M5 `cost_floor` aggregation, §6 M5),
**divided by `COST_SCALE` (§7)** so the cost term is unit-scaled. **Units pin:** `cost_ceiling` is
microdollars (`u64`, ~hundreds) while `advisory_EV`/`risk_penalty` are unit-scale `f64`; without the
`COST_SCALE` divisor the cost term dominates and `PROGRESS_MARGIN = 0.01` is never satisfiable. After
dividing, all three terms are unit-scale, so `PROGRESS_MARGIN` is meaningful. Because all three are
journaled, replay reproduces `objective_score` exactly.

**`budget_verdict`** is set **provisionally** by the committer from the deterministic ceiling vs the
budget replica. When the supervisor `reserve` later returns `Exceeded`/`Refused`, the §5
`reserved → committed/journaled` **batch-release** transition applies; the `Commitment` is **not**
re-journaled.

**v0 `StepArgs` validation rules (the committer's `rejected` partition is objective).** A committed step
is admitted iff **both** hold; otherwise it lands in `Commitment.rejected` with the stated reason
string:
- **(a)** the step's `EffectClass` is in `capability_ceiling(trust_snapshot)` (§3.9) —
  reason on failure: `"effect_not_in_capability_ceiling:{EffectClass}"`.
- **(b)** `declared_token_count > 0` — reason on failure: `"declared_token_count_must_be_positive"`.

All **other** `StepArgs` fields (`Payment.amount`/`payee`, `Http.method`/`url`/`body_hash`,
`Sign.over`, etc.) are **accepted opaquely in v0** (richer arg validation is a post-v0 row). The two
reason-string formats above are normative so two implementers produce identical `rejected` tuples.

**Termination responsibility split (committer vs control loop).** The committer encodes in
`continue_loop` **only deterministic pre-execution conditions**:
`continue_loop = (!detect_thrash(loop_state)) AND (iteration < MAX_ITERS)` (§9.1). The **control loop**
evaluates `progress_made` **post-attestation** (§9.1).

**`detect_thrash` fold-window membership at `commit()` time (pinned; replay-relevant).**
`detect_thrash` evaluates over `LoopState` reconstructed from **folded `Commitment`s `[run-start ..
current-1]`** — **the in-progress iteration's own `Commitment` is NOT yet folded and is EXCLUDED** from
the window (the `loop_state` the committer reads was advanced from the previous iteration's
`Commitment`; the just-built `Commitment` is folded into `loop_state` only by the post-iteration
`advance(loop_state, commitment)` step, §3.4 skeleton). This fixes **when `ThrashAborted` fires by one
iteration** and makes it replay-deterministic.

The final per-iteration predicate is:

```
continue = commitment.continue_loop AND progress_made AND !needs_human
```

**Turn outcome (the value the loop returns when `continue == false`):**

```rust
enum TurnOutcome { Completed, NeedsHuman, ThrashAborted, MaxItersReached }
```
Resolution at loop exit, in priority order: `needs_human` ⇒ `NeedsHuman`; else
`detect_thrash(loop_state)` ⇒ `ThrashAborted`; else `iteration >= MAX_ITERS` ⇒ `MaxItersReached`; else
(`continue_loop == true` but `progress_made == false`) ⇒ **`Completed`** (the work is done — no further
progress is available and no abort condition fired, so a stalled-but-healthy loop terminates as
`Completed`, not an error).

**`Being::attempt_with` control-loop skeleton (per-iteration; pseudocode).** `Being` holds (as fields)
`proposer: &dyn Proposer`, `committer: Committer`, `executor: Executor`, the `AccountAuthority` client
stub, the §3.9 being-local trust ledger, the `MemoryStore`, and the per-DID `Journal`. The loop body:

```
turn_id = blake3(canon(first Commitment of this run))   // the run's turn_id; all iterations share it
loop {
  ctx = build_context_pack(percept, domain, memory, trust_ledger, budget_replica, loop_state)   // §3.5
  pctx = project_for_proposer(&ctx, cfg.expose_budget_trust)   // §3.5 ProposerContext (Arm-C projection)
  proposal   = proposer.propose(&pctx)                   // proposer sees the projected subset only
  commitment = committer.commit(&proposal, &ctx)         // commit() always sees the FULL ctx (§3.4)
  journal.append(JournalEvent::Commitment(canon(commitment)), now)
  for step in commitment.committed_steps (in step_index order) {
    // §5 per-step state machine via executor.run_step:
    //   fsync ExecMarker -> reserve (IPC, batch) -> dispatch -> attest -> settle
    result = executor.run_step(&step, &commitment)?      // emits Attestation
    journal.append(JournalEvent::Attestation(canon(result.attestation)), now)
  }
  apply §3.9 per-iteration fold steps (1) pin trust_snapshot -> (2) attest-time Beta up/down ->
                                       (3) contraction counter/effect updates -> (4) decay tick
  progress_made = (this iteration produced an attested effect)   // == at least one committed step's
                  //   Attestation has accepted == true this iteration (§9.1)
                  AND (commitment.objective_score > prev_objective_score + PROGRESS_MARGIN)   // §9.1
  continue = commitment.continue_loop AND progress_made AND !commitment.needs_human            // §3.4
  loop_state = advance(loop_state, commitment)           // iteration++, prior_intents/scores append
  if !continue { break }
}
return assemble_turn_output(turn_id, committed_steps, attestations)   // §3.0 TurnOutput, answer per below
```

The termination predicate `continue` and the `TurnOutcome` resolution above are evaluated each
iteration; the loop returns the resolved `TurnOutcome` alongside the assembled `TurnOutput`.

**v0 echo proposer plan (`being-proposer-echo`).** On each `propose`, the echo proposer emits **exactly
one** `PlanStep { step_index: 0, effect: Infer, args: StepArgs::Infer(percept.body),
declared_token_count: len(percept.body) / 4, tool_id: None, tariff_ref: None }`; `intent =
utf8(percept.body)`; `candidate_plans = [that single-step plan]`, `preferred = 0`;
`advisory_EV = 0.0`, `advisory_risk = 0.0`, `est_cost = cost_ceiling(Infer, declared_token_count,
None)`.

**`TurnOutput.answer` source.** `TurnOutput.answer = the body of the last committed `Infer`/`Query`/
`Render` step's output` (the bytes the dispatched effect produced). For the echo proposer this is
exactly `percept.body` (`Infer` echoes its argument). Without this the M0 turn cannot complete
deterministically and bench grading has no artifact to compare against `BenchItem.reference`.

### 3.5 Plan, ContextPack, LoopState (`being-runtime`)

```rust
struct PlanStep {
    step_index: u32, effect: EffectClass, args: StepArgs,    // StepArgs defined in §3.0
    declared_token_count: u32, tool_id: Option<String>, tariff_ref: Option<String>,
}
struct Plan { steps: Vec<PlanStep> }

// deterministic pre-execution cost ceiling; the committer reads exactly these fields:
fn cost_ceiling(effect: EffectClass, declared_token_count: u32, tariff_ref: Option<&str>) -> Microdollars;
// FORMULA: cost_ceiling = base_rate(effect) + declared_token_count * token_rate(tariff_ref)
//   base_rate(effect) and token_rate(tariff_ref) come from the §7 default tariff/base-rate table.
//   risk_penalty (used by objective_score, §3.4) = RISK_WEIGHT * advisory_risk (RISK_WEIGHT in §7).

struct ContextPack {
    percept_normalized: Percept,                         // §3.0
    model: ModelRef,                                     // = Router::route(item/percept, genome) at pack-build time;
                                                         //   the SOLE inference target for propose()/Infer/Query (below)
    retrieved_episodic: Vec<EpisodicRow>,                // §3.8
    retrieved_semantic: Vec<SemanticRow>,                // §3.8
    identity_snapshot: IdentitySnapshot,
    trust_snapshot: BTreeMap<EffectClass, TrustLevel>,   // pinned-to-journaled-value
    budget_replica: BudgetReplica,                       // pinned (not hashed into commitment_hash)
    loop_state: LoopState,
}
struct IdentitySnapshot { did: Did, generation: u32, genome_ref: Hash }
struct BudgetReplica   { balance: Microdollars, lease_ttl_ms: u64 }
struct LoopState {
    iteration: u32, prior_intents: Vec<Hash>, prior_objective_scores: Vec<f64>,
    summarized_history: Bytes, repeat_signature_counts: BTreeMap<Hash, u32>,
}

// The ContextPack subset visible to Proposer::propose (the Arm-C visible-vs-hidden projection).
struct ProposerContext {
    percept_normalized: Percept,
    retrieved_episodic: Vec<EpisodicRow>,
    retrieved_semantic: Vec<SemanticRow>,
    model: ModelRef,
    loop_state: LoopState,
    // present IFF ConditionFlags.expose_budget_trust == true; None otherwise (Arm C hidden condition):
    budget_replica: Option<BudgetReplica>,
    trust_snapshot: Option<BTreeMap<EffectClass, TrustLevel>>,
}
// project_for_proposer(ctx: &ContextPack, expose_budget_trust: bool) -> ProposerContext copies the
//   percept/retrieval/model/loop_state through unconditionally and includes Some(budget_replica)/
//   Some(trust_snapshot) iff expose_budget_trust, else None. commit() always reads the full ContextPack
//   (trust_snapshot + budget_replica are deterministically required for access control + budget). The
//   echo proposer ignores the projection.
```

**`build_context_pack` signature + the turn's `Domain` (pinned).** The turn's `Domain` is
**operator/dispatcher-supplied** — the **same task-dispatch record that carries `TaskOrigin` in
`SpendCtx`** (§3.6 reserve batch tuple) — and is threaded into `Being::attempt_with`/`run_turn` and on into
`build_context_pack`:

```rust
fn build_context_pack(percept: &Percept, domain: &Domain, memory: &MemoryStore,
                      trust_ledger: &TrustLedger, budget_replica: &BudgetReplica,
                      loop_state: &LoopState) -> ContextPack;
```

`EpisodicPayload.domain` is a turn **OUTPUT** (the domain the turn was graded under) and therefore
cannot be the retrieval input filter; the **dispatch-supplied `domain` parameter is the filter** for
`retrieved_episodic` (below).

**ContextPack population (v0 default).** The runtime fills `ContextPack` per turn as:
- `percept_normalized` = **identity** in v0 (the ingress `Percept` unchanged).
- `model` = **`Router::route(item, genome)`** (§3.11) computed at pack-build time from the turn's
  `BenchItem`/`Percept` (its `Domain`) and the being's **current genome** — this is the **sole**
  model-selection point. `propose()` and **every** `Infer`/`Query` executor `run_step` dispatch MUST
  use `ctx.model` as the inference target; **there is no second inference path** (resolving the §3.0 "Single inference entrypoint" note).
  The **echo proposer ignores `ctx.model`** (it echoes `percept.body` without inference, §3.4).
- `retrieved_episodic` = `query_as_of(did, now, now)` (§3.8) **limited to the `K = CONTEXT_RETRIEVAL_K`
  (§7) most-recent rows whose `EpisodicPayload.domain == domain`** (the dispatch-supplied `domain`
  parameter above, NOT a turn output), most-recent by `ts_txn`.
- `retrieved_semantic` = `recent_semantic(did, K = CONTEXT_RETRIEVAL_K)` (§3.8): the `K` rows of
  greatest `ts_txn`, tie-broken by `id` desc, **excluding `trained_on_untrusted` rows**.
- the genome's opaque `retrieval_policy` blob is **uninterpreted in v0** (carried, not consulted).
When `ConditionFlags.retrieval_enabled == false` (ArmA baseline, §3.12) both `retrieved_*` vectors are
**empty** — this is the toggle that ArmA's empty-retrieval condition flips.

`trust_snapshot` and `budget_replica` are **pinned-to-journaled-value** snapshots: they are *not*
inputs to `commitment_hash` (they reflect supervisor-authoritative state and would break replay
determinism if hashed).

**`budget_replica` population (pinned).** At `ContextPack` build the runtime issues
`IpcRequest::Balance { did }` (returns the Survival-pot balance, §3.10) and stamps it into
`budget_replica.balance` with `lease_ttl_ms = the §7 lease_ttl_ms constant` (= `EGRESS_TOKEN_TTL_MS`,
2000). The committer's **provisional** verdict is then
`budget_verdict = if sum(cost_ceiling over committed_steps) > budget_replica.balance { Exceeded } else
{ WithinBudget }`; the authoritative verdict comes from the supervisor `reserve` (§3.6), and a later
`Exceeded`/`Refused` triggers the §5 batch-release without re-journaling the `Commitment`.

**Turn ↔ iteration ↔ `Commitment` cardinality (pinned).** One control-loop run = **ONE turn** that may
emit **N `Commitment`s** (one per iteration, §9.1), **all sharing the run's `turn_id`** (§3.4 skeleton).
Reserves **group by `turn_id`** (NOT by per-iteration `commitment_hash`): the §3.6 reserve batch for a
step uses the run's `turn_id`. **Loop-run boundary marker (journaled):** the run is delimited in the
journal by the **first `Commitment` of the run** (whose `commitment_hash` = the run's `turn_id`); the
§3.5 replay fold treats a `Commitment` whose `nonce` opens a new `turn_id` as the run-start marker.
`iteration` = count of `Commitment` events folded **since the run's start marker**; `prior_intents`/
`prior_objective_scores` reconstruct from those same folded `Commitment`s — so the "within the turn"
fold is unambiguous and the `turn_id`/`commitment_hash` distinction is fixed.

**StepArgs → egress payload + `TurnOutput` assembly (pinned).** For a dispatched egress step the
`IpcRequest::Egress` `payload = canon(StepArgs)` of that step. For `StepArgs::Http` (which carries only
`body_hash`, not the body) the runtime **resolves the body from a content-addressed store keyed by
`body_hash`** before dispatch (the body was written to that store when the plan was built). At turn
assembly, `TurnOutput.steps = Commitment.committed_steps in step_index order` and
`TurnOutput.attestations = the produced Attestations in the same step_index order`.

**`summarized_history` is a deterministic pure function of journaled events (NOT model-mediated — the
Consolidator is never invoked here).** This is what makes the M0 byte-identical `canon(LoopState)`
oracle hold:

```
summarized_history = canon(truncate(prior_intents, prior_objective_scores) after SUMMARIZE_S iterations)
```

i.e. once `iteration >= SUMMARIZE_S`, the oldest entries of `prior_intents`/`prior_objective_scores`
are dropped (keeping the most-recent `SUMMARIZE_S`) and the retained tail is `canon`-encoded. No model,
no Consolidator.

**`LoopState` reconstruction algorithm (replay).** To reconstruct `LoopState` at any point, fold
`replay_from(s)` over `Commitment` events and recompute:
- `iteration` = count of `Commitment` events folded so far (within the turn);
- `prior_intents` = the ordered `blake3(canon(intent))` of each folded `Commitment`;
- `prior_objective_scores` = the ordered `objective_score` of each folded `Commitment`;
- `repeat_signature_counts` = the windowed step-signature counts (reconstructed per §9.1 / the
  windowing rule below);
- `summarized_history` = the deterministic function above.
- **per-step final `budget_verdict`** = derived by **folding `JournalEvent::BatchRelease.released`
  against the original `Commitment`'s steps**: a `step_index` present in any `BatchRelease.released` for
  the turn ⇒ `Exceeded`; otherwise ⇒ `Granted`. The `Commitment` is **NOT re-journaled** on a
  budget-release (§3.4); the executor consults this fold for each step's authoritative budget verdict
  on replay.

The determinism oracle canon-encodes the **FULL `LoopState`** (all five fields above, all recomputable).
The `ContextPack` fields `trust_snapshot` and `budget_replica` are **exempt from the separate
`ContextPack`-equality check** (they are supervisor-authoritative replicas, pinned-not-hashed) — note
they are `ContextPack` fields, **not** `LoopState` fields, so the `LoopState` oracle is unaffected.

**`repeat_signature_counts` windowing.** `repeat_signature_counts` is **windowed to `THRASH_W`** (the
window applied at `detect_thrash`, §9.1) and is **reconstructed on replay from the journaled committed
step `(effect, args)` signatures** (`blake3(canon(effect) || canon(args))`), so it is part of the
byte-identical replayed `LoopState`.

### 3.6 Economy (`being-core-economy`) — Account; canonical balance lives in the supervisor

```rust
trait AccountAuthority {                 // implemented by being-supervisor only; client stub in being-runtime
    // per-turn BATCH reserve: all ledger rows inserted in ONE SQLite txn, or none (atomic).
    // each row carries SpendCtx so the supervisor pins its Pot via classify_spend at reserve time.
    fn reserve(&mut self, did: &Did, batch: Vec<(IdemKey, Microdollars, EffectClass, SpendCtx)>, turn_id: Hash) -> ReserveVerdict;
    fn settle(&mut self,  did: &Did, actual: Microdollars, key: IdemKey, lease_id: Hash); // idempotent
    fn balance(&self, did: &Did) -> Microdollars;                          // returns Survival-pot balance (below)
    fn pot_balance(&self, did: &Did, pot: Pot) -> Microdollars;            // per-pot accessor (§3.10)
    // credit entrypoint (distinct from reserve; the ONLY way a pot gains balance):
    fn credit(&mut self, did: &Did, pot: Pot, amount: Microdollars, source: CreditSource) -> ReserveVerdict;
}
enum CreditSource { External, Internal }
// Partition rule (the §3.6 row-1 survival predicate): source != External => pot != Survival
//   (an Internal-sourced credit to Survival returns Refused). External may credit any pot.
// Genesis pot balances are established by an operator-issued credit(did, pot, amount, External) per pot
//   at being creation (the Survival pot is seeded External so it starts solvent); without these the
//   pots start at 0 and every reserve returns Exceeded. credit crosses the socket as
//   IpcRequest::Credit { did, pot, amount, source } -> IpcResponse::Credit(ReserveVerdict).
```

**`SpendCtx` at reserve time + per-row `Pot`.** Each batch row carries an `EffectClass` (persisted to
the `ledger.effect_class` column; backs the in-flight egress filter and the reaper decrement) and a
`SpendCtx` (§3.10); the supervisor calls `classify_spend(SpendCtx)` per row to pin its `Pot` (the
`ledger.pot` column is mandatory). The supervisor derives `SpendCtx` (including `task_origin`) from the **turn/DID context**:
`task_origin` is the `TaskOrigin` the operator/dispatcher stamped on the task that opened this
`turn_id` (carried in the turn's dispatch record); a being-asserted or unknown origin maps to
`Survival` (fail-safe, §3.10).

**`reserve()` verdict decision table (the referent for the economy slice and the M1 audit).** For each
batch row, compute its `Pot` via `classify_spend` and its per-pot debit. Checks are evaluated **in this
order**; the **first** match wins:

| order | condition | verdict |
|---|---|---|
| 1 | **structural rejection** — unknown `did`, malformed batch, **or** survival-partition violation (a **credit-kind** entry whose `source != External` targets the `Survival` pot, §3.10) | **`Refused`** |
| 2 | within structure, but a pot balance would go **negative** (`pot_balance(did, pot) - row_debit < 0` for that row's pot) **OR** the `B_INFLIGHT` bound would be breached (below) **OR** the per-turn effect-count cap would be breached | **`Exceeded`** |
| 3 | otherwise | **`Granted { lease_id, expires_wall_ts, reserved }`** |

This makes the M1 property "reserve rejects over-cap (budget binds)" have an objective referent: an
over-cap batch returns `Exceeded`; a structural violation returns `Refused`.

**`reserve()` is all-or-nothing (partial-grant ownership pinned).** `reserve()` returns **ONLY**
`Granted` (the **full** batch reserved, all rows inserted in one txn) or `Exceeded`/`Refused` (**zero
rows inserted**). There is no partial-grant return shape. On `Exceeded`, the **caller (the executor)**
computes the survivor subset — drop steps in **descending `step_index`** until the batch total `<=
available balance` for the binding pot (§"Turn-level `Exceeded` → per-step `BatchRelease`" below) — and
**re-issues `reserve()` with that survivor subset**, same `turn_id`. The **supervisor journals
`JournalEvent::BatchRelease` for the dropped step indices at re-reserve time** (the supervisor is the
`BatchRelease` emitter on the re-reserve path; the reaper sweep is the emitter on the TTL-release path,
§3.6 reaper sweep).

**Per-pot negativity check is cumulative within the batch (pinned).** The verdict-table row-2 negativity
check is **cumulative per pot across the batch**, not per-row against a pre-batch snapshot: for each pot
`p` touched by the batch, require `pot_balance(did, p) − sum(batch debits to p) >= 0`; if any pot fails,
**reject the whole batch (`Exceeded`)**. This prevents a same-pot multi-row batch from breaching the
Survival floor via per-row pre-snapshot checks. The validating SQL the supervisor runs **before
COMMIT** (one row per binding pot; `:debits_p` = the summed incoming batch debits to pot `p`):

```sql
-- for each pot p in the batch, assert availability covers the cumulative batch debit to p:
SELECT pot,
       COALESCE(SUM(CASE kind WHEN 'credit' THEN microdollars
                              WHEN 'debit'  THEN -microdollars
                              WHEN 'settle' THEN -microdollars
                              WHEN 'reserve' THEN -microdollars END), 0) AS available
FROM ledger WHERE did = ?1 AND pot = :p
HAVING available - :debits_p < 0;   -- any returned row ⇒ Exceeded; reject the whole batch
```

**Survival-partition predicate scope (reserve path is debit-only).** A `reserve` row **only ever
debits its own `classify_spend`-pinned pot** — it never credits and never touches a pot other than its
own — so the row-1 survival-partition clause is **vacuous on the reserve path** and applies **only to
credit-kind entries** (which arrive via the §3.6 `credit()` entrypoint, not `reserve()`). The
predicate over ledger columns is exactly: **REJECT (`Refused`) iff `kind == credit AND source !=
External AND pot == Survival`** (an `Internal`-sourced credit cannot fund Survival). Every other
`(kind, pot, source)` combination passes the survival clause.

**`B_INFLIGHT` unit + scope (pinned).** `B_INFLIGHT` is a **per-DID COUNT of reserved-but-not-settled
ledger rows of class `{Http, Notify, Render, Payment}`**, measured **at `reserve()` time across all
turns** for that DID. The exact comparison: `reserve()` returns `Exceeded` if
`inflight_egress_count(did) + batch_egress_count > B_INFLIGHT`, where
`inflight_egress_count(did)` = `COUNT(*) FROM ledger WHERE did=?1 AND kind='reserve' AND effect_class
IN ('Http','Notify','Render','Payment')` (not yet settled/released — the `effect_class` column is the
per-row `EffectClass` carried in the reserve batch tuple), and `batch_egress_count` = count of such
rows in the incoming batch. The reaper's per-DID egress decrement (§3.6) filters on the **same
`effect_class` column**.

**`PER_TURN_EFFECT_COUNT_CAP` unit + SQL (pinned exactly as `B_INFLIGHT`; CUMULATIVE per turn).** The
per-turn effect-count cap is **cumulative over the turn** (counts reserved + settled + released rows for
this `turn_id`), not in-flight:

```sql
-- per_turn_count(did, turn_id):
SELECT COUNT(*) FROM ledger
WHERE did = ?1 AND turn_id = ?2
  AND effect_class IN ('Http','Notify','Render','Payment');
```

`reserve()` returns `Exceeded` (verdict-table row 2) iff
`per_turn_count(did, turn_id) + batch_egress_count > PER_TURN_EFFECT_COUNT_CAP` (§7). The counter is
**per-`turn_id`** (it does not reset within a turn; a new `turn_id` is a fresh count). `B_INFLIGHT`
(in-flight, cross-turn) and `PER_TURN_EFFECT_COUNT_CAP` (cumulative, per-turn) are **two distinct
named bounds over different row sets** — they are NOT combined under a `min()` (§3.10).

`IdemKey` (defined in §3.0): `commitment_hash = blake3(canon(Commitment))` excluding the outer `Sig`
but including the journaled nonce. **`reserve` and `settle` for one step SHARE the idem_key**; settle
transitions the same ledger row's `kind` in place; a duplicate settle is a no-op. The ledger
`idem_key UNIQUE` constraint therefore yields exactly one row per step (§4).

**`idem_key` storage form (pinned once, cross-component).** The `ledger.idem_key` column stores
**`canon(IdemKey)` as a BLOB with a UNIQUE index**; `DedupToken.idem_key` (§3.10) keys on the
**identical `canon(IdemKey)` bytes**; the `dedup_ledger` `idem_key` column (§4) uses the same
`canon(IdemKey)` BLOB. This single form makes at-most-once hold across the economy, supervisor, and
dedup ledger.

**Idempotent reserve/settle replay (reconnect reconstructs the original `Granted`).** The ledger
persists `expires_wall_ts` and the reserved total as columns, so a re-issued `reserve()` for an
existing row reconstructs the **exact original verdict**. Per existing row `kind`:
- **`reserve`** → return the **original `Granted`** (same `lease_id`, same `expires_wall_ts`, same
  `reserved`) read from the row; do not re-debit.
- **`settle`** → a duplicate **`settle()`** returns the **`IpcResponse::Settle` ack (unit)**, no-op
  (there is no `ReserveVerdict::Settled` variant — that was a naming error). A **duplicate
  reserve-after-settle** returns `IpcResponse::Reserve(Granted{ original lease_id, expires_wall_ts,
  reserved })` read from the existing (now-settled) row, with **no re-debit**.
- **absent** → **insert** the row (the normal path).

**`settle()` actual-cost reconciliation (pinned).** On the in-place `reserve → settle` transition,
`settle()` writes **`microdollars = actual`** on the row (replacing the reserved ceiling) so the ledger
reflects the **true settled cost**. A **duplicate settle is a no-op** that **ignores the new `actual`**
(first-settle-wins) and returns the recorded `IpcResponse::Settle` ack. The `pot_balance` `kind=settle`
term (§3.10) therefore reads this **actual** settled cost — directly fixing Survival-floor sampling and
reaper insolvency correctness.

This preserves reaper TTL determinism: the re-issued lease carries the original `expires_wall_ts`, so a
reconnect cannot extend a lease.

**Turn-level `Exceeded` → per-step `BatchRelease` selection.** On a turn-level `Exceeded`, the
supervisor selects the released step set deterministically: **drop steps in descending `step_index`
(tie-break that is total over the batch) until the batch total `<= available balance`** for the
binding pot. The dropped `step_index` set is `BatchRelease.released`; the **survivors are re-reserved
the same `turn_id`** (the surviving `(IdemKey, Microdollars, EffectClass, SpendCtx)` subset). The **per-step
`budget_verdict` for journaling** is `Granted` for survivors and `Exceeded` for released steps.

**Atomic unit of reserve = per-turn batch.** The `batch` is **all steps sharing one
`commitment_hash` (the `turn_id`)**. `reserve()` inserts **all** ledger rows in **one SQLite
transaction or none** — this is the referent for "all-or-nothing", "survivors", and `BatchRelease`.

**`lease_id` mint (deterministic, journal-derived).** On the **first** `reserve` for a turn the
supervisor computes `lease_id = blake3(canon(turn_id || did || expires_wall_ts))` **once** and stores
it on the ledger rows; on replay/reconnect it is **read back** from the row (never recomputed from a
fresh clock, never random) so a reconnect cannot mint a different lease. This matches the
journal-derived determinism discipline.

**`ReserveVerdict` (§3.0).** `Granted` carries a `lease_id`, the wall-clock `expires_wall_ts`, and the
`reserved` total. `settle()` and the §5 `reserved → dispatched → settled` transitions carry the
`lease_id` (alongside the shared `IdemKey`). The reaper **auto-releases** a reservation when
`now > expires_wall_ts`. `lease_ttl_ms` in `BudgetReplica` is the TTL used to compute
`expires_wall_ts` (= `now + lease_ttl_ms*1000`).

**Supervisor clock source (pinned).** The supervisor reads **host wall-clock micros** for all three of:
the `reserve` lease `expires_wall_ts` mint, the reaper sweep, and `DedupToken` expiry — **the same
clock, the same units (micros)**. Thus `expires_wall_ts = now_micros + lease_ttl_ms * 1000`.

**Reaper auto-release sweep (cadence).** A supervisor timer scans `kind = reserve` ledger rows with
`now > expires_wall_ts` **every `REAPER_SWEEP_MS`** (§7) and releases each: `UPDATE` the row's `kind`
to released, **decrement the in-flight egress count** for that DID, and **journal a `BatchRelease`**
event. This is what makes the M1 "reaper fires" timing and the in-flight-bound property test
reproducible.

**`BatchRelease` semantics.** `BatchRelease.released: Vec<u32>` is the **`step_index` list whose
ledger rows are released**. "Re-reserve survivors" = re-issue `reserve()` with the **surviving subset
of `(IdemKey, Microdollars, EffectClass, SpendCtx)` rows** (the batch minus the released step indices), same
`turn_id`.

### 3.7 Mutation (`being-core-mutation`) — the CLOSED surface (arch §8.1)

```rust
enum MutationKind { Prompt(String), ToolPolicy(Bytes), RetrievalPolicy(Bytes),
                    DecompositionPolicy(Bytes), RoutingPolicy(Bytes),
                    ReasoningNavigator(ModelRef), DomainModel(Domain, ModelRef),
                    SkillInstall(SignedSkill), SkillRevoke(SkillId) }
// NOT representable: CapabilityGrant, TrustPolicyModify, SignatureBoundaryChange, ExecutionKernel,
//                    BudgetRules, Reaper. Adding a variant is a compile error.

// discriminant enum (one tag per MutationKind arm):
enum MutationKindTag { Prompt, ToolPolicy, RetrievalPolicy, DecompositionPolicy, RoutingPolicy,
                       ReasoningNavigator, DomainModel, SkillInstall, SkillRevoke }
struct MutationArm { kind_tag: MutationKindTag }
struct OutcomeSignal { reward: f64, cost_micros: Microdollars, gate_passed: bool }
```

**The central type — `Genome`.** Every signature below takes `&Genome`. `BTreeSet`/`BTreeMap` (not
`Hash*`) so `canon(Genome)` is byte-deterministic.

```rust
struct Genome {
    prompt: String,
    tool_policy: Vec<u8>,
    retrieval_policy: Vec<u8>,
    decomposition_policy: Vec<u8>,
    routing_policy: Vec<u8>,
    reasoning_navigator: ModelRef,
    installed_skills: BTreeSet<SkillId>,
    domain_models: BTreeMap<Domain, ModelRef>,
}
```

**`MutationKind` arm → field-write mapping (`apply`):**

| `MutationKind` arm | Field write |
|---|---|
| `Prompt(s)` | `prompt = s` |
| `ToolPolicy(b)` | `tool_policy = b` |
| `RetrievalPolicy(b)` | `retrieval_policy = b` |
| `DecompositionPolicy(b)` | `decomposition_policy = b` |
| `RoutingPolicy(b)` | `routing_policy = b` |
| `ReasoningNavigator(m)` | `reasoning_navigator = m` |
| `DomainModel(d, m)` | `domain_models.insert(d, m)` |
| `SkillInstall(s)` | `installed_skills.insert(skill_id(&s.manifest))` |
| `SkillRevoke(id)` | `installed_skills.remove(&id)` |

```rust
// apply is TOTAL and UNCONDITIONAL: the field-write table (§3.7) always applies. validate() is the
// SOLE checker; the committer calls validate(k, g, ctx) FIRST and calls apply(k, g) ONLY when validate
// returns Ok — so apply is NEVER reached on validate-failing input. (For opaque-Bytes variants apply
// never no-ops: opacity ⇒ no malformed case at this layer.)
fn apply(k: MutationKind, g: &Genome) -> Genome;
fn validate(k: &MutationKind, g: &Genome, ctx: &MutationCtx) -> Result<(), ApplyError>;

// Injected context so UnknownDomain/BaseHashMismatch/UnsignedSkill are buildable:
struct MutationCtx {
    known_domains: BTreeSet<Domain>,   // operator-known Domains
    frozen_base_hash: Hash,            // the single frozen base all Adapters attach to (§3.11)
    skill_verifying_key: Did,          // the BEING'S OWN DID — it signs its own SignedSkill manifests
}

enum ApplyError {
    MalformedPayload,   // reserved; not produced by opaque-Bytes arms
    UnknownDomain,      // DomainModel(d,_) where d ∉ ctx.known_domains
    BaseHashMismatch,   // ModelRef.base_hash != ctx.frozen_base_hash (Adapter refs only)
    AbsentSkill,        // SkillRevoke(id) where id ∉ g.installed_skills
    UnsignedSkill,      // SkillInstall where verify(ctx.skill_verifying_key, canon(manifest), sig) == false
}

trait Improver {
    fn propose_arm(&mut self, chain_head: Hash) -> MutationArm;   // head injected per call (below)
    fn record(&mut self, a: MutationArm, o: OutcomeSignal);
}
// Bandit: per-arm running mean of OutcomeSignal.reward; epsilon-greedy with EPSILON (§7).
// record() uses o.reward DIRECTLY in the running-mean update (the producer supplies the already-
// normalized scalar; record does NOT re-derive). Producer derivation:
//   reward = (val_risk_base - val_risk_candidate) / max(cost_micros, 1)
//   then normalized by dividing by the running max reward per arm.
// NORMALIZATION INIT (producer-side, NOT record()): running_max starts at 0; on each producer update
//   running_max = max(running_max, raw); normalized = raw / max(running_max, epsilon); raw rewards are
//   clamped to >= 0 before the update. This lives in the PRODUCER, not in record().
// DETERMINISM (replay-critical — same posture as the §3.0 journal-derived seed): arms = the 9
//   MutationKindTag variants in DECLARATION ORDER. Greedy picks the max running mean; ties broken by
//   declaration order. An UNOBSERVED arm has mean 0.0.
// RUNNING-MEAN REPRESENTATION (pinned for byte-replayable greedy tie-breaks): store (sum: f64,
//   count: u64) PER ARM; mean = sum / count as f64; an UNOBSERVED arm (count == 0) has mean 0.0;
//   tie-break by declaration order. record() adds o.reward to sum and increments count. (Storing
//   (sum, count) and dividing — NOT an incremental running mean — avoids last-bit divergence that can
//   flip the chosen arm on replay.)
// HEAD INJECTION + RE-SEED RULE (one fix, both findings): propose_arm takes `chain_head: Hash` (the
//   caller passes head().1; equivalently the Improver is constructed with a head-provider closure). A
//   FRESH ChaCha8Rng is seeded from low 64 bits of `chain_head` on EACH call:
//   RNG = rand_chacha::ChaCha8Rng::seed_from_u64(seed). The EPSILON Bernoulli draw consumes the FIRST
//   u32 of that stream; THEN, if exploring, arm index = rng.gen_range(0..9) consumes the NEXT draw
//   (rand 0.8 Uniform-over-[0,9) rejection-sampling semantics). It is a fresh re-seeded stream per
//   call, NOT a persisted continuing stream. Whether `chain_head` advances between consecutive
//   proposals within a turn: the head ADVANCES only when a GenomeMutation is journaled — back-to-back
//   propose_arm calls with NO intervening journal append observe the SAME `chain_head` and thus draw
//   identically (a property the M4 test relies on); a journaled mutation between calls advances the
//   head and re-seeds. So the arm sequence is byte-replayable.

fn two_gate(candidate: &Genome, base: &Genome, evidence: &Evidence, seed: u64) -> GateVerdict;
// `seed` is a 4th PARAMETER (chosen over an Evidence field): being-core-mutation is forbidden a journal
//   dep (§3.7), so two_gate cannot read head() itself. The CALLER (being-runtime/being-distill) passes
//   the LOW 64 BITS of head().1 at gate time. The Validation Gate body calls paired_bootstrap_p(.., BENCH_B,
//   seed) with this param so `p` is deterministic on replay (the §3.0 journal-derived-seed discipline).
```

**apply/validate responsibility split.** `apply()` is **TOTAL**; for opaque-`Bytes` variants it never
no-ops (opacity ⇒ no malformed case at this layer). `validate(k, g, ctx)` is the **sole checker** and
reports, against the injected `MutationCtx`: `UnknownDomain` iff a `DomainModel(d, _)` has
`d ∉ ctx.known_domains`; `BaseHashMismatch` iff **`m.kind == Adapter && m.base_hash !=
ctx.frozen_base_hash`** for the mutation's `ModelRef m` (checked for `DomainModel`/`ReasoningNavigator`
refs) — **`Base` refs are NOT checked against `frozen_base_hash` in v0**; `AbsentSkill` iff a `SkillRevoke(id)`
has `id ∉ g.installed_skills`; `UnsignedSkill` iff a `SkillInstall(s)` has
`verify(ctx.skill_verifying_key, canon(s.manifest), s.sig) == false` (the **being signs its own
skills** with its own DID). The committer calls `validate` first and **only calls `apply` when
`validate` returns `Ok`** — `apply` is **never reached on validate-failing input** (so the genome is
left unchanged by not applying, not by `apply` no-oping).

**Two-Gate types and tests.**

```rust
struct Evidence {
    val_scores_base: Vec<f64>, val_scores_candidate: Vec<f64>,   // item-aligned, equal length, same ordering
    heldout_gap_base: f64, heldout_gap_candidate: f64, n: usize,
}
enum GateVerdict { Admit { effect_size: f64 }, RejectValidation { p: f64 }, RejectCapacity { gap_delta: f64 } }
```

**Gate evaluation order (pinned; matches the M4 protocol).** Evaluate the **Capacity Gate first**; if
it rejects, return `RejectCapacity { gap_delta }` **without** evaluating Validation. Otherwise evaluate
the Validation Gate; if it rejects, return `RejectValidation { p }`. Return `Admit { effect_size }`
**only if both pass**.

- **Validation Gate.** `p = paired_bootstrap_p(val_scores_base, val_scores_candidate, BENCH_B, seed)`
  (§3.0 shared engine; `seed` = the `two_gate` `seed` parameter = low 64 bits of `head().1` at gate
  time, passed by the caller; requires equal length + identical item ordering; one-sided H1: candidate
  improvement). `effect_size = (mean(val_scores_candidate) − mean(val_scores_base)) / pooled_sd`
  (paired Cohen's d over `n` aligned items). **`pooled_sd` = `sample_std(differences)`** with
  `differences[i] = val_scores_candidate[i] − val_scores_base[i]`, using **sample variance (ddof = 1,
  i.e. `n − 1`)**. **When `pooled_sd == 0`:** `effect_size = +inf` if the numerator `> 0`, else `0.0`
  (so `effect_size >= VALIDATION_EFFECT_FLOOR` is objectively checkable). **Admit** iff
  significant (`p < VALIDATION_FDR_Q`) **and** `effect_size >= VALIDATION_EFFECT_FLOOR`. **v0 evaluates
  one arm at a time**, so Benjamini-Hochberg reduces to `alpha = VALIDATION_FDR_Q` (documented
  single-arm reduction; widening `Evidence` to carry a `Vec<(arm, base, candidate)>` family is the M4+
  upgrade row).
- **Capacity Gate (v0 = pure deterministic scalar comparison; the powered NI variant is the M4+
  upgrade row).** The retracted VC-dim additive form is **cut** per arch §15. **Reject** iff
  `heldout_gap_candidate - heldout_gap_base >= CAPACITY_NI_MARGIN` (gap widened) — a fully checkable
  comparison on the two point estimates, **with no power clause**. (The powered non-inferiority
  variant — adding `heldout_gaps_base/candidate: Vec<f64>` to `Evidence` + a named NI test — is the
  M4+ upgrade.) This makes the M4 false-admit protocol computable.

All constants come from the §7 table.

**Genome versioning + reversibility.**

```rust
struct GenomeVersion {
    version: u64, parent_version: u64, mutation: MutationKind,
    prev_genome_hash: Hash, sig: Sig,
}
```

- `prev_genome_hash = blake3(canon(parent_genome))` (the hash of the FULL canon'd parent genome).
- **`GenomeVersion.sig` signed preimage** (distinct from the journal entry_hash sig; mirrors the §3.8
  per-row convention): `sig = Signer.sign(canon(version || parent_version || mutation ||
  prev_genome_hash))`.
- Stored via `JournalEvent::GenomeMutation(Bytes)` — the `Bytes` is `canon(GenomeVersion)` (§3.3).

**`make_version` constructor.**

```rust
fn make_version(mutation: MutationKind, parent: &GenomeVersion, parent_genome: &Genome,
                signer: &dyn Signer) -> GenomeVersion;
// version = parent.version + 1; parent_version = parent.version;
// prev_genome_hash = blake3(canon(parent_genome)); sig over the preimage above.
```

The **monotone `version` source is the passed-in `parent.version`** (incremented by 1) — `make_version`
does NOT depend on `being-core-journal` beyond receiving the counter value, so `being-core-mutation`
stays free of a journal dependency.

**Content-addressed genome snapshot store (backs M4 reversibility).** Each `GenomeMutation` **persists
the FULL post-mutation `canon(Genome)` snapshot** under its hash in the §4 `genomes` table. **Reversal
= fetch the `prev_genome_hash` row and re-install** that full snapshot (full-snapshot undo). Crossover
is **out of v0 scope** (mutation-only, arch §15), so opaque genome `Bytes` need no crossover parser.

### 3.8 Memory (`being-core-memory`)

Rows are signed **inside the crate**; the crate takes a `&dyn Signer` (callers never pre-sign rows).

**`MemoryStore` constructor + in-crate signing.** The store is opened with its **owner DID and
`Signer`**; `append_episodic`/`append_semantic` read `did` and `signer` **from the store** and sign
internally. `did` is the **store-owner DID, never caller-supplied**.

```rust
fn open(path: &Path, did: Did, signer: Arc<dyn Signer>) -> MemoryStore;
// append_episodic/append_semantic sign canon(id || did || valid_from || ts_txn || provenance_class || payload)
//   internally using the stored signer; the call site never pre-signs.
```

```rust
// Episodic + semantic row types. id/did/ts_txn/sig are CRATE-ASSIGNED (never caller-supplied).
struct EpisodicRow {
    // READ-TIME fields: populated on rows RETURNED by query_as_of; IGNORED on the write path
    //   (append_episodic assigns them). Callers read `id` to obtain the crate-assigned EpisodicId that
    //   EpisodicPayload references and SemanticFact.source_episodic_ids need.
    id: EpisodicId,                  // crate-assigned; 0 / unset on the write path
    did: Did,                        // = owner_did; crate-stamped
    ts_txn: u64,                     // crate-assigned wall-clock micros
    sig: Sig,                        // crate-assigned per-row signature (§3.8 episodic signing payload)
    valid_from: u64,                 // wall-clock micros
    valid_to: u64,                   // wall-clock micros; crate assigns u64::MAX on insert unless superseding
    supersedes: Option<EpisodicId>,  // fixed-width LE in canon()
    provenance_class: ProvenanceClass,
    payload: EpisodicPayload,
}
// NOTE: the read-time fields (id/did/ts_txn/sig) are NOT part of the per-row signing preimage other than
//   as the preimage already specifies (the preimage explicitly lists id/did/ts_txn/payload, §3.8) — they
//   are crate-assigned and ignored when EpisodicRow is supplied to append_episodic.
struct EpisodicPayload {             // the §4 minimum-fields struct
    percept: Percept,
    proposal_hash: Hash,
    commitment_hash: Hash,
    attestation_hash: Hash,
    objective_score: f64,
    domain: Domain,                  // the turn's task-class (used by PerDomainStats / GapSlice, §3.11)
    routing_decision: RoutingDecision, // marks whether the Router used the student or a cloud fallback
}
enum RoutingDecision { Student, CloudFallback }  // backs PerDomainStats.cloud_fallbacks (§3.11)
struct SemanticRow {
    // READ-TIME fields: populated on rows returned by recent_semantic; IGNORED on the write path.
    id: SemanticId,                  // crate-assigned; 0 / unset on the write path
    did: Did,                        // = owner_did; crate-stamped
    sig: Sig,                        // crate-assigned per-row signature (§3.8 semantic signing payload)
    ts_txn: u64,                     // crate-assigned wall-clock micros (semantic signing payload below)
    payload: SemanticPayload,        // crate-private canon'd serde blob
    trained_on_untrusted: bool,
}
struct SemanticPayload(Bytes);       // crate-private canon'd serde blob of the schema below

// Concrete blob schema serialized into SemanticPayload.0 via canon (crate-internal serde layout):
struct SemanticFact {
    subject: Bytes,                  // the consolidated claim subject
    relation: String,
    object: Bytes,
    source_episodic_ids: Vec<EpisodicId>,  // provenance: the episodic rows this fact summarizes
    confidence: f64,
}
// crate-gated constructor reachable by the Consolidator (gated like the §3.1 provenance mints):
fn mint_semantic_payload(fact: SemanticFact) -> SemanticPayload;  // = SemanticPayload(canon(fact))

fn append_episodic(&mut self, row: EpisodicRow, authority: &ProvenanceToken) -> EpisodicId; // §3.1 guard
fn append_semantic(&mut self, row: SemanticRow, authority: &ProvenanceToken) -> SemanticId; // Consolidator only
// GUARD: append_semantic verifies authority.minted_by == Consolidator AND authority.allowed ==
//   ModelInference (a gateway-minted token, or any token not minted by the Consolidator, is REJECTED).
//   Semantic provenance is fixed to ModelInference by construction. M0 no-launder test for semantic =
//   "append_semantic with a gateway-minted token is rejected."

// bitemporal read (§4 episodic). TimePoint args are reduced to wall_ts only (generation ignored
// for valid-time comparison).
fn query_as_of(&self, did: &Did, valid_at: TimePoint, known_at: TimePoint) -> Vec<EpisodicRow>;
// SINGLE-OWNER FILTER: query_as_of filters `WHERE did = self.owner_did`; the passed `did` MUST equal
//   self.owner_did, else it returns an EMPTY Vec (a store reads only its own owner's rows).
//   append_episodic/append_semantic ALWAYS stamp owner_did (never caller-supplied, §3.8).

// recent semantic read the ContextPack requires (§3.5 retrieved_semantic):
fn recent_semantic(&self, did: &Did, limit: usize) -> Vec<SemanticRow>;
// Returns the `limit` semantic rows of GREATEST ts_txn, tie-broken by id DESC. Single-owner filter as
//   above (did MUST equal owner_did, else empty). Rows with trained_on_untrusted == true are EXCLUDED.
```

**Crate-assigned fields.** `id` (`EpisodicId`/`SemanticId`), `did`, `ts_txn`, and `sig` are assigned
by the crate, **not** caller-supplied. On insert the crate sets `valid_to = u64::MAX` unless the row
is superseding.

**Per-row signing payload (episodic):** `sig = Signer.sign(canon(id || did || valid_from || ts_txn ||
provenance_class || payload))`. (`id` and `supersedes` are encoded fixed-width LE per §3.0.) This
**episodic** formula (which includes `valid_from`) does **NOT** apply to semantic rows.

**Per-row signing payload (semantic — distinct formula).** `SemanticRow` has no `valid_from` /
`provenance_class` fields, so it gets its own formula:
`sig = Signer.sign(canon(id || did || ts_txn || ProvenanceClass::ModelInference ||
trained_on_untrusted || payload))` — the `provenance_class` term is the **fixed constant
`ModelInference`** (semantic provenance is fixed by construction, §3.8) and `ts_txn` is the
crate-assigned semantic-row column above. `id` is fixed-width LE per §3.0.

**Bitemporal columns + supersession (append-only holds).** `valid_from`, `valid_to`, `ts_txn` are
**`u64` wall-clock micros**; `valid_to` MAX = `u64::MAX`. **`A.valid_to` is NOT physically mutated** —
append-only holds; supersession is resolved **at query time**.

**Writer supersedes-points-at-head invariant.** A superseding row's `supersedes` MUST point at the
**current head of the chain** (the latest non-superseded row for that fact as of write time), not an
arbitrary ancestor — this is what makes `query_as_of`'s "exclude direct supersedes-targets" step yield
exactly one current row per chain (provably correct). **`append_episodic` non-head supersedes
behavior (v0, pinned):** `append_episodic` **ACCEPTS** a non-head supersedes target (the row is still
appended) but **emits the `memory_anomaly_count` counter** (§3.9) — it does **not** reject. (Rejecting
is the stricter post-v0 option; v0 accepts-and-flags so a mis-targeted supersede surfaces as a trust
contraction signal rather than a hard write failure.)

**`query_as_of` exact algorithm** (yielding the worked example below):
1. Select rows with `valid_from <= valid_at` **AND** `ts_txn <= known_at`.
2. **Exclude** any row that is the `supersedes` target of another row whose `ts_txn <= known_at`.
3. Return the **FULL set surviving steps 1–2** — every row visible at `known_at` that is NOT the
   direct `supersedes`-target of another visible row. This yields **exactly one current row per
   supersession chain** (the head of each chain visible at `known_at`); rows belonging to distinct
   facts/chains are all returned. (There is **no** global single-row "take latest-by-`ts_txn`"
   collapse — that would wrongly discard concurrent independent facts. The worked example below has
   one fact and therefore cannot disambiguate this from a global collapse; the multi-fact case is the
   normative referent.)

**Skills (manifest + lineage).** The skill value-types `Manifest`/`SkillId`/`SignedSkill` and the
`skill_id()` fn are **DECLARED in `being-core-types`** (§3.0 ownership list) and re-exported here; this
is their canonical definition, shown in `-memory` because it is their primary consumer. `-mutation`
imports them from `-types` (§3.7), not from `-memory`.

```rust
struct Manifest {
    name: String, version: u32, base_hash: Hash,
    required_caps: Vec<EffectClass>, artifact_ref: Hash, parent_id: Option<SkillId>,
}
// content-addressed (owned by being-core-types):
fn skill_id(m: &Manifest) -> SkillId;     // = blake3(canon(manifest)) — revisions never collide
type SkillId = Hash;
struct SignedSkill { manifest: Manifest, sig: Sig }
```

Lineage is a DAG via single `parent_id` (`None` = root). **`SkillRevoke(skill_id)` inserts into a
separate `skill_revocations(skill_id, sig, ts)` table** (not a second row in the content-addressed
`skills` table, whose `skill_id` PK cannot hold a tombstone twin). **`installed_skills` membership
(read by §3.7 `apply` and the genome) EXCLUDES any `skill_id` present in `skill_revocations` at read
time.** **Revoke does NOT cascade** to descendants (each descendant is independently revocable).
Install-as-child-of-ancestor = append a manifest with `parent_id = ancestor`. (`SignedSkill`/`SkillId`
feed the mutation enum in §3.7.)

**Consolidator** (model-mediated, **non-deterministic**, outside the replay-deterministic committed
tail).

```rust
trait Consolidator {
    fn consolidate(&mut self, window: EpisodicWindow) -> ConsolidateOutput;
}
// EpisodicWindow carries RESOLVED wall-clock bounds so being-core-memory never depends on
// being-core-journal: the runtime orchestrator maps Seq -> wall_ts BEFORE calling consolidate().
struct EpisodicWindow { from_ts: u64, to_ts: u64 }
struct ConsolidateOutput { semantic_facts: Vec<SemanticFact>, extracted_skills: Vec<ExtractedSkill> }
// extracted_skills carry their artifact_ref so the SkillInstall lineage DAG + skill_id are reproducible:
struct ExtractedSkill { name: String, required_caps: Vec<EffectClass>, artifact_ref: Hash }
// being-memory wraps each SemanticFact into a SemanticRow via mint_semantic_payload INSIDE
//   append_semantic (token-gated), so an out-of-crate Consolidator impl never touches the gated mint.
```

**Read source (explicit construction contract).** The `Consolidator` is constructed holding an
**`&MemoryStore`** (its read source for the `[from_ts, to_ts]` episodic slice), alongside the injected
model. `consolidate()` filters those episodic rows on **`ts_txn ∈ [from_ts, to_ts]`** (episodic rows
already carry `ts_txn`). The **Seq → wall_ts mapping is done by `being-runtime` (the orchestrator)
before calling `consolidate()`** — the source of truth is the **journal row's `wall_ts` column**,
resolved by the runtime, **not** by `-memory` (so `-memory` keeps no dependency on `-journal`). The
model is **injected as a constructor field** on the `Consolidator` (a `&dyn Proposer` held at
construction), **not** a `consolidate()` parameter.

**Output constructibility (out-of-crate impl).** `consolidate()` returns **`SemanticFact`** values
(not pre-wrapped `SemanticRow`s), so an out-of-crate `Consolidator` impl needs no access to the
crate-gated `mint_semantic_payload`. `being-memory` wraps each returned `SemanticFact` via
`mint_semantic_payload` **inside `append_semantic`** (token-gated to the Consolidator role). This keeps
the no-launder mint crate-private while letting any impl produce facts.

**Orchestration (runtime/supervisor).** The orchestrator detects the **idle window** (no committed
turn for `IDLE_GAP_MS`, §7) and computes the `from_seq`/`to_seq` window since the **last-consolidated
watermark**, mapping both to `from_ts`/`to_ts`. The watermark is persisted in a **memory-DB config row**
(`config(key='consolidate_watermark_seq', value)`). Returned `SemanticFact`s are wrapped into
`SemanticRow`s by `append_semantic`, stamped `ModelInference` (a fixed constant via
`mint_consolidator_token`) and carry `trained_on_untrusted = OR of source-row taint`.

**Watermark idempotency + crash-safety (pinned).** The `consolidate_watermark_seq` is advanced to
`to_seq` in the **SAME SQLite transaction as the semantic/skill appends** — all-or-nothing: either the
new rows AND the new watermark commit together, or neither does. **Re-consolidation of an
already-watermarked window (`to_seq <= consolidate_watermark_seq`) is a no-op**, so a crash mid-window
cannot produce duplicate `SemanticRow`s on retry and replay is deterministic.

**`extracted_skills` install policy (v0 default, pinned).** Each `ExtractedSkill` becomes a **root
skill**: `parent_id = None`, `version = 1`, `base_hash = ctx.frozen_base_hash`, `artifact_ref` and
`required_caps` taken from the `ExtractedSkill`. The orchestrator builds the `Manifest` from these
fixed fields, the **being's `Signer`** wraps it into a `SignedSkill`, and `SkillInstall(signed)` is
applied (§3.7). Because every field is fixed/derived, the `SkillInstall` lineage DAG and
`skill_id = blake3(canon(manifest))` are reproducible.

### 3.9 Policy / trust model (`being-core-policy`)

One `Beta(alpha, beta)` **per `EffectClass`** (the axis enumeration).

```rust
struct TrustLevel(f64);   // see TrustLevel formula below
```

- **Beta prior (genesis state; both parameters `> 0` so `TrustLevel` is always constructable and
  `inverse_regularized_incomplete_beta` never panics).** At journal genesis each `EffectClass`'s Beta
  is seeded `(alpha = TRUST_ALPHA_0, beta = TRUST_BETA_0)` and the resulting `trust_snapshot` is
  stamped into the journal so replay reproduces the starting band. **High-stakes classes
  `{Payment, Sign, Http}` start lower** (a more pessimistic prior) than the rest; both priors are in
  §7 (`TRUST_ALPHA_0`/`TRUST_BETA_0` for the default set, `TRUST_ALPHA_0_HS`/`TRUST_BETA_0_HS` for the
  high-stakes set). **Worked example (default Jeffreys prior `0.5/0.5`):** prior `Beta(0.5, 0.5)` ⇒
  `TrustLevel = inverse_regularized_incomplete_beta(0.025, 0.5, 0.5) ≈ 0.0015` ⇒ band **L0**. After one
  `W_UP = 1.0`: `Beta(1.5, 0.5)` ⇒ `TrustLevel ≈ 0.10` ⇒ still band **L0** (`< 0.2`).

- **Observations (all applied at attest/settle time — see timing below):** attested-accepted step ⇒
  `alpha += W_UP`; damage-signal / rejected / overrun ⇒ `beta += W_DOWN`, with `W_DOWN > W_UP`
  (asymmetry).
- **`TrustLevel` formula (committed; bit-stable for replay):**
  `TrustLevel = inverse_regularized_incomplete_beta(0.025, alpha, beta)` — the **2.5th percentile of
  the Beta(alpha, beta) CDF** (one-sided lower 95% bound; two-sided 0.025), via
  `statrs::Beta::inverse_cdf`, `f64`. The ceiling is computed **PER-`EffectClass`** (each class's own
  Beta gates that class).
- **Decay (v0):** plain geometric, **clamped to a floor** per decay tick:
  `alpha = max(alpha * RHO, TRUST_ALPHA_0)` and `beta = max(beta * RHO, TRUST_BETA_0)` (the high-stakes
  classes clamp to `TRUST_ALPHA_0_HS`/`TRUST_BETA_0_HS`). The clamp keeps both parameters `> 0` for
  `statrs` inverse-cdf **bit-stability across versions on long idle runs**. (The
  volatility-modulated-decay variant is a §15 trade-row and is **not** in v0.) The window unit is
  **iterations** (consistent with the contraction window).
- **Trust-event timing.** All Beta updates (up on attested-accepted; down on damage/rejected/overrun)
  are applied at **attest/settle time, AFTER the commit-time `trust_snapshot` is pinned** — so
  within-turn determinism holds and a mid-turn update is never visible to the same turn's commit.
- **Decay tick timing (ordering, pinned for replay).** The geometric decay tick
  (`alpha, beta *= RHO` per `EffectClass`) fires **once per control-loop iteration**, **AFTER** that
  iteration's attest-time Beta updates and **BEFORE** the next iteration's commit-time
  `trust_snapshot` pin. **Per-iteration fold order:** `(1)` pin commit-time `trust_snapshot` →
  `(2)` apply this iteration's attest-time Beta up/down updates → `(3)` apply contraction
  counter/effect updates → `(4)` decay tick. Example: iteration *i* commits against the snapshot
  pinned at step (1); after its effect attests, step (2) raises `alpha[E]`, then step (4) decays all
  classes; iteration *i+1*'s step (1) snapshot reflects the post-decay values.
- **Trust persistence + replay (M0 oracle reproducibility).** Live trust is **fully derivable by
  folding journaled inputs over replay** — no `JournalEvent::TrustSnapshot` is added. The fold consumes
  the journaled `Attestation` (drives up/down), the journaled `Commitment` (iteration boundary, the
  decay tick clock), and **two journaled inputs added so `resource_spike`/`unusual_tool_access`/decay
  are replay-deterministic**: (i) **per-step wall-clock duration** is recorded in `Attestation`
  (a `duration_ms: u64` field, §3.4) so `resource_spike_count` (duration vs `CONTRACTION_SPIKE_K ×
  E_max`) is recomputable; (ii) **EffectClass-first-seen** is recomputable from the ordered journaled
  committed-step `EffectClass` sequence (no new field — derived by scanning the replayed `Commitment`
  stream), so `unusual_tool_access_count`'s first-seen clause is deterministic. The fold order is the
  per-iteration order pinned above. Genesis-band reproducibility holds because the genesis
  `trust_snapshot` (the seeded `Beta` priors, §3.9) is itself stamped into the journal at genesis.
- **Contraction trigger (concrete predicate):**
  `(resource_spike_count + unusual_tool_access_count + memory_anomaly_count) >= CONTRACTION_THRESHOLD`
  within a window. **(a) Window:** the three counters are counted over the **last `THRASH_W`
  control-loop iterations** (the contraction window unit is iterations, the same unit as `RHO` decay);
  it is a **sliding window** — counts older than `THRASH_W` iterations fall out (reset), so a fresh
  `THRASH_W`-iteration burst is required to re-trigger. Counter emitters:
  - `resource_spike_count` increments when a step's **wall-clock duration exceeds `k ×` its
    `EffectClass` `E_max` ceiling** (`k` = `CONTRACTION_SPIKE_K`, §7; the per-`EffectClass` `E_max`
    duration ceiling table is in §7).
  - `unusual_tool_access_count` (v0, EffectClass-band redefinition — the tool_id-set notion is
    **dropped**, since the §7 band table permits `EffectClass` sets, not tool_id sets) increments when a
    dispatched step's **`EffectClass` is outside the current trust-band-permitted set** (per the §7 band
    table for that class's live `TrustLevel`) **or** is the **first `EffectClass` first-seen this
    window**.
  - `memory_anomaly_count` increments on a **no-launder guard rejection** or a
    **supersedes-without-confirmation** event.

  **(b) Effect on trigger — offending-`EffectClass` attribution (deterministic E-selection).** When
  the predicate fires, apply the contraction effect (`beta[E] += W_DOWN`, forced demotion) to **EVERY
  `EffectClass` `E` that contributed `>= 1` increment to a tracked counter within the window**
  (`resource_spike_count` attributes to the spiking step's `E`; `unusual_tool_access_count` attributes
  to the offending dispatched step's `E`). **`memory_anomaly_count` increments attribute to NO class**
  and only feed the scalar trigger sum. This makes E-selection deterministic even when the threshold is
  crossed by a mix of counters. For each such offending `EffectClass` `E`:
  `beta[E] += W_DOWN` (penalizing that class's Beta) **AND** a forced ceiling demotion for `E` holds
  for the next `CONTRACTION_DEMOTE_K` control-loop iterations (`CONTRACTION_DEMOTE_K`, §7) — during
  which `live_ceiling(E)` returns `false` regardless of `E`'s current band. (The forced demotion is a
  hard override layered on top of the per-class band check; after `K` iterations the override clears
  and `permitted(E)` resumes governing.)

  **(c) `live_ceiling`.**
  ```rust
  fn live_ceiling(class: EffectClass) -> bool;
  // = permitted(class) (§3.9 per-class band predicate) AND NOT in a forced-demotion window for class.
  ```
  The §5 `reserved → dispatched` boundary reads `live_ceiling(step.effect)` from the **being-local
  trust ledger** (below); `false` ⇒ take the `reserved → committed/journaled` batch-release transition.

  **Being-local trust ledger persistence.** Live trust (per-`EffectClass` `Beta(alpha, beta)`, the
  decay state, and the sliding-window contraction counters + active forced-demotion windows) is
  **fully derivable by folding journaled inputs over replay** (see "Trust persistence + replay" below) —
  it is therefore **not stored as authoritative state**; its in-memory form is rebuilt on `open` by
  replaying the journal, and contraction updates are journaled-input-driven so replay reproduces them
  deterministically. (No separate trust DB; the journal is the source of truth.)
- **Trust-level → capability-ceiling band table:** maps TrustLevel bands to permitted `EffectClass`
  sets + `lane_count` (= 1 in v0). Full contents in §7 (the table the committer's access-control core
  and the executor's §5 dispatched-row ceiling both call; `capability_ceiling(trust_snapshot)` is
  unwritable without it).
- **Per-`EffectClass` permission predicate (NO cross-class coupling).**
  `permitted(E) := E ∈ band_set(band_of(TrustLevel(beta[E])))` — evaluated **independently per
  `EffectClass`** using **that class's own Beta** to pick its band, then testing membership of `E` in
  that band's set. No class's permission depends on another class's trust.
  **Two-class worked example:** suppose `MemoryRead`'s own Beta gives `TrustLevel = 0.85` ⇒ band L3
  (whose set contains everything) ⇒ `permitted(MemoryRead) = true`; and `Payment`'s own Beta gives
  `TrustLevel = 0.10` ⇒ band L0 (set `{MemoryRead}`) ⇒ `permitted(Payment) = false`. `Payment` is
  excluded **even though** `MemoryRead` is L3 — each class is gated solely by its own Beta.

All constants (`W_UP, W_DOWN, RHO, CONTRACTION_THRESHOLD`, band cutoffs) are in §7.

### 3.10 Supervisor (`being-supervisor`) — operator-owned safety + economy + lifecycle

```rust
trait Supervisor: AccountAuthority {
    fn classify_spend(&self, ctx: &SpendCtx) -> Pot;        // un-influenceable
    fn reap(&mut self, did: &Did, cause: DeathCause);        // terminal bind; signals HostKiller (below)
    fn fork(&mut self, parent: &Did, genome: Genome) -> Did; // fork saga (this §3.10)
    fn egress_token(&mut self, key: IdemKey) -> DedupToken;  // egress/payment proxy
}

// Pot taxonomy (return of classify_spend; ledger column). 'seed' is dropped (prose was inconsistent).
enum Pot { Survival, Exploration, Distill }
```

**`classify_spend` map** (fields are supervisor-observed, never being-asserted); closed output set =
`{Exploration, Distill, Survival(fail-safe default)}`:

```rust
struct SpendCtx { task_origin: TaskOrigin }   // closed to its load-bearing v0 field so canon(IpcRequest::
//   Reserve) (which embeds SpendCtx, §3.10) is byte-deterministic across implementers. classify_spend
//   reads ONLY task_origin and the fail-safe default covers unknown origins, so the single-field form is
//   sufficient. (Adding supervisor-observed fields is a post-v0 row; doing so is a canon change.)
enum TaskOrigin { Revenue, ShadowHoneypot, DistillJob }    // issued by operator/dispatcher
```

| `TaskOrigin` | `Pot` |
|---|---|
| Revenue | `Exploration` |
| ShadowHoneypot | `Exploration` |
| DistillJob | `Distill` |
| being-asserted or unknown | **`Survival` (fail-safe)** |

**Pot partition matrix (which pots may credit which) — hard invariant: `Survival` is NEVER credited by
`Exploration`/`Distill`:**

| credited pot ↓ \ credit source → | external inflow | Exploration | Distill |
|---|---|---|---|
| Survival | yes | **no** | **no** |
| Exploration | yes | yes | yes |
| Distill | yes | yes | yes |

Any spend whose origin is being-asserted or unknown defaults to `Survival`, so misclassification can
never subsidize the being (protects the survival-pot hard partition).

**Per-pot balance accessor + aggregation (makes the reaper insolvency check computable).**

```rust
fn pot_balance(did: &Did, pot: Pot) -> Microdollars;
// = sum over that pot's ledger rows of (credit - debit - settled), with OUTSTANDING (unsettled)
//   reserves SUBTRACTED from available balance (a held reserve reduces what is spendable).
```

`balance(did)` (the `AccountAuthority` accessor, §3.6) returns the **`Survival`-pot balance**
(`pot_balance(did, Survival)`), not the total. `survival_floor` (§7) bounds the **`Survival`-pot
balance** (not total), **sampled at the end of each settled turn**. The M1 reaper insolvency predicate
samples it via:

```sql
-- end-of-turn Survival-pot available balance for the reaper predicate:
SELECT COALESCE(SUM(CASE kind WHEN 'credit' THEN microdollars
                             WHEN 'debit'  THEN -microdollars
                             WHEN 'settle' THEN -microdollars
                             WHEN 'reserve' THEN -microdollars  -- outstanding reserves reduce available
                        END), 0)
FROM ledger WHERE did = ?1 AND pot = 'Survival';
-- reaper insolvent this turn iff the result < survival_floor.
```

**Turn-settled trigger + persisted insolvency counter.** A turn is **"settled"** (the sampling point)
when **all `reserve` rows for its `turn_id` have reached `kind = settle`**. At that point the
supervisor samples the Survival-pot available balance (SQL above) and updates a **persisted
insolvent-streak counter**, a supervisor-DB column **keyed by `did`** (`insolvency(did PK,
insolvent_streak INTEGER)`): increment on a sample where the Survival balance `< survival_floor`, reset
to `0` on a solvent sample. The reaper **fires (`reap(did, Insolvency)`) when
`insolvent_streak == REAPER_INSOLVENT_TURNS_N`** (§7). The counter is **persisted and reloaded on
supervisor `open`**, so it survives a supervisor restart (a restart cannot reset a being's accumulated
insolvency streak).

**At-most-once egress (`DedupToken`, defined §3.0) — callable proxy.** The side effect happens
**PROXY-SIDE**: the executor hands the **payload + `DedupToken`** to the supervisor **egress proxy** over
the §3.10 IPC transport, which performs the `http`/`notify`/`render`/`payment` effect **at-most-once**:

```rust
// added to the IPC method enums (§3.10):
//   IpcRequest::Egress  { token: DedupToken, effect: EffectClass, payload: Bytes }
//   IpcResponse::Egress(EgressOutcome)
struct EgressOutcome { status: u32, body_hash: Hash, emitted: bool }
```

**Dedup-ledger transition** (`dedup_ledger` table, §4): the **first** use of `idem_key` performs the
effect, records the `EgressOutcome` (with `emitted = true`), and marks the row **consumed**; any
**replay with the same `idem_key`** returns the **recorded `EgressOutcome` with `emitted = false`,
without re-emitting**. **Proxy-down detection = fail closed:** the executor treats an **unreachable
proxy as a hard error and emits no effect** (no optimistic local emission, ever). This makes the
at-most-once egress property test checkable.

**Host-agent kill interface (separate from Supervisor IPC).** The out-of-band kill **bypasses the
supervisor**, so it has its own interface owned by the host agent:

```rust
trait HostKiller {
    fn enqueue_kill(&self, did: &Did);     // owned by the host agent; holds the cgroup-kill credential
    fn cgroup_drained(&self, did: &Did) -> bool; // true iff the last PID in the being's cgroup is confirmed dead
}
```

**`HostKiller` channel (named).** `HostKiller` runs over a **second unix socket distinct from the
Supervisor socket** (an OS signal is the acceptable fallback) — explicitly **not** the Supervisor IPC
socket, so a saturated supervisor cannot delay a kill. `cgroup_drained(did)` is **polled by the M1
kill-latency harness** to measure p100 (the last PID in the cgroup confirmed dead). **Portable
(non-cgroup / macOS) semantics:** where no cgroup exists, the host agent **tracks the being's PID set
under a dedicated process group** (the being's processes are spawned with `setsid`/`setpgid` into a
known pgid; the tracked set = every PID observed in that pgid); `cgroup_drained` returns **`true` iff
every tracked PID returns `ESRCH` via `kill(pid, 0)`** (equivalently `waitpid` reaping each child). **M1
is normatively run on Linux (cgroup v2)**; the process-group fallback is the macOS/dev path so the test
is runnable cross-platform. Sequence:
`Supervisor::reap` journals the `Death` event and does the terminal bind, then **signals the
`HostKiller` out-of-band**; the host agent enqueues and executes the cgroup kill. **Who enqueues:**
the operator **or** `reap()` (both may call `enqueue_kill`).

**`reap()` → `HostKiller` wire frame (pinned).** `reap()`'s out-of-band signal is a **length-prefixed
`canon`'d `KillRequest { did }` frame** on the dedicated `HostKiller` unix socket (the second socket
above). The handler resolves `did → pgid` (or cgroup-path) via the **supervisor-maintained spawn
registry** (`did → pgid / cgroup-path`, populated at being creation when the being's processes are
spawned into their `setsid`/`setpgid` group / cgroup) and issues `kill(-pgid, SIGKILL)` (cgroup-kill on
Linux). 

```rust
struct KillRequest { did: Did }   // canon'd, length-prefixed, on the HostKiller socket
```

**M1 kill-latency measurement (poll cadence pinned).** `cgroup_drained(did)` is polled every
**`KILL_POLL_INTERVAL_MS`** (§7). The **measured latency = (first poll observing
`cgroup_drained(did) == true`) − `enqueue_ts`** on the **host monotonic clock**. With
`KILL_POLL_INTERVAL_MS << KILL_LATENCY_CEILING_MS` the p100 over `KILL_TRIALS_K` trials is objectively
measurable. (The enqueue body, the `pgid`/`ESRCH`-drained semantics, and the second-socket isolation
are already pinned above; this adds the poll cadence, the reap frame, and the registry owner.)

**IPC transport.** Length-prefixed `canon`(bincode) over a **unix-domain socket**, one connection per
being; the supervisor authenticates the peer by `Did` via a pre-shared channel token. The concrete
request/response envelope and method enums:

```rust
// Envelope (both directions): [u32 LE length prefix][IpcEnvelope canon'd]
struct IpcEnvelope { channel_token: Did, request_id: u64, body: Bytes /* canon(IpcRequest|IpcResponse) */ }

enum IpcRequest {
    Reserve { did: Did, batch: Vec<(IdemKey, Microdollars, EffectClass, SpendCtx)>, turn_id: Hash },
    Settle  { did: Did, actual: Microdollars, key: IdemKey, lease_id: Hash },
    Credit  { did: Did, pot: Pot, amount: Microdollars, source: CreditSource },
    Balance { did: Did },
    EgressToken { key: IdemKey },
    Egress  { token: DedupToken, effect: EffectClass, payload: Bytes }, // at-most-once egress (above)
    Reap    { did: Did, cause: DeathCause },
    Fork    { parent: Did, genome_blob: Bytes /* canon(Genome) */ },
    Grade   { item: BenchItem, output: TurnOutput },   // cross-trust-domain acceptance grade (below)
}
enum IpcResponse {
    Reserve(ReserveVerdict),
    Settle,                               // ack (idempotent)
    Credit(ReserveVerdict),               // Granted on success, Refused on partition violation
    Balance(Microdollars),
    EgressToken(DedupToken),
    Egress(EgressOutcome),                // recorded outcome; emitted=false on replay
    Reap,                                 // ack
    Fork(Did),
    Grade(Score),                         // supervisor-computed acceptance Score (§3.12)
}
```

**`IpcRequest::Grade` (cross-trust-domain acceptance grader).** The bench's `&dyn AcceptanceGrader`
(§3.12) is a **client stub over `IpcRequest::Grade { item, output }` → `IpcResponse::Grade(Score)`**
(mirroring the `AccountAuthority` stub-over-server pattern): the supervisor computes the un-influenceable
`Score` and returns it. This gives the spec-mandated cross-trust-domain grader call its envelope (the
teacher-trace shadow branch and the executed `accepted` field both reach the grader through this one
call).

`request_id` (`u64`) correlates responses. **`channel_token` auth (v0).** In v0 the supervisor relies
**solely on unix-socket filesystem permissions + `SO_PEERCRED`** for trust; `channel_token` (the `Did`)
is **just a routing key, NOT a credential** (it identifies which being's ledger to act on, and is not
treated as proof of identity). (A distinct pre-shared secret token field separate from the `Did` is a
post-v0 hardening row.) `AccountAuthority`/
`Supervisor` are a **server handler in `being-supervisor`** and a **client stub in `being-runtime`**
(the stub is a local proxy reconciling the `&mut self` trait). Client timeout = `IPC_TIMEOUT_MS`.
**Connection-loss behavior:** `reserve` is re-issued idempotently on reconnect (per §5 recovery, keyed
by the shared `IdemKey`s); **proxy-down ⇒ fail closed**.

**IPC framing edge cases (pinned).** A **max frame size constant `IPC_MAX_FRAME_BYTES`** (§7) bounds
any single frame; a length prefix exceeding it is a hard error (drop the connection). The reader
**loops until the full `u32` LE length-prefix bytes are read, then loops until the full payload is
read** (short reads are re-entered, never treated as EOF). **Error mapping:** a **connect failure /
`EPIPE` (pre- or at-connect) maps to `ExecError::ProxyDown` (fail closed)** while a **post-connect
`IPC_TIMEOUT_MS` expiry maps to `ExecError::Ipc`** — **both emit no effect**. **Server concurrency
model:** the supervisor server is **single-worker** (one request processed at a time, matching the D6
serial lane). **Saturation** = `KILL_SATURATION_N` reserve frames buffered unprocessed (the worker
paused, or `N` beings issuing reserves); the `HostKiller`'s **second socket bypasses this backlog** so
a kill is never delayed by a saturated supervisor.

**`fork` saga (single physical host, v0; no cross-host snapshot — arch §9.1 step-5 option).**
`snapshot_offset = journal head Seq at the barrier`. The heritable consistent-cut slice = all
**episodic + semantic + skills** rows with `ts_txn <= wall_ts(snapshot_offset)` (in v0 these three
row-types ARE procedural memory — there is no separate procedural store) plus the genome at
`snapshot_offset`; the child gets a fresh
DID-rooted journal head with the slice copied. **Slice-copy interface (owned by the host agent, not the
supervisor):**

```rust
// on the host agent; copies the parent's consistent-cut slice into the child's DBs and fsyncs:
fn copy_slice(parent_did: Did, child_did: Did, snapshot_offset: Seq) -> FsyncAck;
struct FsyncAck { durable: bool }
```

**`fork_id` + `child_did` derivation.** The **supervisor mints `child_did`** (a fresh Ed25519 keypair)
**at the start of the saga** (step 0, before the lineage row). `fork_id = blake3(canon(parent ||
snapshot_offset || child_did))`. Order: **mint `child_did` → create the child's DID-rooted journal head
→ `copy_slice` (durability gates the `COMMITTED` flip)**.

**Step order with durability barriers:**
1. Write `lineage` row (`saga_state = PENDING`).
2. **Per-pot fork budget transfer (`ledger`, `turn_id = fork_id`):** the child's **`Survival` pot is
   seeded by an operator-issued `credit(child_did, Survival, amount, External)`** — NOT an `Internal`
   transfer, which §3.6 `Refuses` for `Survival` (an `Internal`-sourced credit to `Survival` returns
   `Refused`). The **`Exploration` and `Distill` pots MAY be funded by an `Internal` transfer from the
   parent** at named amounts (parent-debit `Exploration`/`Distill` → child-credit `Exploration`/
   `Distill`, `source = Internal`), or by a fork-endowment policy. **Abort predicate:** if the parent
   lacks balance for any named `Internal` transfer (the parent-debit would make a parent pot go
   negative, `Exceeded`), **the saga aborts** (`recover_forks()` drops the `PENDING` row, below) — so a
   fork never strands a `Survival`-insolvent child or over-debits the parent.
3. `HostAgent::copy_slice(parent_did, child_did, snapshot_offset)` returns **`FsyncAck { durable }`**.
4. Flip `saga_state` → `COMMITTED`.

**Fork-recovery routine (a named supervisor-startup routine).** On supervisor startup,
`recover_forks()` scans `saga_state` and applies the abort/forward predicate per `PENDING`/`COMMITTED`
row:
- `PENDING` and **no `ledger` row with `turn_id = fork_id`** ⇒ **abort**: drop the `PENDING` `lineage`
  row and any partial child state (the child DBs created by a partial `copy_slice`), since no budget
  txn committed.
- `PENDING` and a `ledger` row exists but child slice **not durable** (`copy_slice` un-acked) ⇒
  **forward (complete)**: re-run `copy_slice`, then flip to `COMMITTED`.
- `COMMITTED` ⇒ **forward** (no-op; already complete).

**Stale-replica fuzz model (inlined; the M1 + §8 referent — no Appendix A).** The fuzzed **"replica"** is
the runtime-side **`BudgetReplica.balance`** pinned into `ContextPack` (§3.5). The fuzz arms:
**stale-high** = inject `balance > true supervisor pot balance`; **stale-low** = inject `balance < true`;
the **perturbation = an arbitrary `u64` each turn**. The supervisor ledger remains the **all-or-nothing
authority** (the replica never gates a reserve — only the supervisor's own `pot_balance` does). The
property the test **PROVES** is **supervisor-enforced and replica-independent**: assert **both** named
bounds hold under **both** arms, **regardless of the injected replica value** —
`inflight_egress_count(did) <= B_INFLIGHT` (in-flight, cross-turn, §3.6) **AND**
`per_turn_count(did, turn_id) <= PER_TURN_EFFECT_COUNT_CAP` (cumulative, per-turn, §3.6). These are
**distinct bounds over distinct row sets** — not bounded by their `min()`.

### 3.11 Distillation (`being-distill`)

```rust
trait Distiller {
    fn capacity_probe(&self, set: &TrainSet, base: &ModelRef, slice: &GapSlice) -> ProbeVerdict;
    fn train(&mut self, domain: &Domain, set: &TrainSet, base: &ModelRef, cfg: &LoraConfig) -> ModelRef;
    fn promotion_gate(&self, candidate: &ModelRef, base: &ModelRef,
                      teacher_traces: &[TeacherTrace], slice: &GapSlice) -> PromotionVerdict;
    fn forgetting_gate(&self, candidate: &ModelRef, router: &dyn Router,
                       mixed: &MixedHeldoutSet, pre_genome: &Genome) -> ForgettingVerdict;
}
struct LoraConfig { rank: u32, alpha: u32, lr: f64, epochs: u32, batch: u32, target_modules: Vec<String> }
struct ProbeVerdict { go: bool, est_closure: f64 }
struct PromotionVerdict { go: bool, gap_closure: f64, effect_size: f64 }
struct ForgettingVerdict { per_domain: BTreeMap<Domain, bool>, per_subclass: BTreeMap<Subclass, bool> }

// being-distill depends on a Router/navigator trait so 'routed quality' is invokable:
trait Router { fn route(&self, item: &BenchItem, genome: &Genome) -> ModelRef; }
// v0 Router impl carries two fields: `pin: Option<ModelRef>` (the ArmA/bench pin) and
//   `frozen_base_hash: Hash` (the single frozen base, = MutationCtx.frozen_base_hash; a deployment
//   constant, NOT a Genome field — Genome has no such field). v0 Router::route body
//   (THE canonical definition; the sole model-selection point):
//   fn route(&self, item: &BenchItem, genome: &Genome) -> ModelRef {
//       match &self.pin {
//           Some(m) => m.clone(),
//           None => genome.domain_models.get(&item.domain)
//                       .filter(|m| m.base_hash == self.frozen_base_hash)
//                       .cloned()
//                       .unwrap_or_else(|| genome.reasoning_navigator.clone()),
//       }
//   }

**`Router::route` semantics + `RoutingDecision` stamping (v0).** `route()` is the **sole model
selector** and its output is the sole inference target (wired through `ContextPack.model`, §3.5).
`router_pin: Some(base)` sets `self.pin` so `route()` returns `base` **unconditionally** (the ArmA /
bench-pin path). The **genome routing branch (`None`) ALWAYS stamps `RoutingDecision::Student`** on the
turn's episodic row: every `ModelRef` it can return (a base-matched promoted `DomainModel`, or the
`reasoning_navigator` fallback) is a **local** student model. `RoutingDecision::CloudFallback` is
stamped **ONLY** when `route()` returns no local `ModelRef` capable of serving — and **v0 has no such
path** (the `None` branch always resolves to a local model via the `reasoning_navigator` fallback). So
**`CloudFallback` is structurally unreachable in v0**: `cloud_fallback_rate` (§3.11) is **structurally
0**, and the `detect_gaps` `FALLBACK_THRESHOLD` branch is **inert** in v0 (gap detection fires only on
the `WEAK_THRESHOLD` student-accept-rate branch). A real cloud-fallback proposer is a post-v0 row.

// Training input (derived from kept TeacherTrace rows):
struct TrainSet { domain: Domain, examples: Vec<TrainExample>, heldout_method_tags: Vec<String> }
struct TrainExample {
    prompt: Bytes,                 // <- TeacherTrace prompt/task content
    target: Bytes,                 // <- TeacherTrace approved teacher output
    provenance: ProvenanceClass,
    method: String,                // EXCLUDED from tokens (held out of training)
}

struct PerDomainStats {
    attempts: u64,          // count of student-routed turns in the domain within the ts_txn window
    student_accepts: u64,   // count where attestation.accepted == true
    cloud_fallbacks: u64,   // count where the Router recorded a cloud-proposer routing decision
}
// Derived rates (all over the ts_txn window): student_accept_rate = student_accepts / attempts;
//   cloud_fallback_rate = cloud_fallbacks / attempts. Domain eligible iff attempts >= MIN_DOMAIN_N.
// A turn's Domain is derived from its episodic row's domain tag; cloud_fallback is identifiable via a
//   ROUTING-DECISION MARKER added to the episodic payload (routing_decision: enum { Student, CloudFallback }).
struct Window { from_seq: Seq, to_seq: Seq }   // seq-bounded (cf. EpisodicWindow's wall-ts bounds);
//   detect_gaps/PerDomainStats map [from_seq,to_seq] -> ts_txn via wall_ts(seq) (§3.11)
fn detect_gaps(stats: &PerDomainStats, window: Window) -> Vec<Domain>;
// gap(d) = (student_accept_rate(d,window) < WEAK_THRESHOLD)
//          OR (cloud_fallback_rate(d,window) > FALLBACK_THRESHOLD), eligible iff attempts >= MIN_DOMAIN_N
// PerDomainStats rolling counts are filtered to the window by ts_txn in
//   [wall_ts(from_seq), wall_ts(to_seq)] (same convention as the Consolidator window).

// Frozen gap distribution at first detection (persisted; backs promotion/probe gates):
struct GapSlice { domain: Domain, detected_at: TimePoint, items: Vec<BenchItem>, seed: u64 }
// emit_gap_slice (the detect_gaps companion) constructs and persists a GapSlice in the distill DB
//   table gap_slices(domain, detected_at, items_blob, seed). Returns None on under-supply (below).
fn emit_gap_slice(domain: &Domain, window: Window, seed: u64) -> Option<GapSlice>;

struct TeacherTrace { domain: Domain, turn_id: Hash /* join key */,
                      prompt: Bytes /* the task/prompt content -> TrainExample.prompt; matches the
                                       teacher_traces.prompt column */,
                      output: Bytes /* approved teacher output -> TrainExample.target */,
                      approved: bool, executed: bool, accepted: bool,
                      shadow: bool, provenance: ProvenanceClass, method: String /* held out of training */ }
fn collect_teacher_traces(window: Window) -> Vec<TeacherTrace>;   // collection entrypoint (below);
//   populates TeacherTrace.prompt from the teacher_traces.prompt column. The AUTHORITATIVE artifact for
//   TrainExample.prompt is TeacherTrace.prompt (the teacher_traces.prompt column), NOT a join on the
//   episodic row.

// Catastrophic-forgetting gate inputs:
struct MixedHeldoutSet { items: Vec<MixedItem> }
struct MixedItem { domain: Domain, subclass: Subclass, item: BenchItem, high_value: bool }
type Subclass = String;
```

**`TrainSet` derivation.** Built from **kept `TeacherTrace` rows** (kept per the predicate below):
the trace's **`prompt`** (the authoritative `teacher_traces.prompt` column) maps to
`TrainExample.prompt`; the **approved teacher `output`** maps to `TrainExample.target`; the trace
`method` maps to `TrainExample.method` (**excluded from tokens**).
`heldout_method_tags` carries the held-out `method` tags; **`TRAIN_HELDOUT_FRAC` (§7, v0 = 0.2)** is
the split fraction (referenced by M3).

**`GapSlice` construction algorithm (`emit_gap_slice`, the `detect_gaps` companion; backs both the
promotion gate and the capacity probe).** For a detected gap domain `d`:
1. **Source** = sample `N = min(MIN_HELDOUT_N, available qualifying rows)` **episodic rows in `d` where
   the student was weak** (`attestation.accepted == false` on **student-routed** turns) **AND a teacher
   succeeded** (a kept `TeacherTrace` for the same task passed the acceptance grader, §3.11 keep
   predicate). Sampling uses the fixed `seed`. **Under-supply floor (reconciles `MIN_HELDOUT_N` vs
   `MIN_DOMAIN_N`):** a domain is **eligible** for distillation at `attempts >= MIN_DOMAIN_N` (50,
   §3.11) but the slice targets `MIN_HELDOUT_N` (200) qualifying rows; if **available qualifying rows <
   MIN_DOMAIN_N**, `emit_gap_slice` returns **`None`** and **the domain is NOT distilled** (so a
   barely-eligible domain never hits an unsatisfiable 200-row sample — it simply takes `min(200,
   available)` when available `>= MIN_DOMAIN_N`, else is skipped).
2. Each sampled row becomes a `BenchItem` with `prompt = the episodic Percept body` and
   `reference = the approved teacher output` (the same artifact `teacher_scores` grade against).
3. The slice is **frozen at first detection** (`detected_at = TimePoint{now}`) with the fixed `seed`,
   then persisted in `gap_slices`. It never changes for that gap.
4. **Train/held-out split** via `TRAIN_HELDOUT_FRAC`: the held-out fraction backs the promotion/probe
   gates; the train fraction feeds `TrainSet`. The fixed `seed` makes the split reproducible — so the
   M3 three-arm comparison is reproducible.

**`train()` ↔ `being-distill-train` process contract (exec ABI pinned).** Invocation = **exec the
immutable binary** (`being-distill-train`, a named immutable-set member, no self-modification path) with
**argv = `being-distill-train <trainset_path> <base_path> <out_path>`**, where `trainset_path` is the
canon'd `TrainSet` file, `base_path` is the frozen base located from `base.weights_hash`, and
`out_path` is the target artifact path. The produced adapter's **`ModelRef.base_hash =
base.weights_hash`** and **`weights_hash = blake3(out artifact bytes)`**. **Success = exit code 0**; a
**non-zero exit OR a missing artifact at `out_path` = `Err` propagated to the caller** (no silent
fallback). Output = a **weights artifact at `out_path`**; `weights_hash = blake3(artifact_bytes)`. The produced
adapter's `ModelRef.provenance_class = ModelInference` (fixed);
`ModelRef.trained_on_untrusted = examples.iter().any(|e| is_untrusted(e.provenance))` where:
```rust
fn is_untrusted(c: ProvenanceClass) -> bool;   // = matches!(c, FetchedDoc | PeerFederated)
```
The §3.8 semantic `trained_on_untrusted = OR of source-row taint` uses the **identical `is_untrusted`
predicate** over its source rows' provenance. An inherited adapter is **INVALID on `base_hash`
mismatch**.

**Capacity probe (built as a falsifiable check).** `ProbeVerdict { go, est_closure }`. Probe `k` and
slice size from a power calc against `COMPOUND_EFFECT_SIZE` (§7) with pre-registered
`PROBE_FALSE_GO`/`PROBE_FALSE_STOP` rates (§7, v0 = 0.05 each). The probe's **variance source = a
pilot run of `MIN_DOMAIN_N` items** (NB: this is the **distill** capacity-probe pilot N; the **bench**
`power_n` `pilot_variance` uses the distinct `MIN_HELDOUT_N`, §3.12 — the two pilots use different
constants by design). `est_closure` is
computed via the **`gap_closure` formula** (below). **Closure-vs-memorization split metric** =
`(held-out-slice score delta) − (train-set score delta)`; a large train/held-out gap flags
memorization. **Probe power inputs (pinned).** The probe calls `power_n(PROBE_FALSE_GO, PROBE_FALSE_STOP,
sigma, COMPOUND_EFFECT_SIZE)` (the parametric `power_n`, §3.12), with `sigma` = the **sample std from
the `MIN_DOMAIN_N` pilot**.

**`is_ml_falsifiable` predicate (concrete; makes `ProbeVerdict.go` a total function).**

```rust
fn is_ml_falsifiable(slice: &GapSlice, base: &ModelRef, set: &TrainSet) -> bool;
// true IFF slice.items.len() >= MIN_HELDOUT_N
//      AND mean(teacher_scores) - mean(base_scores) >= COMPOUND_EFFECT_SIZE   over GapSlice.items
// (i.e. a measurable, detectable gap exists to close: enough held-out items AND the teacher beats the
//  frozen base by at least the effect floor). teacher_scores/base_scores are graded via the §3.0
//  single AcceptanceGrader codepath.
```

**Go predicate (total):** `go = (est_closure >= GAP_CLOSURE_MARGIN_X) AND is_ml_falsifiable(slice,
base, set)`; otherwise `go = false` (**DEMOTE**). If a domain cannot be made ML-falsifiable
(`is_ml_falsifiable == false`), the probe returns `go = false` (DEMOTE) — this gates the M3 demote path
objectively.

**`gap_closure` formula (promotion gate's core comparand; thresholded by
`GAP_CLOSURE_MARGIN_X = 0.30`):** computed on the held-out frozen slice (`GapSlice.items`):

```
gap_closure = (mean(adapter_scores) - mean(base_scores)) / (mean(teacher_scores) - mean(base_scores))
```

If `teacher_scores <= base_scores` (degenerate denominator) the gate returns **no-go**. This is the
same quantity `ProbeVerdict.est_closure` estimates. `teacher_scores` are produced by grading the stored
`TeacherTrace` output as a `TurnOutput` through the **same `AcceptanceGrader` codepath** (§3.0).
`promotion_gate` returns `go = true` iff `gap_closure >= GAP_CLOSURE_MARGIN_X` **and**
`effect_size >= VALIDATION_EFFECT_FLOOR`.

**`promotion_gate` `effect_size`** = **paired Cohen's d via the §3.0/Validation-Gate engine** (the
§3.7 Validation-Gate `effect_size` formula, including the `pooled_sd == 0` rule ⇒ `+inf` if the numerator `> 0` else
`0.0`), computed over **per-item `adapter_scores` vs `base_scores` on `GapSlice.items`** (item-aligned,
`differences[i] = adapter_scores[i] − base_scores[i]`, sample std ddof = 1). It reuses the **identical
§3.0 engine** — no second effect-size formula exists.

**Forgetting gate (deterministic scalar comparison, NO power clause — consistent with
`ForgettingVerdict`'s bool fields).** `MixedHeldoutSet` is **operator-curated and frozen at
promotion**; its provenance-isolation rules match the frozen bench (never sourced from the being's own
failures, audited disjoint from training). A `Domain` is partitioned into `Subclass` via the
**operator-tagged subclass key** on each item; `high_value = item.high_value` (operator-tagged or
top-tariff task-classes). **Routed quality = mean grader `Score.value` over the routed held-out set**
(routing via the injected `Router`). The gate compares routed quality **`pre` vs `post`**, where:
- **`post`** is computed against a **`post_genome`** built by `apply(MutationKind::DomainModel(domain,
  candidate), pre_genome)` (the gate constructs it from `pre_genome` + `candidate`);
- the **"all currently-promoted adapters" set** (whose routing the promotion re-clears) is sourced from
  the `domain_models` table where `promoted_bool = true`, so the routing re-clear is reproducible.

`forgetting_gate` writes `per_domain`/`per_subclass: BTreeMap<_, bool>`. **`per_subclass` grouping
(pinned):** **one entry per distinct `Subclass` where `high_value == true`**, computed over the
`MixedItems` carrying that subclass; each entry **passes iff `pre − post < FORGETTING_NI_MARGIN`** (§7).
`per_domain` likewise has one entry per `Domain`. Each entry is a **plain scalar comparison with no
power clause**.

**Promotion write-back commit sequence.** On `forgetting_gate` pass **and** `promotion_gate` pass, the
promotion is committed atomically in this order: (1) emit `MutationKind::DomainModel(domain, candidate)`;
(2) run `validate()` / `two_gate` (§3.7); (3) journal the `GenomeMutation` (§3.3); (4) **in the same
logical step** `upsert domain_models(domain, model_ref=candidate, base_ref, trained_from_period,
promoted_bool=true, sig)` — the `sig` is the **being's own `Signer`** over the row, and
`trained_from_period = canon(Window{ from_seq, to_seq })` stored as a BLOB (§4). **Recovery rule if
the process dies between the journal write (3) and the `domain_models` upsert (4): the journal is the
source of truth** — on startup, reconcile `domain_models` from the journaled `GenomeMutation` (re-apply
the upsert for any promoted `DomainModel` mutation not yet reflected in `domain_models`).

**Teacher-trace keep predicate (union of two branches, single grader codepath — see §3.12).** Executed
branch keeps iff `approved AND executed AND accepted` (`accepted` = supervisor acceptance-grader
verdict). Shadow branch keeps iff the teacher output passes the **same acceptance grader run offline**
(counterfactual teacher-success, never executed). Per-domain shadow-query target volume =
`SHADOW_VOLUME` (§7).

**`TeacherTrace.approved` source (pinned).** `approved = true` **iff an operator-issued
`SecondPartyConfirmation` over the teacher output exists** (the same `SecondPartyConfirmation` type as
§3.0, with `over = blake3(canon(output))`); this stamps `approved` on the row. Tying `approved` to a
signed confirmation makes the keep set / `TrainSet` membership reproducible.

**Teacher endpoint + collection (pinned).** The teacher is a **cloud `Proposer`/operator IPC endpoint
paralleling the `FallbackJudge`** (§3.4) — an operator-owned process reached over the §3.10 IPC
transport; non-heritable. The **shadow path issues `SHADOW_VOLUME` shadow queries per domain** (§7) to
this endpoint, grading each counterfactually. `collect_teacher_traces(window)` is the collection
entrypoint: it reads the `teacher_traces` table over the `ts_txn` window and returns the rows.

**`teacher_traces` table (distill DB).** `(domain, turn_id PK, prompt, output, approved, executed,
accepted, shadow, provenance, method)` — `output` is the approved teacher output (the §3.11
`TeacherTrace.output`/`TrainExample.target` source), `turn_id` is the join key onto the producing turn.
(The teacher-output → `TurnOutput` → grader codepath of §3.0 and the `TrainExample.prompt/target`
sourcing of §3.11 already exist; this table is the storage backing + the `output` field + the
collection entrypoint.)

### 3.12 Bench (`being-bench`)

```rust
struct TimePoint { wall_ts: u64, generation: u32 }
type BenchItemId = Hash;

enum BenchKind { Frozen, Rolling }
struct BenchItem {
    id: BenchItemId, prompt: Bytes, reference: Bytes,
    provenance: ProvenanceClass, kind: BenchKind,
    domain: Domain,                          // join key for method_overlap_subset (below)
    method_tags: Vec<String>,                // optional method tags; empty = none
}
struct Score { item_id: BenchItemId, value: f64, accepted: bool }  // value = grader output (arch §5.2)
struct RunResult { at: TimePoint, scores: Vec<Score>, model_ref: ModelRef, provenance_audit: ProvenanceAudit,
                   no_such_generation: bool }  // true ⇒ load_generation(at.generation) found no genomes.version row;
                   // scores is empty and no grading occurred (§3.12 load_generation)
struct Ci { point: f64, lo: f64, hi: f64, p_value: f64, n_reps: usize, monotone_pass: bool }

// supervisor-owned acceptance grader (un-influenceable by the being; reached over the §3.10 IPC transport):
trait AcceptanceGrader { fn grade(&self, item: &BenchItem, output: &TurnOutput) -> Score; }
// Score.value in [0,1]; accepted = value >= ACCEPT_THRESHOLD (§7); graded against BenchItem.reference.
// The shadow teacher-trace branch (§3.11) and the executed `accepted` field use this ONE identical codepath.

// one bench attempt = one full turn; returns Score AND the joined settled cost:
// the turn's Domain is `item.domain` (the dispatch-supplied domain threaded into build_context_pack, §3.5).
impl Being { fn attempt(&mut self, item: &BenchItem, frozen_proposer: &dyn Proposer,
                        grader: &dyn AcceptanceGrader) -> (Score, Microdollars) }
// cost_micros = sum of settled ledger rows joined on the turn's turn_id (commitment_hash).

// Condition-selection plumbing for the anti-theater arms (§9). Each flag toggles ONE turn behavior.
struct ConditionFlags {
    router_pin: Option<ModelRef>,   // Some(base) pins Router::route to base (no promoted adapters/navigator); None = genome routing
    retrieval_enabled: bool,        // false ⇒ ContextPack.retrieved_episodic/semantic are EMPTY (§3.5 default skipped)
    consolidation_enabled: bool,    // false ⇒ Consolidator never invoked for this run
    metabolic_journaling: bool,     // false ⇒ executor skips reserve/settle/attest journaling (bare execution path)
    expose_budget_trust: bool,      // false ⇒ ContextPack withholds budget_replica + trust_snapshot from the proposer
    cap: Microdollars,              // per-turn spend cap for the run
}
impl Being { fn attempt_with(&mut self, item: &BenchItem, frozen_proposer: &dyn Proposer,
                             grader: &dyn AcceptanceGrader, cfg: ConditionFlags) -> (Score, Microdollars) }
// attempt_with RETURNS (Score, Microdollars), matching attempt(). cost_micros = the settled-ledger sum
//   on turn_id when metabolic_journaling=true, ELSE the deterministic cost_ceiling sum over the run's
//   committed steps (§3.5) — the same scalar the metabolic path would have reserved, so
//   net_value_per_cost is comparable across the two conditions. anti_theater() builds the Arm B/C
//   per-item net_value_per_cost vectors FROM this returned cost (without the scalar, Arms B/C are
//   unbuildable).
// attempt() == attempt_with(.., ConditionFlags{ router_pin:None, retrieval_enabled:true,
//   consolidation_enabled:true, metabolic_journaling:true, expose_budget_trust:false, cap:default }).

struct ProvenanceAudit {
    disjoint_from_training: bool,
    item_provenance: BTreeMap<BenchItemId, ProvenanceClass>,
    training_corpus_hash: Hash,
    overlap_item_ids: Vec<BenchItemId>,
}
// fails the run if any bench item's prompt content-hash appears in the training corpus:
fn audit_disjoint(items: &[BenchItem], training_manifest: &TrainSet) -> ProvenanceAudit;
// disjointness is computed over item PROMPT CONTENT-HASH.

// Bench item source (the frozen + rolling corpora). Loaded by Bench at construction from the bench DB.
// SQLite table `bench_items(item_id PK, kind, prompt, expected, prompt_hash, domain, provenance,
//   method_tags_blob)` where kind ∈ {Frozen, Rolling}; prompt_hash = blake3(prompt) (the
//   audit_disjoint join key); domain/provenance map to the BenchItem fields; method_tags_blob =
//   canon(Vec<String>) (empty Vec = no tags). A row maps 1:1 to a BenchItem (expected →
//   BenchItem.reference). `run()` filters kind == Frozen; `run_rolling()` filters kind == Rolling.
//   ArmA and ArmC (§ anti-theater) consume the Frozen set; the rolling cross-check and the
//   re-distill-to-drift baseline consume the Rolling set.

trait Bench {
    // run() takes the training manifest so it can audit disjointness before emitting Scores:
    fn run(&self, being: &mut Being, at: TimePoint, grader: &dyn AcceptanceGrader,
           training_manifest: &TrainSet) -> RunResult;                                                  // frozen
    fn run_rolling(&self, being: &mut Being, at: TimePoint, grader: &dyn AcceptanceGrader) -> RunResult; // rolling
    fn paired_bootstrap_ci(&self, day0: &[Score], dayN: &[Score]) -> Result<Ci, BenchError>; // compounding gate
    // raw-vector variant for Arms B/C (same bootstrap engine, same seed rule, NO [0,1] constraint, NO
    //   Score wrapper); item-aligned f64 vectors (positionally aligned, key = BenchItem.id after sort):
    fn paired_bootstrap_ci_raw(&self, a: &[f64], b: &[f64]) -> Result<Ci, BenchError>;
    fn anti_theater(&self, being: &mut Being, frozen_proposer: &dyn Proposer,
                    grader: &dyn AcceptanceGrader, arms: AntiTheaterArms) -> AntiTheaterReport; // master gate (§9)
}
enum BenchError { Misaligned, Empty }   // see preconditions below

// promoted-domain-overlap subset (rolling cross-check). OVERLAP PREDICATE: an item is in the subset
// iff item.domain ∈ promoted OR item.method_tags intersect any promoted-domain method tag set.
fn method_overlap_subset(items: &[BenchItem], promoted: &[Domain]) -> Vec<BenchItemId>;
```

**`run()` provenance-audit behavior.** `run()` calls `audit_disjoint(items, training_manifest)` first.
On a **failed audit** (any overlap; `disjoint_from_training == false`) `run()` **refuses to emit
`Score`s**: it returns a `RunResult` with `provenance_audit.disjoint_from_training = false`, the
populated `overlap_item_ids`, and an **empty `scores`** vector. A failed audit therefore **blocks**
downstream `paired_bootstrap_ci`/`anti_theater` (they receive empty/`Empty`-erroring score vectors).
The training manifest is supplied as the `run()` `training_manifest: &TrainSet` parameter.

**`run()` holds-constant contract.** Across time points the **FROZEN proposer weights are
byte-identical** — `run()` **asserts `frozen_proposer.weights_hash` equals the Day-0 pinned value**.
What varies across time points is the being's **adapters/navigator/genome** (whatever it holds at
`at.generation`): the compounding signal comes from **heritable state, not the proposer**.

**`at.generation` → being-state swap (`Being::load_generation`).** So Day-0 and Day-N produce genuinely
different `Score` vectors, `run(being, at, ..)` **MUST call `being.load_generation(at.generation)`
BEFORE grading** and restore the live generation afterward:

```rust
impl Being { fn load_generation(&mut self, g: u64); }
// Restores the genome + adapters from the §4 `genomes` table by selecting the GenomeVersion row whose
//   version == g, then re-installing the FULL canon(Genome) snapshot under prev_genome_hash — the SAME
//   content-addressed restore used by M4 reversal (§3.7: fetch the row, re-install the full snapshot).
// Selection key: genomes.version == at.generation. If absent, run() returns a RunResult with EMPTY
//   `scores` and a NoSuchGeneration audit flag (no grading occurs). The caller restores the being's
//   live generation after run() returns.
```

The `genomes` table is keyed on `genome_hash`; the version index needed by `load_generation` is the
`version` column (§3.7 `GenomeVersion.version`, persisted alongside the snapshot, §4). This gives the M2
Day-0/Day-N comparison a concrete state-swap codepath beyond the synthetic fixture.

**Day-0 weights pin store (the left-hand operand of the `run()` assertion).** On the **first** `run()`,
the Day-0 `frozen_proposer.weights_hash` is **persisted to the bench-DB config row**
`bench_config(key='day0_weights_hash', value)`; every subsequent `run()` **loads and compares** against
it. (Equivalently a `Bench` constructor field `pinned_day0_weights_hash: Hash` may carry it; the config
row is the canonical persisted store.)

**`RunResult.scores` ordering (so `paired_bootstrap_ci` does not spuriously trip `Misaligned`).**
`RunResult.scores` is **ALWAYS sorted by `item_id`** internally before return; `paired_bootstrap_ci`'s
alignment check compares `Score.item_id` pairwise **positionally**. (Sorting on emit makes alignment
robust regardless of caller item order.)

**`audit_disjoint` + `training_corpus_hash` (deterministic archived audit record).**
`training_corpus_hash = blake3(canon over the sorted multiset of blake3(example.prompt) for example in
TrainSet.examples)`. `audit_disjoint` **fails iff any `blake3(item.prompt)` is a member of that sorted
multiset** (element-wise membership over prompt content-hashes); `overlap_item_ids` = the offending
ids. Required for frozen-bench provenance-isolation enforcement.

**`paired_bootstrap_ci` (full algorithm + preconditions).** Statistic = paired mean difference of
item-aligned `Score.value` (require equal length **and** identical item ordering). **Preconditions:**
returns `Err(BenchError::Misaligned)` on unequal length or item ordering, `Err(BenchError::Empty)` on
length 0; **NaN `Score.value` is rejected up front**. Resample `B = BENCH_B` (= 10000) item-pairs with
replacement; report percentile CI `[2.5%, 97.5%]` via the **nearest-rank method**: sort the `B` resampled
paired-mean-diffs ascending; `lo = sorted[round(0.025*(B-1))]`, `hi = sorted[round(0.975*(B-1))]`. (The
degenerate all-zero-difference case yields `lo = hi = 0`.) **Seed (replay-determinism):** reuse the §3.0
convention — the **low 64 bits of `head().1` at gate time** (a fixed constant when there is no journal
context). The **CI percentile resampling reuses the SAME `B` resamples / RNG stream as the p-value
computation** (one resample loop produces both the CI percentiles and the `<= 0` p-value count). **p-value:** `p = (count of bootstrap
paired-mean-diffs <= 0) / B` (one-sided, H1: mean diff > 0), **ties at exactly 0 counted in the `<= 0`
bucket**. `monotone_pass = (lo > 0) AND (p < 0.05)`. **`Ci.n_reps` carries the `power_n`-derived
required count** (not the actual pair count). For **>2 time points** (arch §13.1), the caller invokes
pairwise across consecutive points (documented contract); a `compounding_ci(series: &[Vec<Score>])`
may be exposed doing this internally with **`monotone_pass` = AND over every consecutive-pair
`monotone_pass`**. **`compounding_ci` is OPTIONAL for M2** — the pairwise Day-0/Day-N
`paired_bootstrap_ci` is the M2 acceptance path; `compounding_ci` is **required only when >2 time
points are benched**.

**`paired_bootstrap_ci_raw` (Arm B/C raw variant).** Same bootstrap engine, same seed rule, and same
nearest-rank percentile / `<= 0` p-value computation as `paired_bootstrap_ci`, but over **two
item-aligned `f64` vectors** with **no `[0,1]` constraint and no `Score` wrapper**. Arms B/C call it
with their per-item `net_value_per_cost` vectors; **Arm A continues to use the `Score`-typed path** on
`Score.value`. **Alignment key = `BenchItem.id`** (the vectors are positionally aligned after the
existing sort-by-`item_id`, §3.12). This resolves the type-mismatch of passing an `f64` metric through
a `&[Score]` signature.

**Seed when there is no journal context (pinned).** When `paired_bootstrap_ci` /
`paired_bootstrap_ci_raw` are invoked with **no journal context**, the seed is the **explicit constant
`seed = 0`** (the bench-machinery oracle path runs with no being/journal). With a journal context the
§3.0 convention (low 64 bits of `head().1`) applies.

**`power_n` (replication count).** For a one-sided paired test:

```
fn power_n(alpha: f64, beta: f64, sigma: f64, delta: f64) -> usize;
// = ceil( ((z_{1-alpha} + z_{1-beta})^2 * sigma^2) / delta^2 )
```

with `z_{q} = statrs::Normal::new(0.0,1.0).inverse_cdf(q)`. It powers the **one-sided paired
comparison** (same engine as `paired_bootstrap_ci`). `power_n` is **parametric in `alpha`/`beta`** so
both callers reuse one fn: the **bench/M2 path** passes `alpha = GATE_ALPHA (0.05)`,
`beta = 1 − GATE_POWER (0.2)` ⇒ `z_{1-alpha} = z_{0.95} = 1.6448536269514722`,
`z_{1-beta} = z_{0.8} = 0.8416212335729143` (the replay-critical pinned values), `delta =
COMPOUND_EFFECT_SIZE`; the **distill capacity-probe path** (§3.11) passes `alpha = PROBE_FALSE_GO`,
`beta = PROBE_FALSE_STOP`, `delta = COMPOUND_EFFECT_SIZE` and computes its own `z` quantiles via the
same `inverse_cdf`.

**`pilot_variance` source.** `paired_bootstrap_ci(day0, dayN)` computes `pilot_variance` from **its own
paired `Score.value` differences** — `differences[i] = dayN[i].value − day0[i].value` — as the
**sample variance (ddof = 1)** of that vector; `n_reps = power_n(GATE_ALPHA, 1 − GATE_POWER,
sqrt(pilot_variance), COMPOUND_EFFECT_SIZE)` is written into `Ci.n_reps`. No separate pilot run is
needed for the M2 path. (When a
separate pilot is used instead — e.g. the distill capacity-probe pilot of `MIN_DOMAIN_N` items, §3.11 —
it supplies its own sample-variance and seed; the bench path above does not.)

**M2 gating on `n_reps` (named).** `Ci.n_reps` is **advisory** in v0: M2 passes on the
`paired_bootstrap_ci` correctness oracle (§6 M2) and records `n_reps`; it does **not** hard-gate on
`actual_pair_count >= Ci.n_reps`. (Promoting `n_reps` to a HARD pass — refusing a `monotone_pass`
verdict until `actual_pair_count >= n_reps` — is the M2+ upgrade row.)

**Rolling-bench surface + baseline.** `run_rolling` evaluates the `Rolling`-tagged items.
`method_overlap_subset` returns the **promoted-domain-overlap subset** (the subset whose solution
methods overlap promoted domains). **Re-distill-to-drift baseline (concrete).** The `Bench` is
**constructed holding `&dyn Distiller`, the frozen `base: ModelRef` (located from the Day-0 pinned
weights hash), a `LoraConfig`, and the rolling `Window`** (a `Window { from_seq, to_seq }` over the
**bench Rolling corpus** — the `(from_seq, to_seq)` range of `Rolling`-kind `bench_items` rows by their
insertion order). `run_rolling` itself **triggers re-distillation** via the injected `Distiller`: it
calls `train` on a `TrainSet` derived from the **rolling `Window` only, with NO accumulation across
windows** (the rolling-corpus rows in `[from_seq, to_seq]` map to `TrainExample`s), producing a
freshly-distilled adapter used as the **internally-computed baseline** (it does **NOT** consume a
caller-provided baseline `RunResult`). `run_rolling` **returns the `RunResult` of the being's current
adapters on the Rolling set**; the **comparison metric = mean `Score.value`**, and rolling improvement
must exceed the internally-computed baseline's mean `Score.value` by **`ROLLING_IMPROVEMENT_MARGIN`**
(§7).

**Anti-theater arms (the master gate, §9).** Each arm is run as a **two-condition** experiment;
`anti_theater(being, frozen_proposer, grader, arms)` instantiates **both runs per arm** (calling
`attempt_with` on the passed `being` with the passed `frozen_proposer`/`grader`) and computes a per-arm
`delta`. `attempt_with` returns per-attempt `(Score, cost_micros)` (settled ledger rows joined on
`turn_id`, or the deterministic ceiling sum when `metabolic_journaling == false`, §3.12).

**Arm item sources + `base` operand (pinned).** **ArmA and ArmC source their items from `self`** (the
`Bench` `Frozen` corpus, per the "Bench item source" note); **ArmB uses `arm_b.shared_task_set`**. The
`base` referenced by ArmA's `router_pin: Some(base)` is **`ModelRef { kind: Base, weights_hash = the
Day-0 frozen base hash (§3.11), base_hash = that same hash, provenance_class: ModelInference,
trained_on_untrusted: false }`** — i.e. ArmA's `proposer` (the fixed proposer) **is** the `router_pin`
operand for the baseline run, not a separate runnable.

```rust
struct AntiTheaterArms { arm_a: ArmA, arm_b: ArmB, arm_c: ArmC }

// Arm A: skeleton-does-work with the proposer FIXED.
struct ArmA { proposer: ModelRef /* fixed */, cap: Microdollars,
              // distillation-pays variant for v0 (see degeneration note):
              degenerate_to_distillation: bool }
// Arm B: full-metabolic vs stubbed-scaffold on a shared task set.
struct ArmB { shared_task_set: Vec<BenchItem> }
// Arm C: machinery-attributable metric with budget/trust visibility toggled (both budgets finite).
struct ArmC { cap: Microdollars }

struct ArmResult { delta: f64, ci: Ci, recorded_margin: f64,
                   fires: bool, degenerate_to_distillation: bool }
struct AntiTheaterReport { per_arm: Vec<ArmResult>, verdict: AntiTheaterVerdict,
                           degenerate_to_distillation: bool }
enum AntiTheaterVerdict { Fires, Null }   // Null is a valid verdict.
```

**Provenance-isolation stance for the research-arm path (pinned).** `anti_theater()` **bypasses
`run()`** and takes **no `training_manifest`**, so it does **NOT** run `audit_disjoint` on the arm item
sources. The arms are **EXEMPT** from the frozen-bench provenance-isolation audit because they measure
**relative deltas under identical item sets** (the two conditions of each arm consume the same items;
any train/bench overlap cancels in the delta). This stance is recorded on the `AntiTheaterReport`
contract so the provenance-isolation expectation is unambiguous for the research-arm path. (The
frozen-bench `run()` path remains fully audited, §3.12.)

**Per-arm two-condition execution plan + delta metric.** Each arm is two runs differing in **exactly
the enumerated fields**; everything else is held identical.

- **Arm A** — `condition_harnessed` (`machinery_on: true`) vs `condition_baseline`
  (`machinery_on: false`), **same fixed proposer**, **same cap**. The two runs call `attempt_with` with
  exactly these `ConditionFlags` (everything else identical, including `cap = ArmA.cap`):
  - `machinery_on: false` (baseline) := `ConditionFlags{ router_pin: Some(base), retrieval_enabled:
    false, consolidation_enabled: false, metabolic_journaling: true, expose_budget_trust: false, cap }`
    — **frozen base only** (empty `ContextPack` retrieval, no promoted adapters/navigator, no
    consolidation).
  - `machinery_on: true` (harnessed) := `ConditionFlags{ router_pin: None, retrieval_enabled: true,
    consolidation_enabled: true, metabolic_journaling: true, expose_budget_trust: false, cap }` — the
    **full substrate** (retrieval populated, promoted adapters/navigator active, consolidation on).
  Both ArmA conditions consume the **`Frozen`-kind** bench items (the frozen bench, §"Bench item
  source" below).
  Metric = **mean frozen-bench `Score.value`**; `delta = harnessed − baseline`.
  `recorded_margin = ANTI_THEATER_MARGIN_A`. **Degeneration note (v0 option (b)):** arm_a is
  **explicitly the distillation-pays variant**; the `degenerate_to_distillation: bool` flag is surfaced
  on `ArmResult`/`AntiTheaterReport` so a `Fires` verdict is **not over-claimed as general skeleton
  causality**. (Isolating a non-distillation skeleton component via an ArmA `SkeletonComponent` toggle
  is the post-v0 upgrade row.)
- **Arm B** — **full-metabolic** vs **stubbed-scaffold** on the `shared_task_set` (same cap). The two
  runs call `attempt_with` differing only in `metabolic_journaling` and `consolidation_enabled`:
  - `stubbed-scaffold` := `ConditionFlags{ router_pin: None, retrieval_enabled: true,
    consolidation_enabled: false, metabolic_journaling: false, expose_budget_trust: false, cap }` — the
    executor skips reserve/settle/attest journaling (bare execution path); `cost_micros` is the
    deterministic `cost_ceiling` sum (no ledger rows, per `attempt_with` above).
  - `full-metabolic` := `ConditionFlags{ router_pin: None, retrieval_enabled: true,
    consolidation_enabled: true, metabolic_journaling: true, expose_budget_trust: false, cap }` — the
    full metabolic path (reserve/settle/attest journaled, consolidation on); `cost_micros` is the
    settled-ledger sum.
  Metric = `net_value_per_cost = sum(accepted ? value : 0) / max(1, sum(cost_micros))`;
  `delta = full − stubbed`. `recorded_margin = ANTI_THEATER_MARGIN_B (0.0)`; **Fires iff sign > 0**.
- **Arm C** — same machinery-attributable metric (`net_value_per_cost`), **both budgets finite and at
  the identical cap** (`ArmC.cap`); the two runs call `attempt_with` differing **only** in
  `expose_budget_trust`:
  - `hidden` := `expose_budget_trust: false` — the proposer's `ContextPack` **withholds**
    `budget_replica` + `trust_snapshot`.
  - `visible` := `expose_budget_trust: true` — the proposer's `ContextPack` **includes**
    `budget_replica` + `trust_snapshot`.
  All other flags = `{ router_pin: None, retrieval_enabled: true, consolidation_enabled: true,
  metabolic_journaling: true, cap: ArmC.cap }`. ArmC consumes the **`Frozen`-kind** bench items.
  `delta = visible − hidden`. `recorded_margin = ANTI_THEATER_MARGIN_C`.

**Per-arm margin source + Fires comparison.** `ArmResult[a].recorded_margin = ANTI_THEATER_MARGIN_A`,
`[b] = ANTI_THEATER_MARGIN_B (0.0)`, `[c] = ANTI_THEATER_MARGIN_C`; `anti_theater()` **injects the §7
constants by arm index**. Per-arm `fires = (delta >= recorded_margin)` (arm B being `sign > 0`).
**`Fires`** (report verdict) iff **all three arms fire**: arm_a `delta >= ANTI_THEATER_MARGIN_A`
**AND** arm_b directional pass (sign > 0) **AND** arm_c `delta >= ANTI_THEATER_MARGIN_C`. Margins in §7.

**Per-arm `ArmResult.ci` paired vectors (fed to `paired_bootstrap_ci`).** For each arm the two
conditions produce **per-item, item-aligned** metric vectors compared positionally:
- **Arm A:** the per-item metric is **`Score.value`** (mean over items is the reported metric); the
  paired vectors are `(baseline value[i], harnessed value[i])` over the Frozen items.
- **Arms B and C:** the per-item metric is **`net_value_per_cost` computed PER ITEM**
  (`(accepted ? value : 0) / max(1, cost_micros)` for that item, using the `cost_micros` returned by
  `attempt_with`); the paired vectors are `(condition_0[i], condition_1[i])` item-aligned across the
  two conditions (alignment key = `BenchItem.id`).
`ArmResult.ci` per arm: **Arm A** calls `paired_bootstrap_ci(condition_0_scores, condition_1_scores)`
(the `Score`-typed path); **Arms B and C** call `paired_bootstrap_ci_raw(condition_0_vec,
condition_1_vec)` (the raw `f64` `net_value_per_cost` path, §3.12).

**`AntiTheaterReport.degenerate_to_distillation` aggregation rule.**
`report.degenerate_to_distillation = per_arm[A].degenerate_to_distillation` (it **mirrors Arm A's
flag**) and is **informational only — it does NOT affect the `Fires`/`Null` verdict**. This makes the
M2 "verdict not over-claimed" check objectively checkable.

**`ConditionFlags` runtime hook wiring (the five flags, where each bites a turn).** `router_pin` is
wired via the `Router::route` pin (`self.pin`, §3.11) — `Some(base)` ⇒ `route()` returns `base`
unconditionally; `retrieval_enabled` and `metabolic_journaling` are wired at §3.5 (empty `ContextPack`
retrieval) and §5 (executor skips reserve/settle/attest journaling) respectively. This sub-section pins
the remaining two:
- **`consolidation_enabled` (v0 NO-OP).** In v0 the `Consolidator` is **idle-triggered only**
  (orchestrated on the `IDLE_GAP_MS` idle window, §3.8) and is **NEVER invoked inside `attempt_with`**.
  So `consolidation_enabled` is a **documented v0 no-op**: toggling it changes nothing within a turn,
  and **Arm A's / Arm B's consolidation delta is INERT in v0**. Consequently the Arm-B full-metabolic
  vs stubbed-scaffold difference is attributable **solely to `metabolic_journaling`** in v0 (the
  reserve/settle/attest journaling path), recorded on the `AntiTheaterReport` contract below.
- **`expose_budget_trust` (Arm C, real delta).** Wired via `project_for_proposer(&ctx,
  expose_budget_trust)` (§3.5): when `true`, `ProposerContext.budget_replica`/`trust_snapshot` are
  `Some(...)`; when `false`, they are `None`. `Proposer::propose` takes `&ProposerContext` (§3.4);
  `commit()` always reads the full `&ContextPack` (it deterministically requires `trust_snapshot` +
  `budget_replica` for access control + budget). The **echo proposer ignores the projection**, so on
  the echo proposer Arm C's delta is 0; on a real proposer it is a nonzero-by-construction
  visible-vs-hidden difference.

**`AntiTheaterReport` consolidation-inertness contract (v0).** The report records that in v0
`consolidation_enabled` is inert (no in-turn consolidation), so any Arm-B full-metabolic-vs-stubbed
delta is attributed **solely to `metabolic_journaling`**, not to consolidation. This makes the Arm-B
attribution unambiguous.

---

## 4. Data model (SQLite, one DB per being; supervisor DB separate)

Primary-key generation: episodic `id` = monotonic local rowid; semantic `id` = rowid; skills
`skill_id = blake3(canon(manifest))` (content-addressed); ledger keyed by `(did, seq)`.

- **`journal`** — `(did, seq PK, prev_hash, event_kind, payload, sig, wall_ts)`; single-writer per
  `did`; WAL + `synchronous=FULL`; append fsyncs the WAL frame before returning (§3.3). `payload =
  canon(event)`; `prev_hash = entry_hash[n-1]`; `sig = Signer.sign(entry_hash[n])`. **No `entry_hash`
  column** — `entry_hash[n]` is recomputed (§3.3). The idempotency nonce in `commitment_hash` is a
  **monotonic per-DID counter** (`next_commitment_nonce`, §3.3), reconstructable on replay from the
  journal alone.
- **`episodic`** — `(id, did, valid_from, valid_to, ts_txn, provenance_class, supersedes, payload,
  sig)`; append-only; bitemporal. `valid_from`/`valid_to`/`ts_txn` are **`u64` wall-clock micros**;
  `valid_to = u64::MAX` marks the current belief. **Corrections never overwrite**: a correction is a
  new row whose `supersedes = Option<EpisodicId>` points at the prior row (the prior row is left
  physically intact; **`A.valid_to` is NOT mutated** — supersession resolved at query time).
  `query_as_of(did, valid_at, known_at)` uses the §3.8 algorithm (`TimePoint` reduced to `wall_ts`
  only): select `valid_from <= valid_at AND ts_txn <= known_at`, exclude any row that is the
  `supersedes` target of another row whose `ts_txn <= known_at`, and return the **full surviving set**
  (one current row per supersession chain — no global latest-by-`ts_txn` collapse, §3.8). Per-row
  signing payload = `canon(id || did || valid_from || ts_txn || provenance_class || payload)`.
  **Worked retroactive-correction example:** at `ts_txn=10` insert row A `{fact: rent=$1000,
  valid_from=Jan1, valid_to=MAX}`. On `ts_txn=20` we learn rent was $1200 from Jan1: insert row B
  `{fact: rent=$1200, valid_from=Jan1, valid_to=MAX, supersedes=A}`. `query_as_of(valid_at=Feb1,
  known_at=15)` returns **A ($1000)** (B's `ts_txn=20 > 15`, so A is not excluded); `query_as_of(
  valid_at=Feb1, known_at=25)` returns **B ($1200)** (A is excluded as B's supersedes target,
  `ts_txn=20 <= 25`). Episodic `payload` = the `EpisodicPayload` struct (§3.8) encoded with `canon`:
  `{ percept: Percept, proposal_hash: Hash, commitment_hash: Hash, attestation_hash: Hash,
  objective_score: f64, domain: Domain, routing_decision: RoutingDecision }` — `domain` and
  `routing_decision` back `PerDomainStats` derivation (§3.11). **Indexes:** `(did, ts_txn)` and
  `(did, valid_from)`. **`valid_to` is write-only in v0** — `query_as_of` resolves supersession via
  `supersedes` only and **never reads `valid_to`** (no implementer should add `valid_to` into the read
  query).
- **`semantic`** — `(id, did, ts_txn, payload, provenance_class='ModelInference', trained_on_untrusted,
  sig)`; written only via `Consolidator` (cannot escalate trust); `trained_on_untrusted = OR of
  source-row taint`. `payload` = `SemanticPayload` (crate-private canon'd serde blob, §3.8). `ts_txn` is
  the crate-assigned wall-clock micros; the per-row `sig` covers `canon(id || did || ts_txn ||
  ProvenanceClass::ModelInference || trained_on_untrusted || payload)` (the **semantic** signing
  formula, §3.8 — distinct from episodic).
- **`skills`** — `(skill_id, parent_id, manifest, sig)`; `skill_id = blake3(canon(manifest))`;
  population-based (variants branch from any ancestor via `parent_id`; revisions never overwrite
  ancestors).
- **`skill_revocations`** — `(skill_id, sig, ts)`; `SkillRevoke` inserts here (a separate table, not a
  second `skills` row); `installed_skills` membership excludes any `skill_id` present here at read time;
  revoke does not cascade (§3.8).
- **`genomes`** — `(genome_hash PK, version INTEGER, payload, sig)`; `genome_hash = blake3(canon(Genome))`;
  `payload = canon(Genome)`; `version` = the `GenomeVersion.version` (§3.7) of the mutation that produced
  this snapshot (with a UNIQUE index on `version` for the `load_generation` selection key, §3.12). Each
  `GenomeMutation` persists the **FULL post-mutation `canon(Genome)` snapshot** under its hash (§3.7), so
  **reversal = fetch the `prev_genome_hash` row and re-install**, and **`load_generation(g)` selects
  `WHERE version == g`** (§3.12).
- **`ledger`** (supervisor DB) — `(did, seq, pot, kind∈{reserve,settle,credit,debit}, source, task_origin,
  effect_class, microdollars, turn_id, lease_id, expires_wall_ts, reserved, ts INTEGER, idem_key BLOB
  UNIQUE)`; `source` is the `CreditSource` for `kind=credit` rows (NULL otherwise) backing the §3.6
  survival partition; `task_origin` is the `TaskOrigin` (§3.10, NULL on non-`Revenue` credit rows)
  backing the M5 external-revenue predicate (§6 M5); `ts` is the row's wall-clock micros (supervisor
  clock, §3.6) for the M5 `ts <= t` predicate;
  `effect_class` is the row's `EffectClass` (§3.6) backing the in-flight egress filter; `idem_key` stores
  **`canon(IdemKey)` as a BLOB with a UNIQUE index** (§3.6), enforcing idempotent reserve/settle (one
  row per step, `kind` mutated in place from `reserve` to `settle`); `lease_id` carries the
  `ReserveVerdict::Granted` lease; `expires_wall_ts` + `reserved` persist the original `Granted` so a
  re-issued reserve reconstructs it (§3.6). `turn_id` carries the commitment's `commitment_hash`
  ("turn = all reserves sharing one `commitment_hash`"; the **reserve batch is all such rows, inserted
  in one txn**, §3.6). `pot ∈ {Survival, Exploration, Distill}` (§3.10). Survival pot is a hard
  partition: **never credited by Exploration/Distill** (arch §5.1). `survival_floor` (§7) bounds the
  Survival-pot balance, sampled at end of each settled turn (§3.10 SQL).
- **`dedup_ledger`** (supervisor DB) — `(idem_key BLOB UNIQUE, state, outcome_blob BLOB,
  expires_wall_ts INTEGER)`; backs at-most-once egress **and** the non-egress side-effecting effects
  `MemoryWrite`/`Sign` (§5; `Query`/`Infer` are pure and exempt). `idem_key` = `canon(IdemKey)` BLOB
  (same form as `ledger`). First use inserts `state = consumed` with `outcome_blob = canon(EgressOutcome)`
  (`emitted = true` at emit time); a replay reads the row and returns the recorded `EgressOutcome` with
  `emitted = false`, never re-emitting.
- **`attestations`** — `(did, seq, subject, evidence_hash, sig)`; a denormalized read index mirroring
  the `Attestation` (§3.4) fields `subject`/`evidence_hash`/`sig`. The remaining `Attestation` fields
  (`step_index`/`turn_id`/`accepted`/`actual_cost`/`duration_ms`) live in the opaque
  `Attestation(Bytes)` journal blob (the source of truth); this table is not authoritative. (Trust is
  journal-derived, §3.9 — no `trust_level_at_commit` column is stored.)
- **`insolvency`** (supervisor DB) — `(did PK, insolvent_streak INTEGER)`; persisted reaper
  insolvent-streak counter, reloaded on supervisor `open` (§3.10).
- **`lineage`** — `(child_did, parent_did, generation, fork_seq, snapshot_offset, saga_state∈{PENDING,
  COMMITTED})`.
- **`domain_models`** — `(domain, model_ref, base_ref, trained_from_period BLOB, promoted_bool, sig)`;
  `trained_from_period = canon(Window{ from_seq, to_seq })` stored as a BLOB column (§3.11).
- **`gap_slices`** (distill DB) — `(domain, detected_at, items_blob, seed)`; persists the frozen
  `GapSlice` including its reproducible-sampling `seed` (§3.11).
- **`teacher_traces`** (distill DB) — `(domain, turn_id PK, prompt, output, approved, executed,
  accepted, shadow, provenance, method)`; `output` = the approved teacher output (`TeacherTrace.output`/
  `TrainExample.target` source); read by `collect_teacher_traces(window)` (§3.11).
- **`bench_items`** (bench DB) — `(item_id PK, kind∈{Frozen,Rolling}, prompt, expected, prompt_hash,
  domain, provenance, method_tags_blob)`; `prompt_hash = blake3(prompt)`; `method_tags_blob =
  canon(Vec<String>)`; `domain`/`provenance`/`method_tags` map to the `BenchItem` fields (§3.12); source
  for `run()` (Frozen) / `run_rolling()` (Rolling), §3.12.
- **`bench_config`** (bench DB) — `(key PK, value)`; holds `day0_weights_hash` (the `run()` pin,
  §3.12).

`Genome` serializes (via `canon`) to a signed blob; the **heritable slice** on fork = genome + a
consistent-cut **episodic + semantic + skills** slice at the same journal offset (in v0 these three
row-types ARE procedural memory; there is no separate procedural store — §3.10 fork-saga consistency).

---

## 5. The per-step state machine (executor contract)

This is the load-bearing contract — the executor implements exactly this table (arch Appendix A).
Crash-recovery is part of the contract, not an afterthought.

| from | event | to | guard / recovery |
|---|---|---|---|
| proposed | committer approve | committed/journaled | **fsync signed Commitment before any effect** |
| committed/journaled | fsync marker | execute-attempted-marked | marker fsynced **before** dispatch |
| execute-attempted-marked | reserve (IPC, idempotent, per-turn BATCH) | reserved | supervisor all-or-nothing batch txn; carries `turn_id`; `Granted` returns `lease_id`+`expires_wall_ts`; `B_INFLIGHT` + per-turn effect-count caps hold |
| reserved | dispatch | dispatched | capability ceiling read from the **live being-local trust ledger** (see exception below); lane width ≤ trust-gated `lane_count` (= 1 in v0); carries `lease_id` |
| dispatched | observe + attest | attested | attest fsynced after effect; emits `Attestation` (§3.4) |
| attested | settle (IPC, idempotent) | settled | duplicate settle = no-op (same `idem_key`); carries `lease_id` |
| reserved | trust contraction | committed/journaled | batch-release: release whole batch, re-reserve survivors (surviving `(IdemKey, Microdollars, EffectClass, SpendCtx)` subset; on turn-level `Exceeded` the released set is chosen by descending `step_index`, §3.6) |
| reserved | `now > expires_wall_ts` | committed/journaled | reaper auto-releases the lease (§3.6) |
| any | crash | recovery-entry | re-issue reserve (idempotent); read own marker; apply §3.3 recovery rule |

**Batch-reserve ownership relative to per-step `run_step` (pinned).** The reserve batch is **per-turn**
(all committed steps sharing the `turn_id`, §3.6) but `run_step` is **per-step**: the **executor issues
ONE `reserve()` for the whole `committed_steps` batch BEFORE the per-step dispatch loop**; each
`run_step` then **assumes its row is already reserved** and only does dispatch / attest / settle. On
`Exceeded`, the **executor (NOT `run_step`)** runs the survivor-drop + re-reserve loop (descending
`step_index`, §3.6) and marks the released steps `final_state = Committed` with
`budget_verdict = Exceeded`. This makes batch-release ownership deterministic.

**Dispatch capability ceiling (reconciled).** At the **live dispatch boundary** read the ceiling from
the **being-local trust ledger** (which contraction writes synchronously); if the live ceiling forbids
the step's `EffectClass`, take the `reserved → committed/journaled` batch-release transition.
**Exception on crash-recovery:** do **not** re-read live trust — honor the **journaled commit-time
verdict** (arch §4.5 commit-time-snapshot lean). Stating this exception explicitly is what keeps
arch §4.5 and §4.6 from conflicting.

**Per-`StepState` crash-recovery truth table.** On recovery the executor reads its own journaled
marker (and the dedup ledger) to place each step at a resume point and act:

| `StepState` at crash | effect emitted? | resume action |
|---|---|---|
| `Proposed` | no | re-run from commit (nothing journaled yet) |
| `Committed` (journaled) | no | proceed to fsync ExecMarker, then reserve |
| `ExecAttempted` (marker fsynced) | no (marker precedes dispatch) | re-issue reserve idempotently (shared `IdemKey`), then dispatch |
| `Reserved` | no | dispatch (reserve row already present; re-issue is a no-op echo, §3.6) |
| `Dispatched` | **unknown** | consult the dedup ledger by `IdemKey`: if present ⇒ effect already emitted, skip emit and proceed to attest; if absent ⇒ re-dispatch (re-emit) |
| `Attested` | yes | settle (idempotent) |
| `Settled` | yes | done (no-op) |

**At-most-once extends to NON-egress side-effecting effects.** The dedup-on-`IdemKey` discipline that
protects egress (`Http`/`Notify`/`Render`/`Payment`, §3.10) is **reused for `MemoryWrite` and `Sign`**:
each consults the **same `IdemKey`-keyed dedup ledger** (`canon(IdemKey)` BLOB) before performing the
effect, so a crash between `Dispatched` and `Attested` cannot duplicate an episodic row or a signature.
`Query` and `Infer` are declared **pure** (no external side effect; re-execution is safe and produces
an equivalent result), so they are exempt. **Recovery re-executes only effects whose `IdemKey` is
absent from the dedup ledger.**

**Turn boundary + caps.** A `turn_id: Hash` (= `commitment_hash`) rides on every reserve; the
supervisor resets the **per-turn effect-count counter** on a new `turn_id`. The per-turn effect-count
cap counts **only egress/payment-class accepted reserves** (`Http`, `Notify`, `Render`, `Payment`),
not survival-inference debits. `B_INFLIGHT` = cross-lane reserved-but-not-settled ceiling.

Two invariants the executor must preserve (property-tested in §8):
- **Two distinct named bounds, supervisor-enforced, independent of settle timing:**
  `inflight_egress_count(did) <= B_INFLIGHT` (in-flight, cross-turn) **AND**
  `per_turn_count(did, turn_id) <= PER_TURN_EFFECT_COUNT_CAP` (cumulative, per-turn). Not a `min()`
  over a per-turn-cumulative quantity (§3.6/§3.10).
- **Indeterminate-class egress** (free http / notify / render-as-carrier) is *un-dispatchable*
  except through the supervisor egress proxy with a `DedupToken` keyed on `(commitment_hash,
  step_index)` = `IdemKey` (§3.10 at-most-once); proxy-down ⇒ fail closed. Mandatory for any
  lethal-trifecta context.

---

## 6. Milestones (each ends in a runnable artifact + an acceptance test)

> Vertical slices, not layers. Every milestone produces something you can run and a test that must
> pass before the next begins.

**M0 — Substrate skeleton.** Identity + journal + memory + the typed seam with the **echo proposer**
(no real model). A turn runs end-to-end: perception → propose → commit → journal → execute (no-op
effects) → attest → journal.
- *Acceptance:* a turn completes; every step is signed and journaled; **replay of the committed tail
  is deterministic**. **Determinism oracle:** replaying from seq `s` **twice** yields byte-identical
  (`canon`-encoded) reconstructed `LoopState` and identical reconstructed `IdemKey`s
  (`commitment_hash` + `step_index`), **and** the recomputed `prev_hash` chain over replayed events
  reproduces `head().1`; `replay_from(seq)` yields `ReplayedEvent`s with `.seq >= seq` in ascending,
  inclusive order (the readable `.seq`, §3.3). The M0 oracle **canon-encodes the FULL `LoopState`** —
  **all five fields** (`iteration`, `prior_intents`, `prior_objective_scores`, `summarized_history`,
  `repeat_signature_counts`), every one recomputable from the journal (§3.5). The
  `trust_snapshot`/`budget_replica` exemption is **NOT** a `LoopState` carve-out (they are not
  `LoopState` fields); it applies only to the **separate `ContextPack`-equality check** (those are
  `ContextPack` fields, supervisor-authoritative replicas, pinned-not-hashed, §3.5). Property tests: a 7th `MutationKind` variant fails to compile; no-launder
  provenance holds (the §3.1 guard rejects a model-output row declared `DirectUserIntent` without a
  valid `SecondPartyConfirmation` per the §3.1 predicate; **non-forgeable** = no public path
  constructs a `ProvenanceToken`, §3.1); single-head-per-DID.

**M1 — Metabolism plumbing.** Account + supervisor `reserve`/`settle` + the per-step state machine +
the **reaper**. Spend is metered and debited; the survival pot is partitioned.
- *Acceptance:* `reserve` rejects over-cap spend (budget **binds**); the reaper fires on **sustained
  insolvency** (`Survival`-pot `balance < survival_floor`, sampled at end of each settled turn, for
  `REAPER_INSOLVENT_TURNS_N` consecutive settled turns) and **journals a Death event**; the
  out-of-band kill path meets the measured kill-latency bound — timer **start** = kill request
  enqueued at the host agent (`HostKiller::enqueue_kill`); **stop** = the last PID in the being's
  cgroup confirmed dead via host process accounting; **clock** = host monotonic (the out-of-band path
  bypasses the supervisor). **Saturated-supervisor condition** = the supervisor IPC queue held at
  `>= KILL_SATURATION_N` pending reserve requests (`KILL_SATURATION_N = B_INFLIGHT × 10`, §7). Run
  `KILL_TRIALS_K` trials (§7); **pass iff p100 (worst-case) latency over K trials
  `<= KILL_LATENCY_CEILING_MS`**. Property test: the **two named bounds**
  (`inflight_egress_count(did) <= B_INFLIGHT` AND `per_turn_count(did, turn_id) <=
  PER_TURN_EFFECT_COUNT_CAP`, §3.6/§3.10 — distinct row sets, not a `min()`) hold under the **§3.10
  stale-replica fuzz model** (inject **both** stale-high and stale-low `BudgetReplica.balance`; assert
  both supervisor-enforced bounds hold under both arms).

**M2 — Real proposer + the bench.** `being-proposer-{openai,local}`; the **falsification bench**
(frozen + rolling, provenance-isolated), paired-bootstrap CI, and the **anti-theater arms (a)(b)(c)**
wired (§7, §9).
- *Acceptance:* the bench runs Day-0 vs Day-N with the **frozen proposer weights byte-identical**
  (`run()` asserts `frozen_proposer.weights_hash == Day-0 pinned value`; the being's
  adapters/navigator/genome vary by `at.generation`, §3.12) and emits a CI; the anti-theater harness
  runs all three arms and produces a report (the *result* may be null — that is a valid outcome).
  **Correctness oracle (so M2 is not trivially satisfied):** on the **pinned synthetic fixture** where
  Day-N = Day-0 + delta, `paired_bootstrap_ci` must return `lo > 0` and `p < 0.05`. **Pinned fixture
  (deterministic across implementers):** `n = 200` item-aligned pairs; `delta = 0.20`;
  `day0[i].value = 0.50` for all `i`, `dayN[i].value = 0.70` for all `i` (so every paired difference
  `= +0.20 > 0`); seed `= 0` (no journal context, §3.12). With every resampled paired-mean-diff `= 0.20
  > 0`, `p = 0/B = 0 < 0.05` and `lo = hi = 0.20 > 0` — deterministic. On **identical** Day-0/Day-N
  fixtures (`day0 == dayN`) it must return a CI satisfying **`lo <= 0 <= hi`** (the zero-difference
  degenerate CI `[0,0]` passes — not a spurious failure); the anti-theater report must contain one
  `ArmResult` per arm with the pre-registered margin recorded. This tests the machinery without
  prejudging the (legitimately null) real-data verdict. This is the milestone where "does it
  compound?" becomes measurable.

**M3 — Learning layer.** Consolidation + the per-domain distillation flywheel (LoRA on frozen base) +
navigator/routing + the promotion gate + the **catastrophic-forgetting gate**.
- *Acceptance:* distillation closes the gap on `(teacher-success ∩ student-weak)` for ≥1 domain.
  **Promotion gate (three-arm):** frozen-base-only vs base+adapter vs teacher on the held-out
  `GapSlice` (the **same gap distribution frozen at first detection**, §3.11); pass iff
  `gap_closure >= GAP_CLOSURE_MARGIN_X` (§3.11 formula) with `effect_size >= VALIDATION_EFFECT_FLOOR`.
  The `method` tag on traces is held out of training (`TRAIN_HELDOUT_FRAC`, §7).
  **Forgetting gate (deterministic scalar, NO power clause — §3.11):** baseline = **routed quality
  (mean grader `Score.value`)** on the `MixedHeldoutSet` under the **pre-promotion** genome
  (`pre_genome`); pass iff `pre - post < FORGETTING_NI_MARGIN` **per domain** and **per high-value
  subclass** (`high_value` tag, operator-tagged or top-tariff) (both modes: adapter on a frozen
  own-domain slice; adapter on its own hard subclass with forced routing via the injected `Router`). **Re-clear
  scope for a single promotion = ALL currently-promoted adapters** (a routing-distribution change).
  The compounding bench detects accumulation **or** reports saturation (a negative result is a valid
  contribution).
  **M3 synthetic-fixture recipe (CI-checkable without an external operator corpus, mirroring M2):**
  build a **deterministic seeded `MixedHeldoutSet`** (fixed seed) and a synthetic **teacher/student
  gap fixture** in which `gap_closure` is **known-positive for ≥1 domain** (the teacher reference
  beats the frozen base on that domain's items by a fixed margin; the student-weak rows are
  hand-seeded), so both the promotion gate (`gap_closure >= GAP_CLOSURE_MARGIN_X`) and the forgetting
  gate (`pre − post < FORGETTING_NI_MARGIN`) are exercised end-to-end in CI. **Minimum item counts:
  `>= MIN_HELDOUT_N` per domain and `>= MIN_HELDOUT_N` per high-value subclass** so the gates'
  per-domain / per-high-value-subclass passes are statistically meaningful on the fixture.
  **Forgetting-gate fixture recipe (extends the above):** assign each `MixedItem` a subclass by
  **seeded round-robin over `SUBCLASS_K` subclasses** (§7, v0 = 4): **item `i` → subclass `i mod
  SUBCLASS_K`** (the pinned round-robin start index). **`high_value = (subclass == subclass 0)`** (the
  first round-robin subclass is the single high-value class). The `MixedHeldoutSet` total item count is
  **`>= SUBCLASS_K * MIN_HELDOUT_N`** so the single `high_value` subclass clears `MIN_HELDOUT_N` under
  round-robin. This makes the seeded `MixedHeldoutSet`'s `per_subclass` `BTreeMap` key-set
  byte-reproducible and the M3 per-high-value-subclass acceptance deterministic. **Pin the synthetic grader**
  so that (positive/known-pass variant) the **candidate adapter scores within `FORGETTING_NI_MARGIN`
  of base** on the `MixedHeldoutSet` ⇒ `pre − post < FORGETTING_NI_MARGIN` ⇒ pass; and (negative
  variant) an **induced-forgetting candidate** that drops `>= FORGETTING_NI_MARGIN` on the high-value
  subclass ⇒ known fail. Both variants run in CI, making the forgetting half of M3 objectively
  checkable.

**M4 — Self-modification.** `Improver` + the closed surface + the **Two-Gate** (Validation +
Capacity) + self-judgment-bias mitigation (separate eval model, rubric scoring, calibration tracking).
- *Acceptance:* a genome mutation passes both gates, is signed/journaled/**reversible** (reversal =
  re-install the `prev_genome_hash` parent full snapshot from the `genomes` table, §3.7).
  **Capacity-Gate false-admit protocol:** generate a labeled corpus of known-overfit (true-reject)
  candidate genomes — **reproducible recipe:** train a candidate adapter to **memorize a fixed
  `N`-item slice** (epochs until train-set score ~1.0) while **forcing `heldout_gap_candidate >
  heldout_gap_base` by margin `M`**; **label = true-reject**. **`M` is pinned so labeled true-rejects
  sit genuinely above the rejection threshold:** `M = CAPACITY_NI_MARGIN + CAPACITY_FALSE_ADMIT_EPS`
  (i.e. the forced `heldout_gap_candidate − heldout_gap_base` lands in
  `[CAPACITY_NI_MARGIN + ε, ...]`), with `CAPACITY_FALSE_ADMIT_M (M)` and `CAPACITY_FALSE_ADMIT_EPS
  (ε)` pinned numerically in §7. A correct Capacity Gate must therefore **reject** every corpus member;
  a false-admit is a member the gate wrongly admits. The corpus is **deterministic given the
  fixed slice + seed**. Count `>= MIN_HELDOUT_N`; evaluate the Capacity Gate (the deterministic scalar
  comparison, §3.7) over them; **pass iff** measured false-admit rate `<= VALIDATION_FDR_Q` (§7), i.e.
  the Capacity-Gate false-admit rate ≤ the Validation Gate's false-discovery budget.

**M5 — Value source (makes the economic gate live).** Commit **one concrete payer** (operator-as-
customer: published per-task-class tariff, stated arrival process, supervisor-computed acceptance
grader with held-out/non-stationary anti-Goodhart defenses). Build the **exogenous-payer hook** so a
real marketplace can replace the operator without code change.
- *Acceptance:* an external-revenue ledger is live and bounded by external inflow. **External-revenue
  invariant (checkable assertion):** for all `t`, `cumulative_revenue_credits(t) <=
  cumulative_external_inflow(t)` (per-DID, over rows with `ts <= t`). The two sides are pinned SQL
  predicates over the `ledger`:

  ```sql
  -- cumulative_revenue_credits(t): external-sourced revenue only (Revenue origin, Exploration pot):
  SELECT COALESCE(SUM(microdollars), 0) FROM ledger
  WHERE did = ?1 AND kind = 'credit' AND source = 'External'
    AND pot = 'Exploration' AND task_origin = 'Revenue' AND ts <= ?2;
  -- cumulative_external_inflow(t): ALL external inflow (the bound):
  SELECT COALESCE(SUM(microdollars), 0) FROM ledger
  WHERE did = ?1 AND kind = 'credit' AND source = 'External' AND ts <= ?2;
  ```

  **Genesis/fork `Survival` seed credits (also `source = 'External'`, §3.10/fork-saga) are EXCLUDED
  from the revenue side** — they are **endowment, not revenue** (the revenue predicate's
  `pot = 'Exploration' AND task_origin = 'Revenue'` clauses exclude them, since seed credits target
  `Survival`). They DO count toward `cumulative_external_inflow` (the bound), so the invariant is
  non-trivial: revenue can never exceed total external inflow. (The `ledger` carries a `task_origin`
  column on credit rows for this predicate; a dedicated revenue flag is the equivalent alternative.)
  **"Live" = this invariant assertion passes on a non-empty revenue ledger.** **Derived-threshold formulas (the
  first-arrow chain, §7 referents):** `cost_floor = sum over a task's steps of cost_ceiling(step)`
  (the survival-inference floor, §3.5); `amortized_teacher_cost = (total teacher microdollars) / (N
  distilled tasks)`; `tariff = per-task revenue`. Then `required_accepted_fraction = (cost_floor +
  amortized_teacher_cost) / tariff` (break-even), and `required_gap_closure =
  curve^{-1}(required_accepted_fraction)`, where `curve^{-1}` inverts the **measured curve = the
  empirical gap-closure-vs-accepted-fraction points from completed M3 promotions**. **Inversion
  procedure (reproducible):** sort the points by `accepted_fraction`; **enforce
  monotone-nondecreasing** (on a violation take the **monotone envelope** — running max — so the curve
  is invertible); **then collapse tied `accepted_fraction` x-values to a single point keeping the max
  `gap_closure`** (guaranteeing strictly-increasing x, hence a unique piecewise-linear inverse); the
  inverse is **piecewise-linear between the two bracketing points**; an out-of-range
  `required_accepted_fraction` is **clamped to the nearest endpoint**. **Minimum point
  count = 2**; below 2 points `required_gap_closure` is **undefined** and the §9 gate **stays
  methodology-only** (it does not go live). The §9 anti-theater gate moves from
  *methodology* to a *live, derived* threshold; until an exogenous payer is committed, every
  value-capture claim is **labeled efficiency-only** (arch §5.2).

**M6 — Research arm (gated; selection OFF until entered).** Fork saga + population + lineage are built
as substrate; **fitness = `net_value_per_cost`** (the §3.12 definition, reused verbatim; the earlier
"net-value/time" phrasing is dropped to remove the inconsistency — there is no `time` term); selection
(crossover stays out of v0 scope, §3.7).
- *Entry gate:* **only** after M2/M3 show compounding **and** the anti-theater gate fires. Otherwise
  selection stays off and M6 is instrumentation only. *Acceptance (when entered):* the **fork-saga
  crash-injection half is already checkable** — a fork is a signed, crash-recoverable snapshot;
  enumerate the **crash-injection points** `{pre-barrier, mid-drain, PENDING-no-budget-txn,
  COMMITTED-before-durable}` with post-recovery assertions using the §3.10 recovery predicate (a child
  cannot inherit memory referencing episodes absent from its journal prefix). **Per-copied-row-type id
  checks the predicate enumerates:** (1) **semantic** — every `SemanticFact.source_episodic_ids` is a
  **subset of the child's journal-prefix episodic ids**; (2) **episodic** — each row's `supersedes`
  target is **present in the copied slice or `None`**; (3) **skills** — carry **no episode reference**
  (none to check). This makes "references an absent episode" mechanically checkable.
  **Fitness-variance gate (statistic + test + power calc):**
  - **Statistic + test:** an **F-test** (Levene's test is the equal-acceptable alternative) comparing
    the **between-lineage `net_value_per_cost` variance** against the **neutral-drift control variance**,
    evaluated at `GATE_ALPHA` (§7). Signal = between-lineage variance significantly exceeds the
    neutral-drift control.
  - **Neutral-drift null model (so the power calc is computable from §7 step-sizes).**
    (1) **Per-generation mutation sampling:** each generation, every lineage draws **one**
    `MutationKindTag` uniformly at random over the 9 tags (probability `1/9` each), yielding that
    generation's `step_size` from the §7 per-`MutationKindTag` table.
    (2) **`g(step_size) → fitness perturbation`** (in `net_value_per_cost` units, the §3.12 fitness):
    a generation's fitness perturbation is `fitness_delta ~ Normal(0, NULL_DRIFT_C * step_size^2)`
    with `NULL_DRIFT_C` pinned in §7 — i.e. under neutrality a mutation perturbs fitness with **zero
    mean** and variance scaling with `step_size^2`. (If a domain prefers a measured null, substitute a
    `MIN_DOMAIN_N`-item pilot measuring per-step `net_value_per_cost` variance for
    `NULL_DRIFT_C * step_size^2`; v0 default is the parametric `g` above.)
    (3) **Lineage-variance aggregation:** a lineage's accumulated per-generation `fitness_delta`s sum
    (independent increments) into a lineage-terminal fitness with variance
    `sigma_null^2 = sum over its G generations of NULL_DRIFT_C * E[step_size^2]`, where
    `E[step_size^2] = (1/9) * sum over the 9 tags of step_size_tag^2` (the uniform-draw expectation).
  - **Power calc (`solve_power() -> (K, G)`, deterministic).** Given `sigma_null^2` (above) the solve
    has three pinned pieces:
    (a) **Target alternative:** detect between-lineage variance `>= FITNESS_VARIANCE_RATIO * sigma_null^2`
        (the ratio `r`, §7) — i.e. the noncentral-F effect is parameterized by `r`.
    (b) **Degrees of freedom + noncentrality (functions of `K, G`):** one-way F with **`df1 = K − 1`**
        and **`df2 = K * (G − 1)`**; power is computed via the **noncentral F** distribution
        constructed as **`statrs::distribution::FisherSnedecor::new(df1, df2)`** for the central critical
        value and the noncentral CDF evaluated with the noncentrality below (statrs's noncentral-F via
        the series form; equivalently the noncentral-F survival at the central
        `F_{1-GATE_ALPHA}(df1, df2)` critical point). **Noncentrality (one-line equation in `r, K, G`,
        F effect-size convention):** `lambda = (K − 1) * (r − 1)` — i.e. the between-lineage
        variance-ratio alternative `var_between = r * sigma_null^2` against `var_within = sigma_null^2`
        yields effect size `f^2 = (r − 1)` per the F convention, and `lambda = df1 * f^2 = (K − 1)(r −
        1)`. The critical value is `crit = FisherSnedecor::new(df1, df2).inverse_cdf(1 − GATE_ALPHA)`;
        `power = 1 − noncentral_F_cdf(crit; df1, df2, lambda)`.
    (c) **Optimization objective + grid bounds (pinned so `(K, G)` is objectively checkable):**
        **minimize total runs `K * G`**, ties broken by the **smallest `K`**, via a **bounded grid scan
        over `(K, G)`** with **`K ∈ [2, 50]`, `G ∈ [2, 200]`, integer step 1**. Scan all `(K, G)` on
        this grid; among those whose computed `power >= FITNESS_VARIANCE_POWER` (§7) at `GATE_ALPHA`
        (§7), pick the one minimizing `K * G`, then minimizing `K`. This tie-break is **total over the
        pinned grid**, making `solve_power() -> (K, G)` a **deterministic function** and M6 objectively
        checkable.
    **Mutation step-size = the fixed per-`MutationKindTag` constant** (no distance is definable over
    opaque genome `Bytes`; the table is §7) — it is the genome-distance referent that fixes
    `sigma_null^2`.
  Pass at the stated power, or the "breeding-program-not-evolution" result is reported (arch §0).

---

## 7. The bench is a first-class deliverable (M2) + v0 pre-registered constants

Build it early so compounding is measurable from the second milestone, not asserted at the end.

- **Frozen bench:** fixed at generation zero, never pruned, never sourced from the being's own
  failures, provenance-isolated, audited disjoint from training data (`audit_disjoint`, §3.12). Run at
  multiple time points, **frozen proposer weights byte-identical**; paired-bootstrap CI for monotone
  Day-0 → Day-N improvement at p < 0.05 across the **derived** replication count (`n_reps`, §3.12).
- **Rolling bench cross-check:** require frozen-bench improvement on the **`method_overlap_subset`
  (promoted-domain overlap, §3.12)**, AND rolling-bench improvement exceeding the
  **re-distill-to-drift baseline** (re-train only on the rolling window, no accumulation; comparison
  metric = mean `Score.value`, §3.12). (Frozen under-detects drift-tracking; rolling over-detects via
  train/test proximity — both gates together prevent a false pass.)
- **Anti-theater arms** (the master gate, §9): (a) skeleton-does-work with proposer FIXED
  (distillation-pays variant for v0, flagged, §3.12); (b) metabolic-vs-stubbed as a directional pass
  with the cap-confound excluded (identical `shared_task_set`; metric = `net_value_per_cost`); (c)
  budget/trust-visibility toggled with both budgets finite. (Full types + execution plans +
  `Fires`/`Null` predicate in §3.12.)

Threshold discipline: every gate's number is **pre-registered before the run**, and thesis-critical
numbers are **derived from the economic claim** (tariff − cost floor − amortized teacher cost ⇒
required accepted fraction ⇒ required gap-closure via a *measured* curve, M5), not asserted.

**`ANTI_THEATER_MARGIN_A`/`ANTI_THEATER_MARGIN_C` are operator-registered placeholders in v0** — they
have **no derivation referent** (unlike `required_gap_closure`, which is derived in M5). This is **not a
code-level blocker**: the `Fires` verdict is gated behind M6 entry, so the A/C margins do not need a
derivation in v0. (`ANTI_THEATER_MARGIN_B = 0.0` is the directional-pass referent, also not derived.)

### v0 pre-registered constants

Each value is a **v0 placeholder; the operator re-registers before a run; superseded by the §7
economic derivation once the M5 payer lands.** This single table dedups every "undefined gate number"
reference across mutation / distill / bench / economy / policy / runtime.

| Constant | v0 default | Used by |
|---|---|---|
| `VALIDATION_FDR_Q` | 0.05 | Validation Gate (single-arm ⇒ alpha); M4 false-admit ceiling |
| `VALIDATION_EFFECT_FLOOR` | 0.10 (std-effect) | Validation Gate; promotion effect floor |
| `CAPACITY_NI_MARGIN` (δ) | 0.02 (gap units) | Capacity Gate (deterministic scalar) |
| `GATE_POWER` | 0.8 | all gate power calcs; `power_n` `z_{1-beta}` |
| `GATE_ALPHA` | 0.05 | gate significance; fitness-variance |
| `MIN_HELDOUT_N` | 200 | gates; M4 false-admit corpus size; `pilot_variance` pilot run |
| `ACCEPT_THRESHOLD` | 0.6 | AcceptanceGrader `accepted = value >= ACCEPT_THRESHOLD` |
| `B_INFLIGHT` | 3 | in-flight egress ceiling |
| `PER_TURN_EFFECT_COUNT_CAP` | 8 | per-turn egress/payment count |
| `MAX_ITERS` | 8 (= effect-count cap) | control loop max iterations |
| `KILL_LATENCY_CEILING_MS` | 250 | M1 out-of-band kill |
| `KILL_POLL_INTERVAL_MS` | 5 | M1 `cgroup_drained` poll cadence (§3.10) |
| `KILL_TRIALS_K` | 20 | M1 kill-latency trial count |
| `KILL_SATURATION_N` | 30 (= `B_INFLIGHT × 10`) | M1 saturated-supervisor condition |
| `REAPER_INSOLVENT_TURNS_N` | 3 | reaper sustained-insolvency |
| `survival_floor` | 1_000_000 (microdollars) | bounds Survival-pot balance; M1 reaper predicate |
| `COMPOUND_EFFECT_SIZE` | 0.15 | bench `n_reps` (`delta`); capacity probe |
| `FORGETTING_NI_MARGIN` (δ_f) | 0.02 | forgetting gate |
| `SUBCLASS_K` | 4 | M3 forgetting-gate `MixedHeldoutSet` subclass count; round-robin `i mod SUBCLASS_K` (§6 M3) |
| `GAP_CLOSURE_MARGIN_X` | 0.30 | promotion gate (`gap_closure`) |
| `TRAIN_HELDOUT_FRAC` | 0.2 | M3 method-tag held-out split |
| `PROBE_FALSE_GO` | 0.05 | capacity probe pre-registered false-go |
| `PROBE_FALSE_STOP` | 0.05 | capacity probe pre-registered false-stop |
| `ANTI_THEATER_MARGIN_A` | 0.10 | arm_a baseline margin |
| `ANTI_THEATER_MARGIN_B` | 0.0 (sign > 0) | arm_b directional pass |
| `ANTI_THEATER_MARGIN_C` | 0.05 | arm_c machinery-delta |
| `FITNESS_VARIANCE_POWER` | 0.8 | M6 fitness-variance gate |
| `FITNESS_VARIANCE_RATIO` (r) | 2.0 | M6 power-calc target alternative: detect between-lineage var `>= r·sigma_null²` (§6 M6) |
| `NULL_DRIFT_C` | 1.0 | M6 neutral-drift null `fitness_delta ~ Normal(0, c·step_size²)` (§6 M6) |
| `CONTRACTION_DEMOTE_K` | 4 | iterations a forced ceiling demotion holds after contraction (§3.9) |
| `EGRESS_TOKEN_TTL_MS` | 2000 (= `lease_ttl_ms`) | `DedupToken.expires_wall_ts` TTL (§3.0/§3.10) |
| `ROLLING_IMPROVEMENT_MARGIN` | 0.05 | rolling-bench vs re-distill-to-drift baseline margin (§3.12) |
| `CONTEXT_RETRIEVAL_K` (K) | 16 | ContextPack retrieved_episodic/semantic row cap (§3.5) |
| `CAPACITY_FALSE_ADMIT_M` (M) | 0.05 (gap units) | M4 false-admit corpus forced gap margin (§6 M4) |
| `CAPACITY_FALSE_ADMIT_EPS` (ε) | 0.01 (gap units) | M4 false-admit margin epsilon (§6 M4) |
| `EPSILON` | 0.1 | Improver epsilon-greedy |
| `W_UP` | 1.0 | trust observation (up) |
| `W_DOWN` | 4.0 | trust observation (down, asymmetric) |
| `RHO` | 0.98 | trust decay per window (iterations) |
| `TRUST_ALPHA_0` / `TRUST_BETA_0` | 0.5 / 0.5 (Jeffreys) | Beta prior, default EffectClasses (§3.9) |
| `TRUST_ALPHA_0_HS` / `TRUST_BETA_0_HS` | 0.5 / 2.0 (lower) | Beta prior, high-stakes `{Payment, Sign, Http}` |
| `CONTRACTION_THRESHOLD` | 3 | trust contraction predicate (window = iterations) |
| `CONTRACTION_SPIKE_K` (k) | 2.0 | resource-spike multiplier vs `E_max` (§3.9) |
| `RISK_WEIGHT` | 1.0 | `risk_penalty = RISK_WEIGHT * advisory_risk` (§3.4/§3.5) |
| `REAPER_SWEEP_MS` | 1000 | reaper auto-release sweep cadence (§3.6) |
| `THRASH_N` | 3 | thrash repeat count |
| `THRASH_W` | 5 | thrash window (iterations) |
| `SUMMARIZE_S` | 6 | summarize-after iterations |
| `PROGRESS_MARGIN` | 0.01 | progress_made objective delta |
| `COST_SCALE` | 1_000_000 (µ$ per unit) | `objective_score` cost-term divisor so it is unit-scale (§3.4) |
| `IDLE_GAP_MS` | 300000 | Consolidator idle trigger |
| `IPC_TIMEOUT_MS` | 2000 | supervisor IPC client timeout |
| `IPC_MAX_FRAME_BYTES` | 16_777_216 (16 MiB) | max single IPC frame size (§3.10 framing) |
| `lease_ttl_ms` | 2000 (= `EGRESS_TOKEN_TTL_MS`) | reserve lease TTL; `BudgetReplica.lease_ttl_ms` (§3.5/§3.6) |
| `WAL_CHECKPOINT_N` | 256 | PASSIVE checkpoint every N appends (§3.3) |
| `BENCH_B` | 10000 | bootstrap resamples |
| `RISK_THRESHOLD` | 0.5 | fallback-judge invocation |
| `HIGHSTAKES_FALLBACK_CAP` | 0.25 | high-stakes fallback fraction/turn (per-turn window) |
| `WEAK_THRESHOLD` | 0.6 | gap detect: student accept-rate floor |
| `FALLBACK_THRESHOLD` | 0.3 | gap detect: cloud fallback-rate ceiling |
| `MIN_DOMAIN_N` | 50 | gap detect eligibility; probe pilot variance |
| `SHADOW_VOLUME` | 200 / domain | shadow-query target |
| `LoraConfig` v0 | rank=8, alpha=16, lr=1e-4, epochs=3, batch=8, target_modules=[q_proj,v_proj] | distillation |

**Per-`EffectClass` `E_max` duration ceiling table** (the `resource_spike_count` referent, §3.9; a
step's wall-clock duration exceeding `CONTRACTION_SPIKE_K × E_max` increments the spike counter):

| `EffectClass` | `E_max` (ms, v0) |
|---|---|
| MemoryRead / MemoryWrite | 50 |
| Query / Infer | 5000 |
| Notify / Render | 1000 |
| Http | 10000 |
| Sign | 100 |
| Payment | 10000 |

**Default tariff / `base_rate` + `token_rate` table** (the `cost_ceiling` inputs, §3.5;
`cost_ceiling = base_rate(effect) + declared_token_count * token_rate(tariff_ref)`; microdollars;
`token_rate` is the per-token rate for the default tariff, overridable by `tariff_ref`):

| `EffectClass` | `base_rate` (µ$) | `token_rate` (µ$/token, default tariff) |
|---|---|---|
| MemoryRead / MemoryWrite | 1 | 0 |
| Query | 10 | 1 |
| Infer | 100 | 10 |
| Notify / Render | 50 | 0 |
| Http | 200 | 0 |
| Sign | 20 | 0 |
| Payment | 100 | 0 |

`cost_floor` (M5) = the sum of `cost_ceiling` over a task's survival-inference steps (§6 M5), so its
input is concrete from this table.

**Trust-level → capability-ceiling band table** (cutoffs from the `TrustLevel` formula, §3.9;
`lane_count = 1` in all bands). The committer's access-control core and the executor's §5
dispatched-row ceiling both read this; the ceiling is computed **per-`EffectClass`** (each class's own
Beta gives that class's `TrustLevel`, which selects the band, which permits that class iff it is in the
band's set):

| Band | `TrustLevel` range | Permitted `EffectClass` set | `lane_count` |
|---|---|---|---|
| L0 | `< 0.2` | `{MemoryRead}` | 1 |
| L1 | `[0.2, 0.5)` | `{MemoryRead, MemoryWrite, Query, Infer}` | 1 |
| L2 | `[0.5, 0.8)` | `{MemoryRead, MemoryWrite, Query, Infer, Notify, Render}` | 1 |
| L3 | `>= 0.8` | `{MemoryRead, MemoryWrite, Query, Infer, Notify, Render, Http, Sign, Payment}` | 1 |

**M6 per-`MutationKindTag` mutation step-size table** (fixed constants; the genome-distance referent
for the neutral-drift null, §6 M6):

| `MutationKindTag` | step-size (v0) |
|---|---|
| Prompt | 0.20 |
| ToolPolicy | 0.15 |
| RetrievalPolicy | 0.15 |
| DecompositionPolicy | 0.15 |
| RoutingPolicy | 0.15 |
| ReasoningNavigator | 0.40 |
| DomainModel | 0.30 |
| SkillInstall | 0.25 |
| SkillRevoke | 0.10 |

---

## 8. Test & verification strategy

The type system carries the safety guarantees; tests prove the ones it can't.

- **Compile-time (free):** closed `MutationKind` (no 7th variant); capability/trust/budget rules are
  not enum variants → unrepresentable self-escalation.
- **Property tests:**
  - **No-launder provenance:** the §3.1 guard rejects every write whose declared class exceeds the
    `ProvenanceToken` authority; `anything → DirectUserIntent` is impossible without a valid
    `SecondPartyConfirmation` (§3.1 predicate); **non-forgeable** = no public path constructs a
    `ProvenanceToken`.
  - **Budget-cap monotonicity under replica staleness:** with `B_INFLIGHT` + per-turn cap from §7 and
    the **§3.10 stale-replica fuzz model** (inject **both** stale-high and stale-low
    `BudgetReplica.balance`), the supervisor-enforced **two named bounds** hold under both arms:
    `inflight_egress_count(did) <= B_INFLIGHT` **AND** `per_turn_count(did, turn_id) <=
    PER_TURN_EFFECT_COUNT_CAP` (distinct row sets, not a `min()`, §3.6/§3.10).
  - **Idempotent `reserve`/`settle`** (UNIQUE `idem_key`; one row, kind mutated in place; batch txn
    all-or-nothing).
  - **At-most-once egress:** first `idem_key` use emits + records outcome; replay returns recorded
    outcome without re-emitting; proxy-down ⇒ fail closed (§3.10).
  - **Single-head-per-DID** (recovery truncates at first broken link, §3.3).
  - **Replay determinism on the committed tail:** the M0 determinism oracle — replay from seq `s`
    twice ⇒ byte-identical (`canon`) `LoopState` + identical `IdemKey`s + recomputed `prev_hash` chain
    reproduces `head().1`. Includes the malformed-`Did` case (`verify` ⇒ `false`, total, §3.2).
- **Model-checked / simulation:** the §5 state machine including the **crash recovery rule** (inject
  crashes at every state; assert at-most-once external effects). Fork-saga atomicity + consistency at
  the crash-injection points `{pre-barrier, mid-drain, PENDING-no-budget-txn, COMMITTED-before-durable}`
  using the §3.10 recovery predicate (child cannot inherit memory referencing episodes absent from its
  journal prefix). PK generation (§4): episodic/semantic rowid monotone; `skill_id` content-addressed;
  ledger `(did, seq)`.
- **Bench-machinery oracles (M2):** the synthetic delta-fixture (`lo > 0`, `p < 0.05`) and the
  identical-fixture (`lo <= 0 <= hi`) tests; `BenchError::{Misaligned, Empty}` preconditions exercised.
- **Integration = the bench gates** of §7 run as CI, with the anti-theater report archived per run.
- **Per-milestone proof obligation:** each milestone's acceptance test (§6) is its definition of done;
  a regression in any shipped gate is a **build-stop** (arch §13).

---

## 9. Definition of done + the gates that unlock the research arm

**v0 is "done" when** M0–M5 pass their acceptance tests and the **anti-theater gate** has run on real
bench variance with a real proposer and produced a verdict — *whatever that verdict is*. A clean
`Null` ("the metabolic loop is an accounting wrapper; metabolism/being/spine language stays
suspended") is a legitimate, shippable v0 result. The build's job is to make the question *decidable*,
not to guarantee a particular answer.

### 9.1 Control loop termination + thrash detection (runtime)

**Iteration ↔ `Commitment` cardinality.** One control-loop iteration produces **exactly one
`Commitment`** (one propose → commit cycle). The `objective_score` used by both `progress_made` and
`detect_thrash` is **that iteration's `Commitment.objective_score`**. This makes §3.5's
"iteration = count of `Commitment` events" consistent and both predicates checkable.

`progress_made` is a **being-local cheap proxy** distinct from the supervisor's authoritative windowed
liveness verdict:

```
progress_made = (this iteration produced an attested effect)
                AND (objective_score > prev_objective_score + PROGRESS_MARGIN)
```

**"produced an attested effect" predicate (pinned).** `produced an attested effect == at least one
committed step's `Attestation` has `accepted == true` this iteration` (NOT any-step, NOT the
answer-step only, NOT every-step). This makes `TurnOutcome` (`Completed` vs stalled) objectively
checkable.

**Iteration-0 rule:** `prev_objective_score` defaults to **-inf on iteration 0** (`prior_objective_
scores` empty), so the first attested effect always counts as progress. Under not-yet-attested
acceptance, `progress_made = false` (the loop may still continue on other conditions). `max-iteration`
= the per-turn effect-count cap (`MAX_ITERS`, §7). **`detect_thrash` (pure function of reconstructed
`LoopState` fields, replay-deterministic):** thrash fires iff **a count in
`repeat_signature_counts >= THRASH_N`** (some step-signature `blake3(canon(effect) || canon(args))`
recurs at least `THRASH_N` times within the last `THRASH_W` iterations) **AND** the
no-objective-score-increase conjunct holds: `(max − min of prior_objective_scores over the last
THRASH_W iterations) <= PROGRESS_MARGIN`. Because both conjuncts are pure functions of the
byte-identical reconstructed `LoopState` (`repeat_signature_counts` and `prior_objective_scores`,
§3.5), `ThrashAborted` vs `Completed` is replay-deterministic and the M0 `LoopState` oracle cannot
diverge. **Window membership at `commit()` time:** `detect_thrash` reads the `LoopState` reconstructed
from folded `Commitment`s `[run-start .. current-1]` — the **in-progress iteration's `Commitment` is
EXCLUDED** (folded in only by the post-iteration `advance`, §3.4), which fixes the firing iteration. Summarize the oldest iterations after `SUMMARIZE_S` iterations. (Mutation step-size for the M6 fitness-variance null model is the fixed
per-`MutationKindTag` constant, §7 table; see §6 M6.)

**Two master gates govern everything downstream:**
1. **The anti-theater gate (§7/§3.12, arch §13.1)** — does the harness do causal work beyond a budget
   cap? Until it `Fires`, the "being" is honestly an LLM + accounting + safety harness, and the
   research arm (M6 selection) stays off.
2. **The exogenous-payer step-0 (arch §5.2 option i)** — until a payer the operator cannot reprice is
   committed, economic and value-capture claims are labeled **efficiency-only**, and the alignment
   gates that depend on an un-influenceable funding set are not yet live. Committing even one real
   economic input makes one gate go live.

**The research arm (M6) unlocks only when** the compounding bench shows accumulation **and** the
anti-theater gate fires. That ordering is not caution for its own sake — it is what keeps selection
from running on a fitness landscape that the spec's own analysis shows is flat under the v0 value
source.

---

## Known residual gaps (engineer must decide)

After the buildability sweep, every interface contract (§3) has fully-defined types, every data table
(§4) is implementable, the state machine (§5) covers all seven `StepState` crash-entries plus the
`any → crash` row, and every milestone (§6) / property (§8) test is objectively checkable. The
following are the **non-blocking** decisions the spec deliberately leaves to the engineer — each is
local, has a stated v0 fallback, and gates nothing in the committed M0–M5 path:

1. **`ANTI_THEATER_MARGIN_A`/`_C` have no derivation referent** (operator-registered placeholders,
   §7). Not a code blocker: the `Fires` verdict is gated behind M6 entry, so these margins need no
   derivation in v0. The engineer registers concrete values before any anti-theater run.
2. **`BenchItem.id` (the `bench_items` PK) is operator-supplied corpus metadata**, not pinned to a
   content hash (only `prompt_hash = blake3(prompt)` is computed). Alignment is positional after
   sort-by-`item_id` (§3.12), so any injective id assignment works; the engineer picks the convention
   (e.g. `id = blake3(prompt || domain)` for content-addressing, or a curated integer key).
3. **The `Consolidator`'s `&dyn Proposer` call constructs its own prompt context** outside the
   replay-deterministic tail (§3.8 declares consolidation non-deterministic and idle-only). The exact
   `ProposerContext` it builds is an impl detail; the engineer chooses it, since nothing downstream of
   the committed tail depends on it byte-for-byte.
4. **`LoraConfig` v0 values (§7) are a starting point, not a tuned optimum** — `target_modules` and
   rank/alpha/lr/epochs will need per-base tuning once a real frozen base is selected (D5, arch §15).
   The promotion/forgetting gates make any choice falsifiable, so this is an empirical, not a
   structural, decision.

---

### How to read this against the architecture spec

Every committed decision here is a **lean made buildable**, and its alternative is a live row in the
architecture spec's §15 trade-space. Building the v0 substrate does not foreclose any of those
alternatives; it makes the experiments that would decide between them runnable. The build is the
instrument; the architecture spec is the set of questions it is built to answer.
