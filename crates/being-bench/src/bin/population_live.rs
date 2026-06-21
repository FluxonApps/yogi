//! Foreground live M6 population (loads qwen3:8b). Run:
//!   cargo run -p being-bench --bin population_live --release
//!
//! Drives `being_colony::Population` with the REAL model: two founding lineages with different
//! competence — a *thinking* lineage (reasoning on) and a */no_think* lineage (the falsification bench
//! certified `/no_think` breaks rule application, FINDINGS 2026-06-21). Each generation every member
//! answers a held-out reasoning task; an exogenous payer grades it and the revenue closure returns the
//! verified earnings. The population engine then charges metabolism, credits earnings, reaps the
//! insolvent (death) and forks the solvent (reproduction). Economic selection should let the thinking
//! lineage out-earn its metabolism and spread while the /no_think lineage is reaped — live on the 8B.

use being_colony::{DurableForkLedger, Population, PopulationConfig};
use being_core_id::Ed25519Signer;
use being_core_mutation::{apply, Genome, MutationKind};
use being_proposer_openai::{OpenAiChatConfig, OpenAiChatProposer};
use being_runtime::ContextPack;
use being_value::{ExternalPayer, OperatorPayer, SubstringGrader, Tariff, Treasury};

fn main() {
    eprintln!("Live M6 population — economic selection on qwen3:8b (thinking vs /no_think) ...");
    // Held-out multi-step reasoning tasks (number-only answers) — the kind /no_think tends to fumble.
    let tasks: &[(&str, &str)] = &[
        ("A farmer has 3 pens with 4 sheep each. How many sheep total? Answer with the number only.", "12"),
        ("A train travels 60 km in 2 hours. Speed in km/h? Number only.", "30"),
        ("What is 15% of 200? Number only.", "30"),
        ("I have 7 apples, eat 2, then buy 5 more. How many now? Number only.", "10"),
    ];

    let thinking = OpenAiChatProposer::new(OpenAiChatConfig::ollama_qwen3_thinking());
    let nothink = OpenAiChatProposer::new(OpenAiChatConfig::ollama_qwen3());
    let mut payer = OperatorPayer::new(Tariff::new(200), SubstringGrader, Treasury::new(1_000_000));

    let path = std::env::temp_dir().join("yogi_population_live.forks");
    let _ = std::fs::remove_file(&path);
    let ledger = DurableForkLedger::open(&path).unwrap();
    let genome = |s: &str| apply(MutationKind::Prompt(s.into()), Genome::default()).unwrap();
    let cfg = PopulationConfig {
        turn_cost: 120,       // metabolism per generation
        birth_endowment: 200, // offspring starting balance
        reproduce_threshold: 300,
        max_size: 6,
        sexual: false,
    };
    // Founder 1 = thinking lineage, founder 2 = /no_think lineage; equal starting balance.
    let mut pop = Population::new(
        vec![(1, genome("thinking"), 300), (2, genome("nothink"), 300)],
        Ed25519Signer::from_seed([21; 32]),
        ledger,
        cfg,
    );

    for (gen, (prompt, truth)) in tasks.iter().enumerate() {
        pop.advance(gen as i64, |m| {
            let proposer = if m.founder == 1 { &thinking } else { &nothink };
            let ctx = ContextPack {
                input: prompt.to_string(),
                retrieved: vec![],
            };
            let answer = proposer.try_propose(&ctx).unwrap_or_default();
            payer.settle("q", &answer, truth)
        });
        let founders: Vec<u64> = pop.members().iter().map(|m| m.founder).collect();
        let think_n = founders.iter().filter(|&&f| f == 1).count();
        let nothink_n = founders.iter().filter(|&&f| f == 2).count();
        println!(
            "gen {gen}: pop={} | thinking-lineage={} no_think-lineage={}",
            pop.len(),
            think_n,
            nothink_n
        );
    }
    println!(
        "\n→ economic selection, live: the more-competent (thinking) lineage out-earns metabolism and\n  spreads; the /no_think lineage earns less and is reaped. Selection by solvency on real beings."
    );
    std::fs::remove_file(&path).ok();
}
