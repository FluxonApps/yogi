//! Foreground full-integration capstone (loads qwen3:8b). Run:
//!   cargo run -p being-bench --bin full_being --release
//!
//! Proves the seams built this session compose into ONE being: a live being that is simultaneously
//!   • **durable** — its signed journal is a `being_colony::DurableJournal` (survives restart, §5),
//!   • **sandboxed** — its executor is a `being_runtime::SandboxedExecutor` (M4 capability broker,
//!     deny-by-default, fail-closed on the live turn path),
//!   • **model-backed** — its proposer is the real Ollama/qwen3 chat proposer,
//! on the metabolic turn loop (supervisor reserve/attest, signed hash-chain). It runs a few turns,
//! then reopens the durable being to show the journal recovered from disk and still verifies.

use being_bench::default_frozen_suite;
use being_colony::durable_being;
use being_core_economy::Account;
use being_core_id::Ed25519Signer;
use being_proposer_openai::{OpenAiChatConfig, OpenAiChatProposer};
use being_runtime::{EchoExecutor, PassThroughCommitter, SandboxedExecutor};
use being_sandbox::{Capability, CapabilitySet};
use being_supervisor::Supervisor;
use std::collections::BTreeSet;

fn main() {
    eprintln!("Full integrated being (foreground — loads qwen3:8b) ...");
    let path = std::env::temp_dir().join("yogi_full_being.journal");
    let _ = std::fs::remove_file(&path);

    // Capabilities: allow pure responses (always) + egress to one host; everything else denied.
    let caps = CapabilitySet::granted([Capability::Egress {
        hosts: BTreeSet::from(["api.allowed.test".to_string()]),
    }]);

    let suite = default_frozen_suite();
    let turns;
    {
        let sup = Supervisor::new(Account::new(1_000_000_000, 0, 1_000_000_000), i64::MAX, 0);
        let mut being = durable_being(
            &path,
            Ed25519Signer::from_seed([11; 32]),
            Supervisor::as_port(&sup),
            OpenAiChatProposer::new(OpenAiChatConfig::ollama_qwen3()),
            PassThroughCommitter,
            SandboxedExecutor::new(EchoExecutor, caps), // capability-gated executor
        )
        .unwrap();

        for (i, t) in suite.iter().take(2).enumerate() {
            let turn = being.turn(&t.prompt, i as i64);
            println!(
                "turn {i}: acted={} obs={:?}",
                turn.acted,
                turn.observations
                    .iter()
                    .map(|o| o.chars().take(48).collect::<String>())
                    .collect::<Vec<_>>()
            );
        }
        turns = being.journal_len();
        println!(
            "\ndurable+sandboxed being: journal_len={} verifies={}",
            being.journal_len(),
            being.journal_verifies(),
        );
    }

    // Reopen → the being's signed journal recovered from disk and still verifies (crash recovery).
    let sup = Supervisor::new(Account::new(1_000_000_000, 0, 1_000_000_000), i64::MAX, 0);
    let recovered = durable_being(
        &path,
        Ed25519Signer::from_seed([11; 32]),
        Supervisor::as_port(&sup),
        OpenAiChatProposer::new(OpenAiChatConfig::ollama_qwen3()),
        PassThroughCommitter,
        SandboxedExecutor::new(EchoExecutor, CapabilitySet::none()),
    )
    .unwrap();
    println!(
        "after restart: journal_len={} (was {turns}) verifies={}",
        recovered.journal_len(),
        recovered.journal_verifies()
    );
    println!("\n→ one being: durable (signed chain survived restart) + sandboxed (capability-gated) + model-backed.");
    std::fs::remove_file(&path).ok();
}
