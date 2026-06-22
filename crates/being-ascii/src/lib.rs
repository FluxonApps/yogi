//! `being-ascii` — the **NLP→ASCII** domain Yogi evolves toward (the survival-wired goal).
//!
//! Reality-check finding (qwen3:8b, 2026-06-22): the base model is *bad but learnable* at ASCII — it
//! produces spatially-structured attempts, few-shot exemplars steer style, and it takes memorized
//! shortcuts (a "smiley" came back as a one-line kaomoji). So this module supplies:
//! - [`StructuralGate`] — the free, ungameable **L1 grader**, including the empirically-motivated
//!   **anti-kaomoji rule** (a real drawing must be *composed* over multiple lines, not a memorized
//!   one-liner) — degenerate Goodhart hacks are rejected for $0 before any judge call.
//! - a **subject × style niche** ([`AsciiArt::style_axis`]/[`size_axis`]) that is *structural* and
//!   therefore decorrelated from quality — the within-niche quality variance the drift gate needs.
//! - pluggable [`Generator`] (the model) and [`QualityJudge`] (the LLM judge); the real cloud versions
//!   plug in foreground, while structural stand-ins keep this crate **pure, loop-safe, model-free**.
//! - [`AsciiEvaluator`] — wires the above into [`being_lineage::Evaluator`] for `illuminate`.

use being_core_mutation::{Genome, MutationKind};
use being_lineage::{Evaluation, Evaluator, Rng, Variator};
use being_proposer_openai::{OpenAiChatConfig, OpenAiChatProposer};
use being_runtime::ContextPack;
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------------------------
// The artifact
// ---------------------------------------------------------------------------------------------

/// A parsed ASCII drawing: rows of text with trailing blank lines trimmed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsciiArt {
    lines: Vec<String>,
}

impl AsciiArt {
    pub fn parse(raw: &str) -> Self {
        let mut lines: Vec<String> = raw
            .replace('\r', "")
            .split('\n')
            .map(|l| l.trim_end().to_string())
            .collect();
        while lines.first().is_some_and(|l| l.is_empty()) {
            lines.remove(0);
        }
        while lines.last().is_some_and(|l| l.is_empty()) {
            lines.pop();
        }
        Self { lines }
    }

    pub fn height(&self) -> usize {
        self.lines.len()
    }

    pub fn width(&self) -> usize {
        self.lines
            .iter()
            .map(|l| l.chars().count())
            .max()
            .unwrap_or(0)
    }

    /// Non-whitespace character count ("ink").
    pub fn ink(&self) -> usize {
        self.lines
            .iter()
            .flat_map(|l| l.chars())
            .filter(|c| !c.is_whitespace())
            .count()
    }

    /// Distinct non-whitespace characters used.
    pub fn distinct_ink(&self) -> usize {
        let mut set = std::collections::BTreeSet::new();
        for c in self.lines.iter().flat_map(|l| l.chars()) {
            if !c.is_whitespace() {
                set.insert(c);
            }
        }
        set.len()
    }

    /// Ink density over the bounding box, in `[0, 1]`.
    pub fn density(&self) -> f64 {
        let area = (self.width() * self.height()) as f64;
        if area == 0.0 {
            return 0.0;
        }
        self.ink() as f64 / area
    }

    /// Niche axis 1 — **style by density** in `[0, 1]` (sparse line-art → dense fill). Structural, so
    /// decorrelated from quality: *what kind* of drawing, not *how good*.
    pub fn style_axis(&self) -> f64 {
        self.density().clamp(0.0, 1.0)
    }

    /// Niche axis 2 — **size** in `[0, 1]`, normalized by a generous canvas (40×20).
    pub fn size_axis(&self) -> f64 {
        ((self.width() * self.height()) as f64 / 800.0).clamp(0.0, 1.0)
    }

    /// The drawing as text (rows joined by newlines).
    pub fn render(&self) -> String {
        self.lines.join("\n")
    }
}

// ---------------------------------------------------------------------------------------------
// L1 structural grader — free, ungameable, anti-Goodhart
// ---------------------------------------------------------------------------------------------

/// Verdict of the structural gate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Validity {
    Ok,
    Rejected(&'static str),
}

impl Validity {
    pub fn is_ok(&self) -> bool {
        matches!(self, Validity::Ok)
    }
}

/// The cheap, ungameable first grading layer. It rejects the degenerate Goodhart hacks the reality
/// check surfaced — *before* any (paid, gameable) judge call.
#[derive(Clone, Copy, Debug)]
pub struct StructuralGate {
    pub min_lines: usize,
    pub max_width: usize,
    pub max_height: usize,
    pub min_density: f64,
    pub max_density: f64,
    pub min_distinct_ink: usize,
}

impl Default for StructuralGate {
    fn default() -> Self {
        Self {
            min_lines: 3, // ANTI-KAOMOJI: a drawing must be composed, not a memorized one-liner
            max_width: 40,
            max_height: 20,
            min_density: 0.03,   // not near-blank
            max_density: 0.90,   // not a solid block (must have negative space)
            min_distinct_ink: 2, // not a single repeated character
        }
    }
}

impl StructuralGate {
    pub fn check(&self, art: &AsciiArt) -> Validity {
        if art.height() < self.min_lines {
            // The empirical failure mode: "smiley" → "( ͡° ͜°)". A one-liner is not a composed drawing.
            return Validity::Rejected("not composed: fewer than min_lines (anti-kaomoji)");
        }
        if art.ink() == 0 {
            return Validity::Rejected("blank");
        }
        if art.distinct_ink() < self.min_distinct_ink {
            return Validity::Rejected("degenerate: a single repeated character");
        }
        if art.width() > self.max_width || art.height() > self.max_height {
            return Validity::Rejected("exceeds canvas bounds");
        }
        let d = art.density();
        if d < self.min_density {
            return Validity::Rejected("near-blank");
        }
        if d > self.max_density {
            return Validity::Rejected("solid fill: no negative space");
        }
        Validity::Ok
    }
}

// ---------------------------------------------------------------------------------------------
// Pluggable model-facing pieces (cloud plugs in here; structural stand-ins keep the crate pure)
// ---------------------------------------------------------------------------------------------

/// Turns a genome (drawing policy: prompt + exemplar skills) + a subject into raw text. The real
/// implementation calls the local/frontier model (foreground); a stand-in keeps tests model-free.
pub trait Generator {
    fn generate(&mut self, genome: &Genome, subject: &str) -> String;
}

/// Scores how well an `AsciiArt` realizes `subject`, in `[0, 1]`. The real implementation is an LLM
/// judge (QDAIF-style, frontier, operator-side so the being can't see/game it). [`StructuralJudge`] is
/// an explicit **placeholder** that lets the loop run without a model — it is NOT a quality measure.
pub trait QualityJudge {
    fn score(&mut self, subject: &str, art: &AsciiArt) -> f64;
}

/// Placeholder judge: a weak structural proxy (rewards character variety + mid-range density) so the QD
/// loop has a gradient in tests. **Do not mistake this for quality** — real quality needs the LLM judge.
pub struct StructuralJudge;

impl QualityJudge for StructuralJudge {
    fn score(&mut self, _subject: &str, art: &AsciiArt) -> f64 {
        let variety = (art.distinct_ink() as f64 / 8.0).min(1.0);
        let d = art.density();
        let mid = 1.0 - (d - 0.3).abs() / 0.3; // peaks near a "drawn" density, not blank/solid
        (0.5 * variety + 0.5 * mid.clamp(0.0, 1.0)).clamp(0.0, 1.0)
    }
}

// ---------------------------------------------------------------------------------------------
// Frontier judge: Claude (via `claude -p`) scores quality; calls are the being's metered "salary"
// ---------------------------------------------------------------------------------------------

/// Runs a frontier prompt and returns its text (or `None` on failure). The real impl shells out to
/// `claude -p` (FOREGROUND ONLY — never in the green-gate); a stub keeps tests model-free.
pub trait FrontierRunner {
    fn run(&mut self, prompt: &str) -> Option<String>;
}

/// Real runner: invokes the local `claude` CLI in headless print mode. Each call spends real frontier
/// budget (your Claude subscription) — that scarcity *is* the being's metabolic pressure.
pub struct ClaudeCliRunner;

impl FrontierRunner for ClaudeCliRunner {
    fn run(&mut self, prompt: &str) -> Option<String> {
        let out = std::process::Command::new("claude")
            .arg("-p")
            .arg(prompt)
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

/// Build the judging rubric. Live validation (2026-06-22) showed a bare "reply with an integer" rubric
/// is **flat** (good-cat, crude-cat, and even a kaomoji all scored ~6-7). A **chain-of-thought +
/// explicit criteria + structured score line** rubric discriminated cleanly (0.8 / 0.4 / 0.1) — matching
/// the LLM-judge literature (reasoning-then-score beats a bare absolute number). The being never sees
/// this prompt or the judge's reasoning (anti-Goodhart: it can't directly optimize the judge's words).
pub fn rubric_prompt(subject: &str, art: &AsciiArt) -> String {
    format!(
        "You are a STRICT judge of ASCII art. Intended subject: \"{subject}\".\n\n{}\n\n\
         Step 1: in one sentence, describe what this actually looks like and whether it reads as a {subject}.\n\
         Step 2: score it, being discriminating — reserve 8-10 for genuinely recognizable AND \
         well-composed; 4-6 for crude-but-suggestive; 1-3 for a vague blob; 0 if it is not a composed \
         drawing.\nEnd with a final line EXACTLY in this form: SCORE: <n>/10",
        art.render()
    )
}

/// Parse a score from judge output → `[0,1]`. Prefers the structured `SCORE: <n>/10` line; falls back
/// to the last integer in `0..=10`. No parseable score → 0.
pub fn parse_score(out: &str) -> f64 {
    fn first_int_le10(s: &str) -> Option<u32> {
        let mut digits = String::new();
        for c in s.chars() {
            if c.is_ascii_digit() {
                digits.push(c);
            } else if !digits.is_empty() {
                break;
            }
        }
        digits.parse::<u32>().ok().filter(|n| *n <= 10)
    }
    if let Some(idx) = out.find("SCORE:") {
        if let Some(n) = first_int_le10(&out[idx + "SCORE:".len()..]) {
            return n as f64 / 10.0;
        }
    }
    // Fallback: the LAST integer in 0..=10 (models tend to end with the score).
    let mut last: Option<u32> = None;
    let mut digits = String::new();
    let flush = |digits: &mut String, last: &mut Option<u32>| {
        if let Ok(n) = digits.parse::<u32>() {
            if n <= 10 {
                *last = Some(n);
            }
        }
        digits.clear();
    };
    for c in out.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else {
            flush(&mut digits, &mut last);
        }
    }
    flush(&mut digits, &mut last);
    last.map(|n| n as f64 / 10.0).unwrap_or(0.0)
}

/// The frontier quality judge. Each `score` call spends one unit of the being's frontier-call budget
/// (its "salary"); when the budget is exhausted it can no longer afford to be judged (returns 0).
pub struct ClaudeJudge<R: FrontierRunner> {
    pub runner: R,
    pub calls_made: u64,
    pub max_calls: u64,
    pub microdollars_per_call: i64,
    pub spent: Cost,
}

impl<R: FrontierRunner> ClaudeJudge<R> {
    /// `max_calls` is the hard salary cap — a safety bound so a runaway loop can't drain the account.
    pub fn new(runner: R, max_calls: u64) -> Self {
        Self {
            runner,
            calls_made: 0,
            max_calls,
            microdollars_per_call: 20_000, // ~2¢/call notional; the metered scarcity is what matters
            spent: Cost::default(),
        }
    }

    pub fn budget_exhausted(&self) -> bool {
        self.calls_made >= self.max_calls
    }
}

impl<R: FrontierRunner> QualityJudge for ClaudeJudge<R> {
    fn score(&mut self, subject: &str, art: &AsciiArt) -> f64 {
        if self.budget_exhausted() {
            return 0.0; // out of salary — cannot afford a frontier judgment
        }
        self.calls_made += 1;
        self.spent.frontier_microdollars += self.microdollars_per_call;
        match self.runner.run(&rubric_prompt(subject, art)) {
            Some(out) => parse_score(&out),
            None => 0.0,
        }
    }
}

/// Microdollar cost of producing/grading a drawing. The economic layer (a being earns its keep) reads
/// this; `frontier` is real money (teacher/judge calls), `local` is a notional cost so arbitrage isn't
/// trivially "everything local is free profit".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cost {
    pub frontier_microdollars: i64,
    pub local_microdollars: i64,
}

impl Cost {
    pub fn total(&self) -> i64 {
        self.frontier_microdollars + self.local_microdollars
    }
}

// ---------------------------------------------------------------------------------------------
// Evaluator: wires the domain into being-lineage::illuminate (QDAIF-style)
// ---------------------------------------------------------------------------------------------

/// Evaluates an ASCII drawing **genome** over a set of probe subjects: generate → structural-gate →
/// judge, with `fitness` = mean quality of the *valid* drawings and `behavior` = `[mean style, mean
/// size]` (the structural niche, decorrelated from quality). Plugs straight into `illuminate`.
pub struct AsciiEvaluator<G: Generator, J: QualityJudge> {
    pub generator: G,
    pub judge: J,
    pub gate: StructuralGate,
    pub subjects: Vec<String>,
    /// Notional local cost charged per generation (the model run); accumulates across evaluations.
    pub local_cost_per_gen: i64,
    pub spent: Cost,
    /// The highest-scored *valid* drawing seen so far (for the live dashboard) — retained as it's judged,
    /// so showing the being's best work costs no extra model calls.
    pub best_sample: Option<BestSample>,
}

/// The being's best judged drawing so far — what the live status card renders.
#[derive(Clone, Debug, PartialEq)]
pub struct BestSample {
    pub score: f64,
    pub subject: String,
    pub art: String,
}

impl<G: Generator, J: QualityJudge> AsciiEvaluator<G, J> {
    pub fn new(generator: G, judge: J, subjects: Vec<String>) -> Self {
        Self {
            generator,
            judge,
            gate: StructuralGate::default(),
            subjects,
            local_cost_per_gen: 1,
            spent: Cost::default(),
            best_sample: None,
        }
    }
}

impl<G: Generator, J: QualityJudge> Evaluator for AsciiEvaluator<G, J> {
    fn evaluate(&mut self, genome: &Genome) -> Evaluation {
        let mut qsum = 0.0;
        let mut style = 0.0;
        let mut size = 0.0;
        let n = self.subjects.len().max(1) as f64;
        for subject in self.subjects.clone() {
            let art = AsciiArt::parse(&self.generator.generate(genome, &subject));
            self.spent.local_microdollars += self.local_cost_per_gen;
            style += art.style_axis();
            size += art.size_axis();
            // A drawing that fails the structural gate scores 0 — degenerate hacks earn nothing.
            let valid = self.gate.check(&art).is_ok();
            let q = if valid {
                self.judge.score(&subject, &art).clamp(0.0, 1.0)
            } else {
                0.0
            };
            // Retain the best valid drawing for the live dashboard (free — already drawn + judged).
            if valid && self.best_sample.as_ref().is_none_or(|b| q > b.score) {
                self.best_sample = Some(BestSample {
                    score: q,
                    subject: subject.clone(),
                    art: art.render(),
                });
            }
            qsum += q;
        }
        Evaluation {
            fitness: qsum / n,
            behavior: vec![style / n, size / n],
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Closed-surface variation for ASCII genomes
// ---------------------------------------------------------------------------------------------

/// Varies an ASCII genome along the **closed mutation surface**: swap the drawing-style directive
/// (a `Prompt` edit) or install/revoke a technique exemplar (a `SkillInstall`/`SkillRevoke`). The
/// being can only ever differ along these sanctioned axes — no forbidden power is expressible.
pub struct AsciiVariator {
    pub style_directives: Vec<String>,
    pub exemplars: Vec<String>,
}

impl Default for AsciiVariator {
    fn default() -> Self {
        Self {
            style_directives: vec![
                "Draw using clean line-art (/ \\ | _ - ( ) characters).".into(),
                "Draw using blocky fills (# @ * characters) with negative space.".into(),
                "Draw minimally: a few strokes that suggest the subject.".into(),
            ],
            exemplars: vec![
                "exemplar:compact-animal".into(),
                "exemplar:boxy-object".into(),
                "exemplar:face-3line".into(),
            ],
        }
    }
}

impl Variator for AsciiVariator {
    fn vary(&mut self, rng: &mut Rng, parent: &Genome) -> Vec<MutationKind> {
        let roll = rng.next_u64() % 3;
        match roll {
            0 if !self.style_directives.is_empty() => {
                let i = (rng.next_u64() as usize) % self.style_directives.len();
                vec![MutationKind::Prompt(self.style_directives[i].clone())]
            }
            1 if !self.exemplars.is_empty() => {
                let i = (rng.next_u64() as usize) % self.exemplars.len();
                vec![MutationKind::SkillInstall(self.exemplars[i].clone())]
            }
            _ => {
                // Revoke an installed exemplar if any, else a no-op style nudge.
                if let Some(s) = parent.installed_skills.iter().next() {
                    vec![MutationKind::SkillRevoke(s.clone())]
                } else if let Some(d) = self.style_directives.first() {
                    vec![MutationKind::Prompt(d.clone())]
                } else {
                    vec![]
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Local ASCII drawer (qwen3:8b via Ollama) — the being's generator. Generation is FOREGROUND-only.
// ---------------------------------------------------------------------------------------------

/// Default exemplar library: maps an exemplar id (installed via the closed-surface `SkillInstall`) to a
/// few-shot example. The reality check showed exemplars steer the model's style — this is that lever.
pub fn default_exemplar_library() -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "exemplar:compact-animal".to_string(),
            " /\\_/\\\n( o.o )\n > ^ <".to_string(),
        ),
        (
            "exemplar:boxy-object".to_string(),
            " ___\n|   |\n|___|".to_string(),
        ),
        (
            "exemplar:face-3line".to_string(),
            " ___\n(o o)\n \\_/".to_string(),
        ),
    ])
}

/// Build the drawing prompt for `genome` + `subject` — **pure**, no model, so it's unit-testable. The
/// genome's prompt is the style directive; its installed exemplar-skills become few-shot context.
pub fn draw_prompt(
    genome: &Genome,
    subject: &str,
    library: &BTreeMap<String, String>,
) -> (String, Vec<String>) {
    let retrieved: Vec<String> = genome
        .installed_skills
        .iter()
        .filter_map(|s| library.get(s))
        .map(|a| format!("Example ASCII art:\n{a}"))
        .collect();
    let base = if genome.prompt.trim().is_empty() {
        "Draw clean, recognizable ASCII art."
    } else {
        genome.prompt.as_str()
    };
    let input = format!(
        "{base}\nDraw a {subject} as ASCII art (at most 12 lines). \
         Output ONLY the art — no commentary, no code fences."
    );
    (input, retrieved)
}

/// Extract the ASCII art from a raw model response: drop a `<think>…</think>` block (qwen3 thinking
/// mode — keep only what follows the last `</think>`; an unclosed/truncated think yields no art) and
/// unwrap a fenced code block if present. Pure + testable. *Found empirically: `/no_think` returns an
/// empty string for drawing prompts, so the generator runs in thinking mode and relies on this.*
pub fn extract_art(raw: &str) -> String {
    let body = if let Some(pos) = raw.rfind("</think>") {
        &raw[pos + "</think>".len()..]
    } else if raw.contains("<think>") {
        "" // think opened but never closed (truncated) → no usable art
    } else {
        raw
    };
    // Unwrap a fenced block (the art is often inside ``` … ```).
    let body = if let Some(start) = body.find("```") {
        let after = &body[start + 3..];
        let after = after.split_once('\n').map(|(_, r)| r).unwrap_or(after); // drop the ```lang line
        match after.find("```") {
            Some(end) => &after[..end],
            None => after,
        }
    } else {
        body
    };
    body.trim_matches('\n').to_string()
}

/// The local ASCII drawer: qwen3:8b via the Ollama client, in **thinking mode** (no_think returns empty
/// for drawing prompts). `generate` performs a model call and is therefore **foreground-only** (never
/// exercised by the green-gate — tests use a stub generator).
pub struct OllamaGenerator {
    proposer: OpenAiChatProposer,
    pub library: BTreeMap<String, String>,
}

impl OllamaGenerator {
    pub fn new() -> Self {
        Self {
            proposer: OpenAiChatProposer::new(OpenAiChatConfig::ollama_qwen3_thinking()),
            library: default_exemplar_library(),
        }
    }
}

impl Default for OllamaGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl Generator for OllamaGenerator {
    fn generate(&mut self, genome: &Genome, subject: &str) -> String {
        let (input, retrieved) = draw_prompt(genome, subject, &self.library);
        let raw = self
            .proposer
            .try_propose(&ContextPack { input, retrieved })
            .unwrap_or_default();
        extract_art(&raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_rejects_the_kaomoji_shortcut() {
        // The exact reality-check failure: "smiley" → a one-line kaomoji. Not a composed drawing.
        let art = AsciiArt::parse("( ͡° ͜°)");
        assert_eq!(
            StructuralGate::default().check(&art),
            Validity::Rejected("not composed: fewer than min_lines (anti-kaomoji)")
        );
    }

    #[test]
    fn gate_rejects_blank_degenerate_and_solid() {
        let g = StructuralGate::default();
        assert!(!g.check(&AsciiArt::parse("\n\n\n")).is_ok()); // blank
        assert!(!g.check(&AsciiArt::parse("|||\n|||\n|||")).is_ok()); // single repeated char
        let solid = "########\n########\n########\n########";
        assert!(!g.check(&AsciiArt::parse(solid)).is_ok()); // solid fill, no negative space
    }

    #[test]
    fn gate_accepts_a_real_composed_drawing() {
        let cat = " /\\_/\\\n( o.o )\n > ^ <";
        assert_eq!(
            StructuralGate::default().check(&AsciiArt::parse(cat)),
            Validity::Ok
        );
    }

    #[test]
    fn style_and_size_axes_are_in_range_and_track_density() {
        let sparse = AsciiArt::parse(" .\n. .\n .");
        let dense = AsciiArt::parse("####\n#..#\n####");
        assert!(sparse.style_axis() < dense.style_axis());
        for a in [&sparse, &dense] {
            assert!((0.0..=1.0).contains(&a.style_axis()) && (0.0..=1.0).contains(&a.size_axis()));
        }
    }

    // A deterministic stand-in generator: emits one of three template arts keyed by the genome's prompt
    // length parity + subject — enough to exercise the evaluator/illuminate pipeline with no model.
    struct CannedGenerator;
    impl Generator for CannedGenerator {
        fn generate(&mut self, genome: &Genome, subject: &str) -> String {
            let pick = (genome.prompt.len() + subject.len()) % 3;
            match pick {
                0 => " /\\_/\\\n( o.o )\n > ^ <".into(),
                1 => " __\n|  |\n|__|".into(),
                _ => "( ͡° ͜°)".into(), // the kaomoji shortcut — should score 0 (gate rejects)
            }
        }
    }

    // A stub frontier runner — returns canned judge output, so the ClaudeJudge logic is tested without
    // ever calling `claude -p` (the green-gate stays model-free).
    struct StubRunner(&'static str);
    impl FrontierRunner for StubRunner {
        fn run(&mut self, _prompt: &str) -> Option<String> {
            Some(self.0.to_string())
        }
    }

    #[test]
    fn parse_score_prefers_structured_line_else_last_integer() {
        // structured SCORE: line (the validated rubric's output) wins
        assert_eq!(parse_score("Step 2: ...\nSCORE: 8/10"), 0.8);
        assert_eq!(parse_score("SCORE: 10/10"), 1.0);
        assert_eq!(parse_score("SCORE: 0/10"), 0.0);
        // fallback: last integer 0..=10, dodging the "0-10" echoed from the prompt
        assert_eq!(parse_score("On the 0-10 scale I rate this an 8"), 0.8);
        assert_eq!(parse_score("7"), 0.7);
        assert_eq!(parse_score("no number here"), 0.0);
    }

    #[test]
    fn extract_art_strips_think_and_fences() {
        // thinking mode: keep only what follows </think>
        assert_eq!(
            extract_art("<think>plan the cat</think>\n /\\_/\\\n(o.o)\n >^<"),
            " /\\_/\\\n(o.o)\n >^<"
        );
        // fenced block is unwrapped
        assert_eq!(extract_art("```\n /\\_/\\\n(o.o)\n```"), " /\\_/\\\n(o.o)");
        // truncated think (no close) → no art
        assert_eq!(extract_art("<think>still reasoning and cut off"), "");
        // plain art passes through
        assert_eq!(extract_art(" /\\\n(o.o)"), " /\\\n(o.o)");
    }

    #[test]
    fn draw_prompt_includes_subject_and_resolves_exemplars() {
        let lib = default_exemplar_library();
        let mut g = Genome::default();
        g.installed_skills.insert("exemplar:compact-animal".into());
        let (input, retrieved) = draw_prompt(&g, "cat", &lib);
        assert!(input.contains("cat"));
        assert_eq!(retrieved.len(), 1);
        assert!(retrieved[0].contains("o.o")); // the exemplar's art is in the few-shot context
    }

    #[test]
    fn rubric_includes_subject_and_art() {
        let art = AsciiArt::parse(" /\\_/\\\n( o.o )\n > ^ <");
        let p = rubric_prompt("cat", &art);
        assert!(p.contains("cat") && p.contains("o.o"));
    }

    #[test]
    fn claude_judge_spends_salary_and_reaps_at_cap() {
        let art = AsciiArt::parse(" /\\_/\\\n( o.o )\n > ^ <");
        let mut judge = ClaudeJudge::new(StubRunner("SCORE: 9/10"), 2); // salary = 2 calls
        assert_eq!(judge.score("cat", &art), 0.9);
        assert_eq!(judge.score("cat", &art), 0.9);
        assert!(judge.budget_exhausted());
        assert_eq!(judge.score("cat", &art), 0.0); // out of salary → can't afford a judgment
        assert_eq!(judge.calls_made, 2); // exhausted call does not spend
        assert_eq!(judge.spent.frontier_microdollars, 40_000);
    }

    #[test]
    fn evaluator_scores_and_describes_a_genome() {
        let mut e = AsciiEvaluator::new(
            CannedGenerator,
            StructuralJudge,
            vec!["cat".into(), "house".into(), "tree".into()],
        );
        let ev = e.evaluate(&Genome::default());
        assert!((0.0..=1.0).contains(&ev.fitness));
        assert_eq!(ev.behavior.len(), 2); // [style, size] niche
        assert!(e.spent.local_microdollars >= 3); // charged per generation
    }

    #[test]
    fn evaluator_retains_best_judged_drawing_for_the_dashboard() {
        let mut e = AsciiEvaluator::new(
            CannedGenerator,
            StructuralJudge,
            vec!["cat".into(), "house".into(), "tree".into()],
        );
        e.evaluate(&Genome::default());
        let b = e
            .best_sample
            .as_ref()
            .expect("a valid drawing should be retained");
        assert!(b.score > 0.0 && !b.art.is_empty());
    }

    #[test]
    fn evaluator_wires_into_illuminate_and_fills_cells() {
        use being_lineage::{illuminate, Archive, BehaviorDescriptor, IlluminationConfig};
        let mut e = AsciiEvaluator::new(
            CannedGenerator,
            StructuralJudge,
            vec!["cat".into(), "house".into()],
        );
        let mut v = AsciiVariator::default();
        // 2D niche: style(density) × size, each binned over [0,1].
        let descriptor = BehaviorDescriptor::bounded([(0.0, 1.0, 4), (0.0, 1.0, 4)]).unwrap();
        let mut archive = Archive::new();
        let cfg = IlluminationConfig::new(30, 42);
        illuminate(
            &mut archive,
            &descriptor,
            Genome::default(),
            1,
            &mut e,
            &mut v,
            &cfg,
            None,
        );
        assert!(
            !archive.is_empty(),
            "illumination filled at least one ASCII niche"
        );
    }
}
