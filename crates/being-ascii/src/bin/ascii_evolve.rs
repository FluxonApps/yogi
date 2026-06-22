//! Foreground live ASCII evolution (loads qwen3:8b + spends `claude -p` judge calls). Run:
//!   cargo run -p being-ascii --bin ascii_evolve --release
//! Watch it live in another terminal: ./scripts/status.sh
//!
//! qwen3:8b draws (local), Claude judges quality (`claude -p`, the metered salary), and `illuminate`
//! evolves the drawing genomes one generation at a time. After each generation it streams progress to
//! `.yogi/ascii_evolve.tsv` + the current best drawing to `.yogi/ascii_best.txt`, which the status card
//! renders — so the quality evolution (or its plateau when salary runs out) is visible in real time.
use being_ascii::{AsciiEvaluator, AsciiVariator, ClaudeCliRunner, ClaudeJudge, OllamaGenerator};
use being_core_mutation::Genome;
use being_lineage::{illuminate, Archive, BehaviorDescriptor, IlluminationConfig};
use std::fs;
use std::io::Write;

fn main() {
    let subjects = vec!["cat".to_string(), "house".to_string()];
    let salary: u64 = 14; // hard cap on `claude -p` judge calls — bounds subscription spend
    let gens = 6;

    let tsv = ".yogi/ascii_evolve.tsv";
    let best_txt = ".yogi/ascii_best.txt";
    let _ = fs::create_dir_all(".yogi");
    let _ = fs::write(
        tsv,
        "gen\tbest\tmean\tqd\tniches\tsalary_used\tsalary_cap\n",
    );
    let _ = fs::write(best_txt, "(no drawing yet)\n");

    eprintln!(
        "live ASCII illumination: {gens} generations x {} subjects; salary cap {salary} Claude calls...",
        subjects.len()
    );

    // Shared flywheel store: the generator few-shots from the being's own Claude-validated best
    // drawings (≥ 0.4), so good work feeds the next generation. Threshold 0.4 sits just above the
    // prompt-only plateau (0.30) so only genuinely-better drawings ratchet in.
    let store = being_ascii::ExemplarStore::shared(0.4, 3);
    let mut evaluator = AsciiEvaluator::with_store(
        OllamaGenerator::with_store(store.clone()),
        ClaudeJudge::new(ClaudeCliRunner, salary),
        subjects.clone(),
        store.clone(),
    );
    let mut variator = AsciiVariator::default();
    let descriptor = BehaviorDescriptor::bounded([(0.0, 1.0, 4), (0.0, 1.0, 4)]).unwrap();
    let mut archive = Archive::new();

    for gen in 0..gens {
        // One MAP-Elites step per generation (advancing seed so each generation samples differently).
        let cfg = IlluminationConfig::new(1, 1000 + gen as u64);
        illuminate(
            &mut archive,
            &descriptor,
            Genome::default(),
            1,
            &mut evaluator,
            &mut variator,
            &cfg,
            None,
        );

        let best = archive.best().map(|e| e.fitness).unwrap_or(0.0);
        let mean = archive.mean_fitness().unwrap_or(0.0);
        if let Ok(mut f) = fs::OpenOptions::new().append(true).open(tsv) {
            let _ = writeln!(
                f,
                "{gen}\t{best:.3}\t{mean:.3}\t{:.3}\t{}\t{}\t{salary}",
                archive.qd_score(),
                archive.len(),
                evaluator.judge.calls_made
            );
        }
        if let Some(b) = &evaluator.best_sample {
            let _ = fs::write(
                best_txt,
                format!("score={:.2} subject={}\n{}\n", b.score, b.subject, b.art),
            );
        }
        println!(
            "gen {gen}: best={best:.2} mean={mean:.2} niches={} salary={}/{salary} learned={}",
            archive.len(),
            evaluator.judge.calls_made,
            store.borrow().learned_count()
        );
    }
    println!(
        "\nDone. flywheel learned {} drawings (best {:?}). Live dashboard: ./scripts/status.sh",
        store.borrow().learned_count(),
        store.borrow().best_learned_score()
    );
}
