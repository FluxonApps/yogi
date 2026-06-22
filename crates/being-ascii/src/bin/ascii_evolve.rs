//! Foreground live ASCII evolution (loads qwen3:8b + spends `claude -p` judge calls). Run:
//!   cargo run -p being-ascii --bin ascii_evolve --release
//!
//! qwen3:8b draws (local), Claude judges quality (`claude -p`, the metered salary), and `illuminate`
//! evolves the drawing genomes (prompt + exemplar skills) across a subject×style niche. The salary cap
//! hard-bounds Claude spend: once exhausted, candidates can't afford judging (score 0) — the reaper
//! pressure in microcosm.
use being_ascii::{
    AsciiArt, AsciiEvaluator, AsciiVariator, ClaudeCliRunner, ClaudeJudge, Generator,
    OllamaGenerator,
};
use being_core_mutation::Genome;
use being_lineage::{illuminate, Archive, BehaviorDescriptor, IlluminationConfig};

fn main() {
    let subjects = vec!["cat".to_string(), "house".to_string()];
    let salary: u64 = 14; // hard cap on `claude -p` judge calls — bounds subscription spend
    let iters = 6;
    eprintln!(
        "live ASCII illumination: {iters} iters x {} subjects; salary cap {salary} Claude calls...",
        subjects.len()
    );

    let mut evaluator = AsciiEvaluator::new(
        OllamaGenerator::new(),
        ClaudeJudge::new(ClaudeCliRunner, salary),
        subjects.clone(),
    );
    let mut variator = AsciiVariator::default();
    let descriptor = BehaviorDescriptor::bounded([(0.0, 1.0, 4), (0.0, 1.0, 4)]).unwrap();
    let mut archive = Archive::new();
    let cfg = IlluminationConfig::new(iters, 7);

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

    println!("\n=== result ===");
    println!("niches filled: {}", archive.len());
    println!(
        "qd_score: {:.3}   mean_fitness: {:?}",
        archive.qd_score(),
        archive.mean_fitness()
    );
    println!(
        "salary spent (Claude judge calls): {}/{}   frontier_microdollars: {}",
        evaluator.judge.calls_made, salary, evaluator.judge.spent.frontier_microdollars
    );

    if let Some(best) = archive.best() {
        println!(
            "\nbest fitness {:.2}  prompt={:?}  skills={:?}",
            best.fitness, best.genome.prompt, best.genome.installed_skills
        );
        let mut g = OllamaGenerator::new();
        for subj in &subjects {
            let art = AsciiArt::parse(&g.generate(&best.genome, subj));
            println!("\n--- best genome draws '{subj}' ---\n{}", art.render());
        }
    }
}
