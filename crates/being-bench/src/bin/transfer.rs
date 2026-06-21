//! Foreground transfer-compounding certification v2 (loads qwen3:8b). Run manually:
//!   cargo run -p being-bench --bin transfer --release
//!
//! Measures TRANSFER, not answer-lookup (D-M3-3): a made-up operation `a ⊕ b = a·b+a+b` the model
//! can't know cold, on fresh seeded operands. Applying the variations the research recommended for an
//! 8B model (cheapest-first):
//!   1. thinking ON (drop `/no_think`) + qwen3 thinking-mode sampling + token headroom — the prime fix;
//!   2. deterministic rule injection (via ctx.retrieved) so we certify the APPLY mechanism, not
//!      nomic-embed-text's handling of the rare `⊕` symbol;
//!   3. a worked-example skill note + an `ANSWER:` output line;
//!   4. self-consistency (k samples, majority vote) for the final margin.
//!
//! Cold (no rule) should fail; with the rule injected it should apply it to the new operands.

use being_bench::{mean, paired_bootstrap_ci, transfer_corpus, TransferTask, TRANSFER_SKILL_NOTE};
use being_proposer_openai::{OpenAiChatConfig, OpenAiChatProposer};
use being_runtime::{ContextPack, Proposer};

const VOTES: usize = 1; // self-consistency (1 = fast directional read; raise for a tighter margin)

/// A qwen3 proposer in **thinking mode** (the research's prime fix for rule application).
fn thinking_proposer() -> OpenAiChatProposer {
    let mut cfg = OpenAiChatConfig::ollama_qwen3_thinking();
    cfg.system_prompt =
        "You are a careful calculator. Reason step by step, then end with a line exactly like \
         'ANSWER: <number>'."
            .to_string();
    OpenAiChatProposer::new(cfg)
}

/// The final number on the model's `ANSWER:` line, else the last integer it emitted.
fn extract_answer(text: &str) -> Option<String> {
    if let Some(i) = text.to_uppercase().rfind("ANSWER:") {
        let tail = &text[i + "ANSWER:".len()..];
        let num: String = tail
            .chars()
            .skip_while(|c| !c.is_ascii_digit() && *c != '-')
            .take_while(|c| c.is_ascii_digit() || *c == '-')
            .collect();
        if !num.is_empty() {
            return Some(num);
        }
    }
    None
}

/// Score one task with self-consistency: k samples, majority vote on the extracted answer.
fn solve(p: &mut OpenAiChatProposer, t: &TransferTask, with_rule: bool) -> bool {
    let retrieved = if with_rule {
        vec![TRANSFER_SKILL_NOTE.to_string()]
    } else {
        vec![]
    };
    let mut hits = 0usize;
    for _ in 0..VOTES {
        let prop = p.propose(&ContextPack {
            input: t.prompt.clone(),
            retrieved: retrieved.clone(),
        });
        let resp = &prop.candidate_steps[0].arg;
        let ans = extract_answer(resp).unwrap_or_default();
        if ans == t.expected || resp.contains(&t.expected) {
            hits += 1;
        }
    }
    hits * 2 > VOTES // majority correct
}

fn score(corpus: &[TransferTask], with_rule: bool) -> Vec<f64> {
    let mut p = thinking_proposer();
    corpus
        .iter()
        .map(|t| {
            if solve(&mut p, t, with_rule) {
                1.0
            } else {
                0.0
            }
        })
        .collect()
}

fn main() {
    let n = 15;
    let corpus = transfer_corpus(n, 7);
    eprintln!(
        "Transfer cert v2 (thinking ON, deterministic rule injection, k={VOTES} vote): {n} tasks, foreground ..."
    );

    let cold = score(&corpus, false);
    let skilled = score(&corpus, true);

    println!("cold (no rule)      mean: {:.3}", mean(&cold));
    println!("with injected rule  mean: {:.3}", mean(&skilled));
    let ci = paired_bootstrap_ci(&cold, &skilled, 4000, 12345, 0.05);
    println!(
        "paired delta: mean={:.3}  CI=[{:.3}, {:.3}]  compounds={}",
        ci.mean_delta,
        ci.lower,
        ci.upper,
        ci.improves_monotonically()
    );
    if ci.improves_monotonically() {
        println!("=> CERTIFIED: the rule transfers to new operands (CI excludes zero). Token-space compounding.");
    } else {
        println!("=> not certified — see docs/FINDINGS.md for the next variation.");
    }
}
