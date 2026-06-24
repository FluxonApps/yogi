# Experiment TLDR: local-model democratization (Yogi)

## Where the experiment stands

**Question:** can a sub-frontier local model (4-bit Qwen3-8B on a 16 GB laptop) be made genuinely good at
real tasks, for free and safely. That is Yogi's democratization thesis.

### What held up

- **Skill grain works, for free.** On homogeneous tasks (one rule, many instances) the self-distillation
  ratchet goes from cold 0 to about 98%, within safe-by-construction limits (F1 through F9). This is the
  real result.
- **Tool-use is the usability win.** On a hard real task (BIRD text-to-SQL), a 4-bit 8B given a `run_sql`
  tool plus execution feedback reached its ceiling gold-free in about 2 turns (40%), beating sampling,
  voting, and fine-tuning. Scaffolding beats parameters for usability.

### What did not work, now well mapped

- **A hard ceiling around 40% on heterogeneous SQL,** confirmed across every lever: distillation in 5 forms
  (including frontier-correct traces and scaling to about 100), self-consistency, pass@k (plateaus at k=8),
  tool-use, internalized self-critique, and rich-schema enrichment. The frontier baseline on the same set
  was 75%, so the gap is the base model's generation capability, not selection, sampling, or schema.
- **Salary bought correct traces but not generalization,** and it did not amortize. A crutch here, not a
  frontier-push.

### One line

Democratization is real and free at the skill grain and the tool-use loop. Heterogeneous-domain generation
capability is bounded locally around 40%, and only scale (roughly 900k examples) or RL raises it.

## What to try next, ranked

1. **Toolspace evolution.** The most Yogi-native bet. We distilled the tool-use loop but never let the being
   evolve the tools themselves inside the closed mutation surface. Tools are where the leverage is and the
   ceiling does not bind. This ratchets on the toolspace, not the weights, and stays safe by construction.
   My top pick.
2. **Verifier-internalization, on the right task.** Learn a selector to pick the correct answer among N
   candidates, but demonstrate it where capability is high and verification is the gap (an F1 through F9
   class skill), not on capability-bounded BIRD where it cannot help.
3. **Multi-model matrix.** Cheap and externally useful. Run the existing harness across GLM-5.2, Gemma, and
   DeepSeek, by quant level and with or without tools, to produce the model comparison table the report
   currently lacks.
4. **Cross the ceiling itself.** Different scope. Scale (about 900k synthetic traces) or RL would lift the
   40%, but it leaves the local, cheap, few-shot setting. Treat it as a separate effort.

If it were my call, number 1, toolspace evolution. It is on-thesis, safe, and it sidesteps the one wall we
proved is real.
