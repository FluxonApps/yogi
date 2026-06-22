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
            qsum += if self.gate.check(&art).is_ok() {
                self.judge.score(&subject, &art).clamp(0.0, 1.0)
            } else {
                0.0
            };
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
