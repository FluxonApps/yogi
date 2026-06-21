//! `yogi` — the deployable entrypoint for the local Yogi build (build-spec §2).
//!
//! `yogi status` summarizes what's built (no model, instant). `yogi run [turns]` drives a real being —
//! durable signed journal + capability sandbox + the local qwen3 proposer, on the metabolic turn loop —
//! for a few turns (foreground; loads the model). Pure-std arg parsing, no extra deps.

use being_colony::durable_being;
use being_core_economy::Account;
use being_core_id::Ed25519Signer;
use being_proposer_openai::{OpenAiChatConfig, OpenAiChatProposer};
use being_runtime::{EchoExecutor, PassThroughCommitter, SandboxedExecutor};
use being_sandbox::CapabilitySet;
use being_supervisor::Supervisor;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("status") => status(),
        Some("run") => {
            let turns = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(3);
            run(turns);
        }
        Some("version") => println!("yogi {}", env!("CARGO_PKG_VERSION")),
        _ => usage(),
    }
}

fn usage() {
    eprintln!(
        "yogi — a trust-native, self-evolving being (local build)\n\n\
         USAGE:\n  \
         yogi status        what's built (no model, instant)\n  \
         yogi run [turns]   run a durable, sandboxed being on qwen3:8b (foreground)\n  \
         yogi version"
    );
}

fn status() {
    println!("Yogi — local build");
    let rows = [
        ("identity", "Ed25519 + W3C did:key (did:key:z6Mk…)"),
        ("journal", "blake3 hash-chain, signed, durable (survives restart)"),
        ("economy", "i64 microdollars, overflow-checked, reaper-enforced"),
        (
            "policy",
            "static risk ceiling + dynamic Beta trust model (earn-slow/lose-fast)",
        ),
        (
            "isolation",
            "capability broker + real wasm32 executor guest (zero ambient authority)",
        ),
        ("memory", "episodic + semantic (embeddings) + signed skills"),
        (
            "distillation",
            "token-space (live) + weight/LoRA (foreground), gap→promote",
        ),
        (
            "evolution",
            "MAP-Elites illumination + live economic population (mutation+crossover+selection+death)",
        ),
        ("value", "exogenous payer wired to metabolism — earns its keep"),
    ];
    for (k, v) in rows {
        println!("  {k:<12}: {v}");
    }
    println!("\nRun `yogi run` to drive a live being on the local model.");
}

fn run(turns: usize) {
    eprintln!("yogi run — durable + sandboxed being on qwen3:8b ({turns} turns, foreground)…");
    let path = std::env::temp_dir().join("yogi_run.journal");
    let _ = std::fs::remove_file(&path);
    let prompts = [
        "What is 6 times 7? Answer with the number only.",
        "What is the capital of France? One word.",
        "Name a primary color. One word.",
        "What is 100 minus 1? Number only.",
    ];

    let sup = Supervisor::new(Account::new(1_000_000_000, 0, 1_000_000_000), i64::MAX, 0);
    // No capabilities granted: the being is fail-closed for external effects; pure responses pass.
    let mut being = match durable_being(
        &path,
        Ed25519Signer::from_seed([1u8; 32]),
        Supervisor::as_port(&sup),
        OpenAiChatProposer::new(OpenAiChatConfig::ollama_qwen3()),
        PassThroughCommitter,
        SandboxedExecutor::new(EchoExecutor, CapabilitySet::none()),
    ) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("failed to open durable being: {e}");
            std::process::exit(1);
        }
    };

    for (i, p) in prompts.iter().take(turns).enumerate() {
        let turn = being.turn(p, i as i64);
        let obs: Vec<String> = turn
            .observations
            .iter()
            .map(|o| o.chars().take(60).collect())
            .collect();
        println!("turn {i}: acted={} {obs:?}", turn.acted);
    }
    println!(
        "\njournal: {} entries, verifies={} (durable — survives restart)",
        being.journal_len(),
        being.journal_verifies()
    );
    std::fs::remove_file(&path).ok();
}
