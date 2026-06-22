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
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

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

    /// Niche axis 2′ — **aspect** (width/height) in `[0, 1]`, more ASCII-sensitive than raw size: a tall
    /// tower (≈0) vs a square (≈0.25) vs a wide banner (→1) land in different niches, so QD coverage
    /// spreads across *shapes* (the first run collapsed to one niche on the size axis).
    pub fn aspect_axis(&self) -> f64 {
        let h = self.height().max(1) as f64;
        (self.width() as f64 / h / 4.0).clamp(0.0, 1.0) // w/h in [0,4] → [0,1]
    }

    /// The drawing as text (rows joined by newlines).
    pub fn render(&self) -> String {
        self.lines.join("\n")
    }
}

// ---------------------------------------------------------------------------------------------
// BREAKTHROUGH: a drawing DSL + deterministic canvas — change the ACTION SPACE.
// Research (Symbolic Graphics Programming 2509.05208; Visual Sketchpad 2406.09403): LLMs fail at
// emitting spatial characters directly but compose geometric PRIMITIVES well, and "encoding graphics
// as programs with precise parameters outperforms direct generation". So the being emits a PROGRAM of
// primitive ops; this renderer guarantees spatial correctness — letting the evolved *system* draw what
// the 8B base cannot one-shot. (The point of evolution: transcend base capability, not inherit it.)
// ---------------------------------------------------------------------------------------------

/// A fixed-size character grid with deterministic drawing primitives.
pub struct Canvas {
    w: usize,
    h: usize,
    grid: Vec<Vec<char>>,
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            grid: vec![vec![' '; w]; h],
        }
    }

    pub fn put(&mut self, x: i64, y: i64, ch: char) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.grid[y as usize][x as usize] = ch;
        }
    }

    pub fn hline(&mut self, x: i64, y: i64, len: i64, ch: char) {
        for i in 0..len.max(0) {
            self.put(x + i, y, ch);
        }
    }

    pub fn vline(&mut self, x: i64, y: i64, len: i64, ch: char) {
        for i in 0..len.max(0) {
            self.put(x, y + i, ch);
        }
    }

    /// Rectangle outline.
    pub fn rect(&mut self, x: i64, y: i64, w: i64, h: i64, ch: char) {
        if w <= 0 || h <= 0 {
            return;
        }
        self.hline(x, y, w, ch);
        self.hline(x, y + h - 1, w, ch);
        self.vline(x, y, h, ch);
        self.vline(x + w - 1, y, h, ch);
    }

    pub fn render(&self) -> String {
        let mut rows: Vec<String> = self
            .grid
            .iter()
            .map(|r| r.iter().collect::<String>().trim_end().to_string())
            .collect();
        while rows.last().is_some_and(|r| r.is_empty()) {
            rows.pop();
        }
        rows.join("\n")
    }
}

/// Execute a drawing PROGRAM (one primitive op per line) on a fresh canvas and return the rendered art.
/// Ops (1-indexed coords from top-left): `put X Y CH` · `hline X Y LEN CH` · `vline X Y LEN CH` ·
/// `rect X Y W H CH`. Unknown/malformed lines are ignored (robust to model noise). This is the being's
/// new action space — compose shapes, not characters.
pub fn run_program(prog: &str, w: usize, h: usize) -> AsciiArt {
    let mut c = Canvas::new(w, h);
    let n = |s: &str| s.parse::<i64>().unwrap_or(0);
    let ch = |s: &str| s.chars().next().unwrap_or('#');
    for line in prog.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        match t.as_slice() {
            ["put", x, y, c2] => c.put(n(x), n(y), ch(c2)),
            ["hline", x, y, l, c2] => c.hline(n(x), n(y), n(l), ch(c2)),
            ["vline", x, y, l, c2] => c.vline(n(x), n(y), n(l), ch(c2)),
            ["rect", x, y, ww, hh, c2] => c.rect(n(x), n(y), n(ww), n(hh), ch(c2)),
            _ => {} // ignore commentary / malformed lines
        }
    }
    AsciiArt::parse(&c.render())
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
    /// The flywheel store: every Claude-validated drawing is offered to it via `learn`. Shared with the
    /// generator (which few-shots from the learned set), so good drawings feed the next generation.
    pub store: Rc<RefCell<ExemplarStore>>,
}

/// The being's best judged drawing so far — what the live status card renders.
#[derive(Clone, Debug, PartialEq)]
pub struct BestSample {
    pub score: f64,
    pub subject: String,
    pub art: String,
}

impl<G: Generator, J: QualityJudge> AsciiEvaluator<G, J> {
    /// Evaluator with a private store (no generator sharing — for stub-generator tests).
    pub fn new(generator: G, judge: J, subjects: Vec<String>) -> Self {
        Self::with_store(generator, judge, subjects, ExemplarStore::shared(0.5, 3))
    }

    /// Evaluator sharing a flywheel store with the generator (the live wiring).
    pub fn with_store(
        generator: G,
        judge: J,
        subjects: Vec<String>,
        store: Rc<RefCell<ExemplarStore>>,
    ) -> Self {
        Self {
            generator,
            judge,
            gate: StructuralGate::default(),
            subjects,
            local_cost_per_gen: 1,
            spent: Cost::default(),
            best_sample: None,
            store,
        }
    }
}

impl<G: Generator, J: QualityJudge> Evaluator for AsciiEvaluator<G, J> {
    fn evaluate(&mut self, genome: &Genome) -> Evaluation {
        let mut qsum = 0.0;
        let mut style = 0.0;
        let mut aspect = 0.0;
        let n = self.subjects.len().max(1) as f64;
        for subject in self.subjects.clone() {
            let art = AsciiArt::parse(&self.generator.generate(genome, &subject));
            self.spent.local_microdollars += self.local_cost_per_gen;
            style += art.style_axis();
            aspect += art.aspect_axis();
            // A drawing that fails the structural gate scores 0 — degenerate hacks earn nothing.
            let valid = self.gate.check(&art).is_ok();
            let q = if valid {
                self.judge.score(&subject, &art).clamp(0.0, 1.0)
            } else {
                0.0
            };
            if valid {
                // Flywheel: offer the validated drawing to the store (kept iff ≥ threshold and novel).
                self.store.borrow_mut().learn(&art.render(), q);
                // Retain the best valid drawing for the live dashboard (free — already drawn + judged).
                if self.best_sample.as_ref().is_none_or(|b| q > b.score) {
                    self.best_sample = Some(BestSample {
                        score: q,
                        subject: subject.clone(),
                        art: art.render(),
                    });
                }
            }
            qsum += q;
        }
        Evaluation {
            fitness: qsum / n,
            behavior: vec![style / n, aspect / n], // niche: style-density × aspect (shape)
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

/// Build the prompt-mutation instruction (pure, testable). EvoPrompt/OPRO-style: ask the model to
/// rewrite the current drawing instruction into a clearly *different* one, to push output diversity
/// (the lever for the stuck `niches=1` coverage).
pub fn mutation_prompt(parent_prompt: &str) -> String {
    let cur = if parent_prompt.trim().is_empty() {
        "Draw clean, recognizable ASCII art."
    } else {
        parent_prompt
    };
    format!(
        "You mutate instructions for an ASCII-art drawing model. Current instruction:\n\"{cur}\"\n\n\
         Write ONE different, short instruction that would lead to a clearly DIFFERENT drawing — vary \
         the density, orientation, character set, framing, or level of detail. Output ONLY the new \
         instruction on a single line, no quotes or commentary."
    )
}

/// **LLM-guided variator** (EvoPrompt/OPRO/PromptBreeder): mutates the genome's drawing prompt via the
/// model itself, instead of a fixed list of directives — richer variation → more varied drawings →
/// QD coverage can spread. The mutation is a model call, so `vary` is **foreground-only** (tests use
/// `AsciiVariator`, keeping the green-gate model-free).
pub struct LlmVariator {
    proposer: OpenAiChatProposer,
}

impl LlmVariator {
    pub fn new() -> Self {
        Self {
            proposer: OpenAiChatProposer::new(OpenAiChatConfig::ollama_qwen3_thinking()),
        }
    }
}

impl Default for LlmVariator {
    fn default() -> Self {
        Self::new()
    }
}

impl Variator for LlmVariator {
    fn vary(&mut self, _rng: &mut Rng, parent: &Genome) -> Vec<MutationKind> {
        let raw = self
            .proposer
            .try_propose(&ContextPack {
                input: mutation_prompt(&parent.prompt),
                retrieved: Vec::new(),
            })
            .unwrap_or_default();
        // Strip <think>/fences, take the first non-empty line as the new instruction.
        let new = extract_art(&raw)
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("")
            .to_string();
        if new.is_empty() {
            vec![] // model produced nothing usable → no variation this step
        } else {
            vec![MutationKind::Prompt(new)]
        }
    }
}

/// Prompt the model to emit a DRAWING PROGRAM (shape primitives) rather than raw ASCII — the
/// breakthrough action space. Pure + testable.
pub fn program_prompt(subject: &str, w: usize, h: usize) -> String {
    format!(
        "Draw a {subject} on a {w}x{h} character grid (x=0..{} left→right, y=0..{} top→bottom) by \
         composing these commands, ONE PER LINE:\n\
         rect X Y W H C   (rectangle outline, char C)\n\
         hline X Y LEN C  (horizontal line)\n\
         vline X Y LEN C  (vertical line)\n\
         put X Y C        (single character)\n\
         Think about the shape, then output ONLY the command lines (no commentary).",
        w.saturating_sub(1),
        h.saturating_sub(1)
    )
}

/// **Program-synthesis generator**: the model emits a shape program (which LLMs do well), and
/// [`run_program`] renders it deterministically (guaranteeing spatial correctness the base lacks). This
/// is the breakthrough generator — the evolved system draws by composition, not char-emission. The
/// genome's prompt is prepended as a style/approach directive. Foreground-only (model call).
pub struct ProgramGenerator {
    proposer: OpenAiChatProposer,
    pub w: usize,
    pub h: usize,
}

impl ProgramGenerator {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            proposer: OpenAiChatProposer::new(OpenAiChatConfig::ollama_qwen3_thinking()),
            w,
            h,
        }
    }
}

impl Generator for ProgramGenerator {
    fn generate(&mut self, genome: &Genome, subject: &str) -> String {
        let mut input = program_prompt(subject, self.w, self.h);
        if !genome.prompt.trim().is_empty() {
            input = format!("{}\n{input}", genome.prompt);
        }
        let raw = self
            .proposer
            .try_propose(&ContextPack {
                input,
                retrieved: Vec::new(),
            })
            .unwrap_or_default();
        // The model's output is a program; strip think/fences, render it deterministically to ASCII.
        run_program(&extract_art(&raw), self.w, self.h).render()
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

/// JSON-escape a string (the subset needed for `prompt`/`completion` corpus fields — quotes,
/// backslashes, newlines, control chars). Pure; avoids a serde dependency.
pub fn json_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

/// One MLX-LoRA training line: `{"prompt": …, "completion": …}` (the format `scripts/distill_lora.sh`
/// expects). Used to build the **teacher-distillation corpus** (instruction → the teacher's drawing).
pub fn corpus_line(prompt: &str, completion: &str) -> String {
    format!(
        "{{\"prompt\": \"{}\", \"completion\": \"{}\"}}",
        json_escape(prompt),
        json_escape(completion)
    )
}

/// The **self-distillation flywheel** state: a fixed seed library (resolvable by genome exemplar-skills)
/// plus a growing set of the being's OWN Claude-validated high-score drawings. Every generation
/// few-shots from the top learned drawings, so the being learns from its own validated best work — the
/// earned-intelligence thesis applied to ASCII. Shared (Rc<RefCell>) between the generator (reads) and
/// the evaluator (writes after judging). No model in this type — pure, loop-safe.
pub struct ExemplarStore {
    seed: BTreeMap<String, String>,
    /// **Best-per-niche** Claude-validated drawings, keyed by the (style×aspect) cell. Keeping the best
    /// of EACH shape-niche — rather than a global top-K — is the research-grounded guard against
    /// **entropy decay / diversity collapse**, the named self-improvement failure mode (B-STaR, ReST)
    /// that produced the first run's niches=1 plateau. The Claude judge is the persistent grounding
    /// against the companion drift failure.
    learned: BTreeMap<(u8, u8), (String, f64)>,
    threshold: f64,
    top_k: usize,
}

/// The (style-density, aspect) niche cell of a drawing, 4×4 buckets — the diversity axis for the flywheel.
fn niche_cell(art: &AsciiArt) -> (u8, u8) {
    let s = (art.style_axis() * 4.0) as u8;
    let a = (art.aspect_axis() * 4.0) as u8;
    (s.min(3), a.min(3))
}

impl ExemplarStore {
    pub fn new(seed: BTreeMap<String, String>, threshold: f64, top_k: usize) -> Self {
        Self {
            seed,
            learned: BTreeMap::new(),
            threshold,
            top_k,
        }
    }

    /// A shared store with the default seed library.
    pub fn shared(threshold: f64, top_k: usize) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self::new(
            default_exemplar_library(),
            threshold,
            top_k,
        )))
    }

    /// Resolve a genome's exemplar-skill id to its seed art.
    pub fn resolve(&self, id: &str) -> Option<&String> {
        self.seed.get(id)
    }

    pub fn seed_library(&self) -> &BTreeMap<String, String> {
        &self.seed
    }

    /// Offer a Claude-validated drawing to the flywheel: kept iff it clears the threshold AND beats the
    /// current best in ITS shape-niche (so diversity is preserved — we keep the best of each shape, not
    /// N copies of the single global best). Returns true if it became (or replaced) a niche's exemplar.
    pub fn learn(&mut self, art: &str, score: f64) -> bool {
        if score < self.threshold {
            return false;
        }
        let cell = niche_cell(&AsciiArt::parse(art));
        match self.learned.get(&cell) {
            Some((_, s)) if *s >= score => false, // a better drawing already holds this niche
            _ => {
                self.learned.insert(cell, (art.to_string(), score));
                true
            }
        }
    }

    /// The top-K learned drawings to inject as few-shot context — the best across DISTINCT niches
    /// (diverse by construction), highest-scored first.
    pub fn top_learned(&self) -> Vec<String> {
        let mut v: Vec<&(String, f64)> = self.learned.values().collect();
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        v.into_iter()
            .take(self.top_k)
            .map(|(a, _)| a.clone())
            .collect()
    }

    /// Number of distinct shape-niches with a learned exemplar (a diversity readout).
    pub fn learned_count(&self) -> usize {
        self.learned.len()
    }

    pub fn best_learned_score(&self) -> Option<f64> {
        self.learned
            .values()
            .map(|(_, s)| *s)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }
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
    /// Shared flywheel store: genome exemplar-skills resolve against its seed library, and its top
    /// learned (Claude-validated) drawings are injected as few-shot context into every generation.
    store: Rc<RefCell<ExemplarStore>>,
}

impl OllamaGenerator {
    /// Standalone generator with a private store (no flywheel sharing — e.g. one-off draws).
    pub fn new() -> Self {
        Self::with_store(ExemplarStore::shared(0.5, 3))
    }

    /// Generator sharing a flywheel store with the evaluator (so learned drawings feed back).
    pub fn with_store(store: Rc<RefCell<ExemplarStore>>) -> Self {
        Self {
            proposer: OpenAiChatProposer::new(OpenAiChatConfig::ollama_qwen3_thinking()),
            store,
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
        let (input, retrieved) = {
            let store = self.store.borrow();
            let (input, mut retrieved) = draw_prompt(genome, subject, store.seed_library());
            // Flywheel: few-shot from the being's own validated-best drawings.
            for art in store.top_learned() {
                retrieved.push(format!("Example of good ASCII art:\n{art}"));
            }
            (input, retrieved)
        };
        let raw = self
            .proposer
            .try_propose(&ContextPack { input, retrieved })
            .unwrap_or_default();
        extract_art(&raw)
    }
}

// ---------------------------------------------------------------------------------------------
// TOOLSPACE: the being evolves WHICH drawing strategy to use — the action space itself is evolvable.
// "Nothing human" (operator, 2026-06-22): the operator provides a capability-bounded toolspace; the
// being's closed-surface evolution (a `ToolPolicy` mutation) DISCOVERS which tool/composition works,
// selected by fitness — strategy-discovery happens IN the being, not the operator. Safety is the
// toolspace BOUNDARY (every tool is sandboxed/judged, none can self-grant capability), not a human in
// the loop. Maximal autonomy inside a bounded enclosure.
// ---------------------------------------------------------------------------------------------

/// A drawing strategy the being can select via its (closed-surface) `tool_policy`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawTool {
    /// Emit ASCII directly (the base's native, weak action).
    Direct,
    /// Emit a shape PROGRAM rendered deterministically (transcends spatial-char weakness).
    Program,
    /// Draw, get a Claude critique, then redraw with the feedback (Self-Refine; weak-gen+strong-critic).
    Refine,
}

impl DrawTool {
    pub const ALL: [DrawTool; 3] = [DrawTool::Direct, DrawTool::Program, DrawTool::Refine];

    /// Decode the being's chosen tool from its genome's `tool_policy` (default = Direct).
    pub fn from_genome(g: &Genome) -> DrawTool {
        match g.tool_policy.as_slice() {
            b"program" => DrawTool::Program,
            b"refine" => DrawTool::Refine,
            _ => DrawTool::Direct,
        }
    }

    pub fn tag(self) -> &'static [u8] {
        match self {
            DrawTool::Direct => b"direct",
            DrawTool::Program => b"program",
            DrawTool::Refine => b"refine",
        }
    }
}

/// Ask Claude (frontier critic) for specific, actionable fixes to a drawing. Foreground (`claude -p`).
pub fn claude_critique(subject: &str, art: &str) -> String {
    let p = format!(
        "This is an attempt at ASCII art of a {subject}:\n{art}\n\n\
         In 1-2 sentences give SPECIFIC, actionable fixes to make it read more clearly as a {subject}. \
         No preamble."
    );
    match std::process::Command::new("claude")
        .arg("-p")
        .arg(&p)
        .output()
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => String::new(),
    }
}

/// The toolspace generator: dispatches to the being's genome-selected [`DrawTool`]. Foreground (model
/// calls; Refine also spends a `claude -p` critique). The being evolves the tool via `ToolPolicy`.
pub struct ToolspaceGenerator {
    proposer: OpenAiChatProposer,
    pub w: usize,
    pub h: usize,
    /// HARD CAP on `claude -p` critique calls (CLAUDE.md: a runaway loop must never drain the
    /// subscription). The Refine tool's critiques are metered separately from the judge's salary; when
    /// this is exhausted, Refine degrades to a plain draw (no critique). `0` disables critiques.
    critique_budget: u64,
    pub critiques_made: u64,
}

impl ToolspaceGenerator {
    /// Default critique budget of 12 (bounds Refine's frontier spend alongside the judge salary).
    pub fn new(w: usize, h: usize) -> Self {
        Self::with_critique_budget(w, h, 12)
    }

    pub fn with_critique_budget(w: usize, h: usize, critique_budget: u64) -> Self {
        Self {
            proposer: OpenAiChatProposer::new(OpenAiChatConfig::ollama_qwen3_thinking()),
            w,
            h,
            critique_budget,
            critiques_made: 0,
        }
    }

    fn ask(&mut self, input: String, retrieved: Vec<String>) -> String {
        extract_art(
            &self
                .proposer
                .try_propose(&ContextPack { input, retrieved })
                .unwrap_or_default(),
        )
    }

    fn direct(&mut self, genome: &Genome, subject: &str) -> String {
        let (input, retrieved) = draw_prompt(genome, subject, &default_exemplar_library());
        self.ask(input, retrieved)
    }

    fn program(&mut self, subject: &str) -> String {
        let raw = self.ask(program_prompt(subject, self.w, self.h), Vec::new());
        run_program(&raw, self.w, self.h).render()
    }

    fn refine(&mut self, genome: &Genome, subject: &str) -> String {
        let first = self.direct(genome, subject);
        // Salary cap on critiques: exhausted budget (or empty draw) → no critique, return the draw.
        if first.trim().is_empty() || self.critiques_made >= self.critique_budget {
            return first;
        }
        self.critiques_made += 1;
        let crit = claude_critique(subject, &first);
        if crit.trim().is_empty() {
            return first;
        }
        let input = format!(
            "Your ASCII {subject}:\n{first}\n\nA critic says: {crit}\n\n\
             Redraw the {subject} applying those fixes. Output ONLY the art."
        );
        let revised = self.ask(input, Vec::new());
        if revised.trim().is_empty() {
            first
        } else {
            revised
        }
    }
}

impl Generator for ToolspaceGenerator {
    fn generate(&mut self, genome: &Genome, subject: &str) -> String {
        match DrawTool::from_genome(genome) {
            DrawTool::Direct => self.direct(genome, subject),
            DrawTool::Program => self.program(subject),
            DrawTool::Refine => self.refine(genome, subject),
        }
    }
}

/// Variator that evolves the being's TOOL choice (action-space search) plus a prompt nudge — so the
/// being discovers the best drawing strategy itself, via the closed `ToolPolicy`/`Prompt` surface.
pub struct ToolspaceVariator {
    pub style_directives: Vec<String>,
}

impl Default for ToolspaceVariator {
    fn default() -> Self {
        Self {
            style_directives: vec![
                "Use bold, simple shapes.".into(),
                "Add detail and texture.".into(),
                "Keep it minimal and clean.".into(),
            ],
        }
    }
}

impl Variator for ToolspaceVariator {
    fn vary(&mut self, rng: &mut Rng, _parent: &Genome) -> Vec<MutationKind> {
        if rng.next_u64().is_multiple_of(2) {
            // Switch the drawing tool (explore the action space).
            let t = DrawTool::ALL[(rng.next_u64() as usize) % DrawTool::ALL.len()];
            vec![MutationKind::ToolPolicy(t.tag().to_vec())]
        } else if !self.style_directives.is_empty() {
            let i = (rng.next_u64() as usize) % self.style_directives.len();
            vec![MutationKind::Prompt(self.style_directives[i].clone())]
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawtool_decodes_from_genome_and_being_evolves_it() {
        let mut g = Genome::default();
        assert_eq!(DrawTool::from_genome(&g), DrawTool::Direct); // default action
        g.tool_policy = b"program".to_vec();
        assert_eq!(DrawTool::from_genome(&g), DrawTool::Program);
        g.tool_policy = b"refine".to_vec();
        assert_eq!(DrawTool::from_genome(&g), DrawTool::Refine);
        g.tool_policy = b"unknown".to_vec();
        assert_eq!(DrawTool::from_genome(&g), DrawTool::Direct); // unknown → safe default
    }

    #[test]
    fn toolspace_variator_explores_tools_and_prompts() {
        let mut v = ToolspaceVariator::default();
        let mut rng = Rng::new(7);
        let (mut tools, mut prompts) = (0, 0);
        for _ in 0..60 {
            for m in v.vary(&mut rng, &Genome::default()) {
                match m {
                    MutationKind::ToolPolicy(_) => tools += 1,
                    MutationKind::Prompt(_) => prompts += 1,
                    _ => {}
                }
            }
        }
        // The being explores the ACTION SPACE (tool choice) AND prompts — strategy-discovery in-being.
        assert!(tools > 0 && prompts > 0);
    }

    #[test]
    fn canvas_dsl_composes_a_house_from_primitives() {
        // The kind of program an 8B can plausibly emit (compositional, not spatial-char emission).
        let prog = "rect 0 1 7 5 #\nhline 1 0 5 ^\nput 3 3 +";
        let art = run_program(prog, 14, 9);
        assert!(StructuralGate::default().check(&art).is_ok()); // a real composed drawing
        let r = art.render();
        assert!(r.contains('#') && r.contains('+') && r.contains('^'));
        assert!(art.height() >= 5);
    }

    #[test]
    fn run_program_ignores_commentary_and_malformed_lines() {
        let art = run_program("Here is a box:\nrect 0 0 5 4 #\noops not a command", 10, 8);
        assert!(art.render().contains('#') && art.height() >= 4);
    }

    #[test]
    fn program_prompt_documents_the_dsl_and_subject() {
        let p = program_prompt("cat", 16, 10);
        for kw in ["cat", "rect", "hline", "vline", "put"] {
            assert!(p.contains(kw), "missing {kw}");
        }
    }

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
    fn flywheel_keeps_best_per_niche_validated_only() {
        let mut store = ExemplarStore::new(BTreeMap::new(), 0.5, 5);
        assert!(!store.learn("aa\nbb\ncc", 0.2)); // below threshold → rejected
        assert!(store.learn(" /\\\n(o.o)\n >^<", 0.6)); // validated → learned (1 niche)
        assert_eq!(store.best_learned_score(), Some(0.6));
        // same shape-niche, HIGHER score → replaces (a better version), niche count unchanged
        let before = store.learned_count();
        assert!(store.learn(" /\\\n(o.o)\n >^<", 0.9));
        assert_eq!(store.learned_count(), before);
        assert_eq!(store.best_learned_score(), Some(0.9));
        // same niche, lower score → does NOT displace the better incumbent
        assert!(!store.learn(" /\\\n(o.o)\n >^<", 0.7));
    }

    #[test]
    fn evaluator_feeds_the_flywheel_on_high_scores() {
        let store = ExemplarStore::shared(0.4, 3);
        let mut e = AsciiEvaluator::with_store(
            CannedGenerator,
            StructuralJudge,
            vec!["cat".into(), "house".into(), "tree".into()],
            store.clone(),
        );
        e.evaluate(&Genome::default());
        assert!(
            store.borrow().learned_count() >= 1,
            "validated drawings should enter the flywheel"
        );
    }

    #[test]
    fn corpus_line_escapes_art_safely() {
        let line = corpus_line("Draw a \"cat\"", " /\\_/\\\n(o.o)");
        // valid: escapes quotes, backslashes, newlines so it's a parseable JSONL record
        assert!(line.contains("\\\"cat\\\""));
        assert!(line.contains("/\\\\_/\\\\")); // backslashes doubled
        assert!(line.contains("\\n")); // newline escaped, not literal
        assert!(!line.contains('\n')); // no raw newline in the line
    }

    #[test]
    fn mutation_prompt_references_the_current_instruction() {
        let p = mutation_prompt("Draw blocky art.");
        assert!(p.contains("Draw blocky art.") && p.to_lowercase().contains("different"));
        // empty parent → falls back to a base instruction, still asks for a different one
        assert!(mutation_prompt("").to_lowercase().contains("different"));
    }

    #[test]
    fn aspect_axis_separates_tall_from_wide() {
        let tower = AsciiArt::parse("/\\\n||\n||\n||");
        let wide = AsciiArt::parse("______\n#    #\n######");
        assert!(tower.aspect_axis() < wide.aspect_axis());
        for a in [&tower, &wide] {
            assert!((0.0..=1.0).contains(&a.aspect_axis()));
        }
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
