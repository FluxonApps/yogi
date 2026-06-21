//! being-bench: the falsification harness (build-spec §7; architecture §13).
//!
//! The *machinery* lives here and is pure + deterministic — no model, no network — so the automated
//! loop can test it: a frozen task suite, substring scoring, a paired-bootstrap CI for the
//! compounding gate, and the anti-theater arm comparison. The actual *runs* (which load the model)
//! live in the `bench` binary and are foreground/operator-only (16 GB rule).
//!
//! The compounding question — "does Day-N beat Day-0 with the model held constant?" — is decided by
//! [`paired_bootstrap_ci`]; the gate is [`Ci::improves_monotonically`] (CI excludes zero, positive).
//! There is nothing to compound until learning exists (M3), but the instrument is built now so the
//! signal is measurable the moment it could appear.

/// A frozen benchmark task: a prompt and an expected substring marking a correct answer.
#[derive(Clone, Debug)]
pub struct BenchTask {
    pub id: String,
    pub prompt: String,
    pub expected: String,
}

impl BenchTask {
    pub fn new(
        id: impl Into<String>,
        prompt: impl Into<String>,
        expected: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            prompt: prompt.into(),
            expected: expected.into(),
        }
    }
}

/// 1.0 if `response` contains `task.expected` (case-insensitive), else 0.0.
pub fn score_response(task: &BenchTask, response: &str) -> f64 {
    if response
        .to_lowercase()
        .contains(&task.expected.to_lowercase())
    {
        1.0
    } else {
        0.0
    }
}

/// Score a suite against per-task responses (same order/length).
pub fn score_suite(tasks: &[BenchTask], responses: &[String]) -> Vec<f64> {
    tasks
        .iter()
        .zip(responses)
        .map(|(t, r)| score_response(t, r))
        .collect()
}

pub fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        0.0
    } else {
        xs.iter().sum::<f64>() / xs.len() as f64
    }
}

/// Indices of tasks the bare model fails **cold** (no memory) — the only items where memory/skills can
/// show a meaningful lift (LongMemEval-V2's cold-answerability filter; D-M3-3). `cold` is per-task
/// pass/fail with no memory.
pub fn cold_failing_indices(cold: &[bool]) -> Vec<usize> {
    cold.iter()
        .enumerate()
        .filter(|(_, &passed)| !passed)
        .map(|(i, _)| i)
        .collect()
}

/// LiMem memorization score (CounterBench; D-M3-3): `Acc × (1 − consistency)`, where `consistency` is
/// the fraction of originally-correct items still correct after a structure-preserving perturbation
/// (same method, different answer). **High = solves originals but breaks on tiny edits = memorization,
/// not capability.** `original`/`perturbed` are per-task pass/fail over the same tasks.
pub fn limem(original: &[bool], perturbed: &[bool]) -> f64 {
    if original.is_empty() || original.len() != perturbed.len() {
        return 0.0;
    }
    let acc = original.iter().filter(|&&b| b).count() as f64 / original.len() as f64;
    let correct: Vec<usize> = original
        .iter()
        .enumerate()
        .filter(|(_, &b)| b)
        .map(|(i, _)| i)
        .collect();
    let consistency = if correct.is_empty() {
        1.0
    } else {
        correct.iter().filter(|&&i| perturbed[i]).count() as f64 / correct.len() as f64
    };
    acc * (1.0 - consistency)
}

/// Deterministic PRNG (xorshift64) so bootstrap resampling is reproducible from a seed.
struct Xorshift64(u64);

impl Xorshift64 {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// A confidence interval on the paired (Day-N − Day-0) mean delta.
#[derive(Clone, Debug)]
pub struct Ci {
    pub mean_delta: f64,
    pub lower: f64,
    pub upper: f64,
}

impl Ci {
    /// The compounding gate: the CI excludes zero on the positive side (Day-N reliably > Day-0).
    pub fn improves_monotonically(&self) -> bool {
        self.lower > 0.0
    }
}

/// Paired bootstrap CI on `day_n[i] - day0[i]`. Deterministic given `seed`. `alpha` e.g. 0.05.
pub fn paired_bootstrap_ci(
    day0: &[f64],
    day_n: &[f64],
    iterations: usize,
    seed: u64,
    alpha: f64,
) -> Ci {
    assert_eq!(
        day0.len(),
        day_n.len(),
        "paired CI needs equal-length samples"
    );
    let n = day0.len();
    let deltas: Vec<f64> = day0.iter().zip(day_n).map(|(a, b)| b - a).collect();
    let mean_delta = mean(&deltas);
    if n == 0 || iterations == 0 {
        return Ci {
            mean_delta,
            lower: mean_delta,
            upper: mean_delta,
        };
    }
    let mut rng = Xorshift64::new(seed);
    let mut means: Vec<f64> = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let mut acc = 0.0;
        for _ in 0..n {
            acc += deltas[rng.below(n)];
        }
        means.push(acc / n as f64);
    }
    means.sort_by(f64::total_cmp);
    Ci {
        mean_delta,
        lower: percentile(&means, alpha / 2.0),
        upper: percentile(&means, 1.0 - alpha / 2.0),
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = (p * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Anti-theater (build-spec §7): does the harness do causal work, or is it an accounting wrapper?
#[derive(Clone, Debug)]
pub struct AntiTheaterReport {
    /// Arm (a): harnessed machinery − plain-prompted agent, proposer held fixed.
    pub skeleton_delta: f64,
    /// Arm (b): full metabolic loop − stubbed (infinite budget), value-per-cost.
    pub metabolic_delta: f64,
    pub margin: f64,
    /// Fires if (a) OR (b) clears the margin. Arm (c) — finite-vs-infinite budget alone — is an
    /// accounting artifact and is deliberately NOT counted.
    pub fires: bool,
}

/// Build the anti-theater report from the four arm score sets.
pub fn anti_theater(
    harnessed: &[f64],
    plain: &[f64],
    metabolic: &[f64],
    stubbed: &[f64],
    margin: f64,
) -> AntiTheaterReport {
    let skeleton_delta = mean(harnessed) - mean(plain);
    let metabolic_delta = mean(metabolic) - mean(stubbed);
    AntiTheaterReport {
        skeleton_delta,
        metabolic_delta,
        margin,
        fires: skeleton_delta >= margin || metabolic_delta >= margin,
    }
}

// ---------------------------------------------------------------------------------------------
// Transfer corpus (D-M3-3): measures TRANSFER, not answer-lookup. A made-up operation the model
// cannot know cold; instances use fresh seeded operands, so passing requires APPLYING a learned
// rule to new inputs (the rule is the skill; the answer is never stored). Deterministic generator.
// ---------------------------------------------------------------------------------------------

/// The learnable rule (the skill). Self-contained + retrievable; the answer to any specific instance
/// is NOT here — only the method, so solving a new operand pair is transfer, not recall.
pub const TRANSFER_RULE: &str =
    "To compute a ⊕ b, calculate (a times b) plus a plus b. For example 2 ⊕ 3 = 2*3+2+3 = 11.";

/// The skill note in the format the research found small models actually apply (D-M3-3): definition +
/// worked examples showing the substitution + an explicit answer-line constraint.
pub const TRANSFER_SKILL_NOTE: &str = "Rule for the operation ⊕: a ⊕ b = (a × b) + a + b. \
Worked examples: 2 ⊕ 3 = 2*3 + 2 + 3 = 6 + 5 = 11.  4 ⊕ 5 = 4*5 + 4 + 5 = 20 + 9 = 29. \
Apply this rule to the new operands.";

/// One transfer task over `⊕`.
#[derive(Clone, Debug)]
pub struct TransferTask {
    pub prompt: String,
    pub expected: String,
}

/// `n` deterministic transfer tasks (seeded operands). Each answer = `a*b + a + b`; a cold guess of
/// `a+b` or `a*b` is always wrong (they differ by ≥4), so the bare model reliably fails without the rule.
pub fn transfer_corpus(n: usize, seed: u64) -> Vec<TransferTask> {
    let mut rng = Xorshift64::new(seed);
    (0..n)
        .map(|_| {
            let a = (rng.next() % 8 + 2) as i64; // 2..=9
            let b = (rng.next() % 8 + 2) as i64;
            TransferTask {
                prompt: format!("What is {a} ⊕ {b}?"),
                expected: (a * b + a + b).to_string(),
            }
        })
        .collect()
}

/// A small frozen suite for the foreground demo (provenance-isolated: fixed here, never from the
/// being's own failures).
pub fn default_frozen_suite() -> Vec<BenchTask> {
    vec![
        BenchTask::new("arith-1", "What is 2 + 2? Reply with just the number.", "4"),
        BenchTask::new(
            "cap-france",
            "What is the capital of France? One word.",
            "Paris",
        ),
        BenchTask::new(
            "sky-color",
            "What color is a clear daytime sky? One word.",
            "blue",
        ),
        BenchTask::new("arith-2", "What is 10 minus 7? Just the number.", "3"),
        BenchTask::new(
            "days-week",
            "How many days are in a week? Just the number.",
            "7",
        ),
        // Harder tier — calibrated to sit nearer the model's edge so compounding/self-mod have
        // headroom to move the score (the easy tier above saturates at 1.0 cold).
        BenchTask::new(
            "mult",
            "What is 17 multiplied by 23? Reply with just the number.",
            "391",
        ),
        BenchTask::new(
            "prime-7th",
            "What is the 7th prime number? Just the number.",
            "17",
        ),
        BenchTask::new(
            "cap-australia",
            "What is the capital of Australia? One word.",
            "Canberra",
        ),
        BenchTask::new(
            "leap-1900",
            "Is the year 1900 a leap year? Answer yes or no.",
            "no",
        ),
        BenchTask::new(
            "anagram",
            "Rearrange the letters of 'silent' into another common word. One word.",
            "listen",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_corpus_is_deterministic_and_well_formed() {
        let c1 = transfer_corpus(10, 7);
        let c2 = transfer_corpus(10, 7);
        assert_eq!(c1.len(), 10);
        let p1: Vec<&str> = c1.iter().map(|t| t.prompt.as_str()).collect();
        let p2: Vec<&str> = c2.iter().map(|t| t.prompt.as_str()).collect();
        assert_eq!(p1, p2); // deterministic for a seed
        assert!(c1
            .iter()
            .all(|t| t.expected.parse::<i64>().map(|v| v >= 8).unwrap_or(false)));
    }

    #[test]
    fn cold_failing_indices_selects_only_failures() {
        assert_eq!(
            cold_failing_indices(&[true, false, true, false]),
            vec![1, 3]
        );
        assert!(cold_failing_indices(&[true, true]).is_empty());
    }

    #[test]
    fn limem_flags_memorization_not_capability() {
        let originals = [true, true, true, true];
        // solves originals, breaks on every perturbation → maximal memorization
        assert!((limem(&originals, &[false, false, false, false]) - 1.0).abs() < 1e-9);
        // solves originals AND perturbations → robust, no memorization
        assert!(limem(&originals, &[true, true, true, true]).abs() < 1e-9);
    }

    #[test]
    fn scoring_is_case_insensitive_substring() {
        let t = BenchTask::new("t", "capital of France?", "Paris");
        assert_eq!(score_response(&t, "It is paris, I think."), 1.0);
        assert_eq!(score_response(&t, "London"), 0.0);
    }

    #[test]
    fn identical_runs_do_not_show_improvement() {
        let day0 = vec![1.0, 0.0, 1.0, 1.0];
        let ci = paired_bootstrap_ci(&day0, &day0, 1000, 42, 0.05);
        assert_eq!(ci.mean_delta, 0.0);
        assert!(!ci.improves_monotonically());
        assert!(ci.lower <= 0.0 && ci.upper >= 0.0);
    }

    #[test]
    fn uniform_gain_shows_monotonic_improvement() {
        let day0 = vec![0.0, 0.0, 0.0, 0.0];
        let day_n = vec![1.0, 1.0, 1.0, 1.0];
        let ci = paired_bootstrap_ci(&day0, &day_n, 1000, 7, 0.05);
        assert_eq!(ci.mean_delta, 1.0);
        assert!(ci.improves_monotonically()); // every resample mean is +1 → lower bound > 0
    }

    #[test]
    fn bootstrap_is_deterministic_for_a_seed() {
        let a = vec![0.0, 1.0, 0.0, 1.0, 0.0];
        let b = vec![1.0, 1.0, 1.0, 0.0, 1.0];
        let c1 = paired_bootstrap_ci(&a, &b, 500, 123, 0.05);
        let c2 = paired_bootstrap_ci(&a, &b, 500, 123, 0.05);
        assert_eq!((c1.lower, c1.upper), (c2.lower, c2.upper));
    }

    #[test]
    fn anti_theater_fires_only_on_a_real_margin() {
        // skeleton beats plain by 0.3, metabolic ties stubbed → fires on (a)
        let r = anti_theater(&[1.0, 1.0, 0.0], &[0.0, 1.0, 0.0], &[1.0], &[1.0], 0.2);
        assert!(r.skeleton_delta > 0.0);
        assert!(r.fires);
        // no arm clears the margin → does not fire (the accounting-wrapper outcome)
        let r2 = anti_theater(&[1.0], &[1.0], &[1.0], &[1.0], 0.2);
        assert!(!r2.fires);
    }

    #[test]
    fn frozen_suite_is_nonempty_and_scored() {
        let suite = default_frozen_suite();
        let responses: Vec<String> = suite.iter().map(|t| t.expected.clone()).collect();
        let scores = score_suite(&suite, &responses);
        assert_eq!(mean(&scores), 1.0); // perfect responses score 1.0
    }
}
