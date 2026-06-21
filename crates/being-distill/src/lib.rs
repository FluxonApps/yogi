//! `being-distill` — the distillation flywheel's **decision machinery** (build-spec §M3, D-M3-4).
//!
//! The flywheel is: detect where the student is weak but the teacher succeeds → collect teacher traces
//! on exactly those tasks → fine-tune a student (LoRA) → **gate** the result before promoting it. The
//! fine-tuning loads a model and is foreground/heavy; **this crate is the pure half** — which tasks to
//! distill on ([`gap_set`]) and whether a freshly-distilled `DomainModel` may be promoted
//! ([`PromotionGate`]). Pure and loop-safe: no model, no clock, no I/O. The bench is the only judge.
//!
//! The M3 acceptance has two clauses, both encoded here:
//! 1. **gap closure** — distillation closes the gap on `(teacher-success ∩ student-weak)` for a domain
//!    by at least a pre-registered per-domain margin;
//! 2. **non-inferiority** — every promotion re-clears the mixed-set floor, i.e. the new student does not
//!    regress the broad mixed set (no catastrophic forgetting).

/// Mean of a boolean pass/fail slice (0.0 for empty).
fn pass_rate(passes: &[bool]) -> f64 {
    if passes.is_empty() {
        return 0.0;
    }
    passes.iter().filter(|p| **p).count() as f64 / passes.len() as f64
}

/// The distillation target set: indices where the **teacher** succeeds but the **student** does not —
/// the `(teacher-success ∩ student-weak)` set (build-spec §M3). Distilling elsewhere is wasted: tasks
/// the student already passes need no teaching, and tasks the teacher also fails have no signal to give.
/// Requires aligned, equal-length slices (same task ordering); extra teacher entries beyond the
/// student's length are ignored.
pub fn gap_set(teacher_pass: &[bool], student_pass: &[bool]) -> Vec<usize> {
    teacher_pass
        .iter()
        .zip(student_pass)
        .enumerate()
        .filter(|(_, (t, s))| **t && !**s)
        .map(|(i, _)| i)
        .collect()
}

/// Fraction of the gap set the **new** (post-distill) student now passes. Returns 0.0 for an empty gap
/// (nothing to close ⇒ no closure to claim).
pub fn gap_closure(new_student_pass: &[bool], gap: &[usize]) -> f64 {
    if gap.is_empty() {
        return 0.0;
    }
    let closed = gap
        .iter()
        .filter(|&&i| new_student_pass.get(i).copied().unwrap_or(false))
        .count();
    closed as f64 / gap.len() as f64
}

/// The M3 promotion gate. Constants are pre-registered (the §7 table at M3).
#[derive(Clone, Copy, Debug)]
pub struct PromotionGate {
    /// Minimum fraction of the `(teacher∩weak)` gap the new student must close.
    pub gap_margin: f64,
    /// Allowed regression on the broad mixed set (non-inferiority slack); the new student must score
    /// at least `mean(old) - ni_epsilon` (no catastrophic forgetting).
    pub ni_epsilon: f64,
}

/// The gate's verdict — every component is reported so the decision is auditable (the bench is judge).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PromotionVerdict {
    pub promoted: bool,
    /// Closure achieved on the gap set.
    pub gap_closure: f64,
    /// `mean(new) - mean(old)` on the mixed set (negative = regression).
    pub mixed_delta: f64,
    pub gap_ok: bool,
    pub non_inferior: bool,
}

impl PromotionGate {
    /// Decide whether to promote the freshly-distilled student. All slices are bench pass/fail:
    /// `teacher`/`student_old` aligned over the **domain** tasks (to compute the gap), `student_new`
    /// the new student over the same domain tasks, and `mixed_old`/`mixed_new` over the **broad mixed
    /// set** (to check non-inferiority). Promote iff the gap is closed by `gap_margin` AND the mixed
    /// set is non-inferior.
    pub fn evaluate(
        &self,
        teacher: &[bool],
        student_old: &[bool],
        student_new: &[bool],
        mixed_old: &[bool],
        mixed_new: &[bool],
    ) -> PromotionVerdict {
        let gap = gap_set(teacher, student_old);
        let gap_closure = gap_closure(student_new, &gap);
        let mixed_delta = pass_rate(mixed_new) - pass_rate(mixed_old);
        let gap_ok = gap_closure >= self.gap_margin;
        let non_inferior = mixed_delta >= -self.ni_epsilon;
        PromotionVerdict {
            promoted: gap_ok && non_inferior,
            gap_closure,
            mixed_delta,
            gap_ok,
            non_inferior,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gap_set_is_teacher_success_intersect_student_weak() {
        // teacher: pass pass fail pass ; student: fail pass fail fail
        let teacher = [true, true, false, true];
        let student = [false, true, false, false];
        // gap = teacher-pass AND student-fail = indices 0 and 3 (index 1 student already passes;
        // index 2 the teacher also fails → no signal).
        assert_eq!(gap_set(&teacher, &student), vec![0, 3]);
    }

    #[test]
    fn gap_closure_counts_only_the_gap_indices() {
        let gap = vec![0, 3];
        // new student passes index 0 but not 3 → closed 1 of 2.
        let new = [true, true, false, false];
        assert!((gap_closure(&new, &gap) - 0.5).abs() < 1e-9);
        // empty gap → 0.0 (nothing to close)
        assert_eq!(gap_closure(&new, &[]), 0.0);
    }

    fn gate() -> PromotionGate {
        PromotionGate {
            gap_margin: 0.5,
            ni_epsilon: 0.05,
        }
    }

    #[test]
    fn promotes_when_gap_closed_and_mixed_non_inferior() {
        let teacher = [true, true, true, true];
        let student_old = [false, false, true, true]; // gap = {0,1}
        let student_new = [true, true, true, true]; // closes both → closure 1.0
        let mixed_old = [true, true, false, true]; // 0.75
        let mixed_new = [true, true, true, true]; // 1.00 (improved)
        let v = gate().evaluate(&teacher, &student_old, &student_new, &mixed_old, &mixed_new);
        assert!(v.promoted);
        assert!((v.gap_closure - 1.0).abs() < 1e-9);
        assert!(v.mixed_delta > 0.0);
    }

    #[test]
    fn rejects_catastrophic_forgetting_even_if_gap_closed() {
        let teacher = [true, true, true, true];
        let student_old = [false, false, true, true]; // gap = {0,1}
        let student_new = [true, true, true, true]; // gap fully closed
                                                    // but the mixed set regressed badly (0.90 → 0.60, delta -0.30 < -ni_epsilon)
        let mixed_old = [true, true, true, true, true, true, true, true, true, false];
        let mixed_new = [
            true, true, true, true, true, true, false, false, false, false,
        ];
        let v = gate().evaluate(&teacher, &student_old, &student_new, &mixed_old, &mixed_new);
        assert!(v.gap_ok);
        assert!(!v.non_inferior);
        assert!(
            !v.promoted,
            "must not promote a student that forgot the broad set"
        );
    }

    #[test]
    fn rejects_when_gap_not_closed_enough() {
        let teacher = [true, true, true, true];
        let student_old = [false, false, false, false]; // gap = {0,1,2,3}
        let student_new = [true, false, false, false]; // closes 1/4 = 0.25 < 0.5 margin
        let mixed = [true, true, true, true];
        let v = gate().evaluate(&teacher, &student_old, &student_new, &mixed, &mixed);
        assert!(!v.gap_ok);
        assert!(v.non_inferior); // mixed unchanged
        assert!(!v.promoted);
    }

    #[test]
    fn empty_gap_does_not_promote() {
        // Student already matches the teacher everywhere → no gap → nothing distillation can claim.
        let teacher = [true, true, true];
        let student_old = [true, true, true];
        let student_new = [true, true, true];
        let mixed = [true, true, true];
        let v = gate().evaluate(&teacher, &student_old, &student_new, &mixed, &mixed);
        assert_eq!(v.gap_closure, 0.0);
        assert!(!v.promoted);
    }
}
