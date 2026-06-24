# How to Evaluate and Deploy Local vs Frontier Models: A Practical Guide

*A companion to the field report (`local-model-eval-report.md`). The report is the experiment; this is the
playbook. Grounded in measured results on one hard task (text-to-SQL, BIRD) plus the tooling that already
exists off the shelf. Numbers are one data point; the decision framework generalizes.*

---

## 1. The one idea to internalize: there are two ceilings

A model has a **static ceiling** (what it gets one-shot, or with sampling, or after fine-tuning) and a
**tool-assisted ceiling** (what it gets when it can act, observe, and correct). These are different numbers,
and the gap is large.

Measured example (4-bit Qwen3-8B on BIRD simple/moderate):

| Condition | Accuracy |
|---|---:|
| One-shot | ~37% |
| Sampling / voting / fine-tuning / static rich schema | still ~37 to 40% |
| Interactive tools (explore the DB, then run-and-fix) | **~48%** (confirmed n=80) |
| Frontier one-shot (Claude) | ~75% |

The jump from 37 to 48 came from scaffolding, with zero training, zero extra parameters, and zero API cost.
That gap is not a property of the model. It is a property of the harness around the model.

**Decision rule:** pick the smallest model whose *tool-assisted* ceiling clears your task's accuracy bar,
then spend on scaffolding before you spend on parameters.

---

## 2. How to evaluate (so the numbers mean something)

Most public benchmarks report the static ceiling (one-shot or a fixed pipeline). That understates usable
accuracy and ranks the wrong unit. The unit that matters is "model plus scaffold."

A trustworthy eval has five properties:

1. **A free, deterministic verifier.** Execution accuracy for SQL/code, unit tests, schema/type checks,
   regex. No human grading, no LLM-as-judge. This is what makes the eval cheap and repeatable.
2. **A with-tools vs without-tools axis.** Report both. The delta is usually your biggest finding.
3. **A difficulty split** (simple / moderate / hard) and, if relevant, a **task-shape split**
   (homogeneous, one skill many instances, vs heterogeneous, every item different). A single accuracy hides
   both.
4. **A frontier baseline** on the same held-out set, measured identically (not an estimate).
5. **Honest n.** Small held-out sets are noisy. A few-point difference at n=40 is within noise; confirm at
   n=80 or more before claiming a win. (We learned this the hard way: a 47% reading at n=40 needed an n=80
   rerun to confirm as a real 48%.)

**Gotchas that cost real time:**
- Eval truncation. If the eval's max output length is shorter than the model's answer, you silently chop it
  and score zero. This once looked like total failure that was actually 98%. Always diagnose
  model-versus-eval before believing a bad score.
- One model at a time on small RAM. Do not co-load. Fine-tuning needs aggressive memory settings or it OOMs.
- Compact the prompt. Sending `table(col, col)` instead of full DDL was about 1.5x faster with no accuracy
  loss.
- Disable library progress bars in batch runs. They corrupt parsed output.

---

## 3. The levers, ranked by return on investment

1. **Agent loop plus a free verifier.** Let the model run its output, see the error or result, and fix it.
   Cheapest, highest impact. This is the 37 to 48 jump.
2. **A curated, minimal toolspace.** Give the model exactly the tools it needs (for SQL: foreign-key paths,
   on-demand sample values, execute). Then add tools one at a time and keep only those that measurably raise
   accuracy. Important nuance: a *richer* toolset *hurt* the weak model (it dropped from 48 to 37 with extra
   tools). Curate, do not pile on.
3. **Quantization to fit the machine.** 4-bit ran at about 5 to 6 GB and was capability-bound, not
   quant-bound, on this task. Prefer quantization over streaming weights from SSD, which adds per-token
   latency that kills an interactive loop.
4. **Best-of-N or self-consistency.** Pays off mainly when the base (or the agent) is already strong enough
   that the correct answer is frequent. Over a weak one-shot model it did nothing here. Over a stronger agent
   it can help. Costs N times the compute.
5. **Fine-tuning at scale, or RL.** This is what moves the *base* capability ceiling, and it is what the
   published wins use (roughly 900k synthetic traces, or RL). It is heavyweight and usually a
   powerful-machine activity. Few-shot local fine-tuning did not generalize on a heterogeneous task.

---

## 4. What is already available off the shelf

You rarely need to build from scratch. The pieces exist.

- **Local runtimes:** Ollama and llama.cpp (GGUF, broad hardware), MLX (Apple Silicon), vLLM and TGI (server
  and cloud GPUs).
- **Quantization:** GGUF, AWQ, GPTQ, bitsandbytes. 8-bit and 4-bit are the common stops. Pre-quantized
  community builds exist for most open-weight models.
- **Open-weight models worth benchmarking:** Qwen, GLM, Gemma, DeepSeek, Llama families. Use a frontier
  model (Claude, GPT) as the baseline and, optionally, as a teacher for trace generation.
- **Agent / tool-use frameworks:** native tool-calling APIs, plus ReAct-style loops, DSPy, LangChain,
  LlamaIndex, and lightweight options like smolagents. The pattern is more important than the framework:
  propose, execute, observe, correct.
- **Verifiers:** the cheapest part. SQL and code execute. Schemas and types check. Tests run. If your task
  has a free verifier, use it everywhere (eval, gating, best-of-N selection, training signal).
- **Text-to-SQL specifically:** decomposition (DIN-SQL), multi-agent pipelines (MAC-SQL), candidate
  generation plus selection (CHASE-SQL), schema linking and retrieval. These are scaffolding patterns, not
  new models, and they are exactly the "with-tools" condition.
- **Self-improvement patterns:** Voyager (a growing, verified skill library of reusable code), Darwin Godel
  Machine and ADAS (agents that evolve their own code and tools, validated on a benchmark), STaR / rejection
  fine-tuning (distill verified traces back into weights), best-of-N with a verifier.

The frontier of the field right now is making the agent **evolve its own toolspace**, keeping only what a
benchmark says helps. That is the direction worth watching and the one we are actively pushing.

---

## 5. Decision framework and recipes

Match the recipe to the requirement, not to the hype.

- **Cheap, private, good-enough on an internal task:** local 8B at 4-bit, plus a curated tool loop, plus a
  free verifier. Zero API cost, data stays on the machine, and you recover a large fraction of the
  achievable accuracy. Add a verifier-gated retry or a human in the loop for the rest.
- **High accuracy required:** a powerful model (frontier API, or a large open-weight model on a GPU box),
  plus the same tool loop and verifier, plus best-of-N. Optionally fine-tune at scale if you have the data.
- **Best of both, in practice:** route by difficulty. Let the local model plus tools handle the easy and
  moderate cases it can verify, and escalate only the hard or low-confidence cases to a powerful model. The
  free verifier is what makes the routing safe.

The underlying object is an **accuracy-versus-cost frontier**. Local-plus-tools maps the cheap end. A
powerful model on a powerful machine maps the high-accuracy end. Knowing both lets you make build-versus-buy
calls with numbers instead of vibes.

---

## 6. What changes with powerful models on powerful machines

The levers transfer, but not all in the same way.

- **Tools and verifiers still help**, including frontier models. Agentic SQL systems on strong models reach
  70 percent and up. "Always add a tool loop and a verifier" is universal.
- **The curate-do-not-pile-on rule flips.** Strong models can orchestrate richer toolspaces without getting
  distracted. Tool design should scale with model capability. The "more tools hurt" effect is a weak-model
  phenomenon.
- **The heavyweight levers become available.** Big machines let you keep large models resident (no 4-bit
  forced, no one-model-at-a-time), run real best-of-N and self-consistency (which pay off more when the base
  is strong), and fine-tune or RL at the scale that actually raises the base ceiling. Those are the levers
  that were out of scope locally.
- **The evaluation methodology is identical.** Evaluate powerful models the same way (with-tools vs
  without-tools, by difficulty, against a baseline, at honest n) or you will publish misleading one-shot
  numbers. The protocol in section 2 is the transferable asset.

---

## 7. Where this generalizes, and where it does not

The mechanisms (tools plus a free verifier raise the effective ceiling; protocol matters more than metric;
curate tools for weak models; quantize to fit) should transfer to any task with a cheap verifier: code, SQL,
math, structured extraction, anything you can check programmatically. They transfer least to open-ended
generation, where no free verifier exists, which is exactly the case where you lean on a more powerful model
instead.

One honest limit: this is one task, one benchmark, a difficulty slice, modest n. Treat the numbers as a
worked example and the framework as the reusable part. Run your own task through section 2 before trusting any
single accuracy, including these.

---

## 8. Cross-task generalization: the lever-map measured across domains

The same harness (one Task interface + a generic agent loop + a free verifier) was run across six verifiable
task types. Two things generalize and one varies predictably:

| Task (verifier) | One-shot | Agent-loop | Benefit |
|---|---:|---:|---|
| SQL / BIRD (execution vs gold) | 37% | 48% | **+11** |
| Code / MBPP (unit tests) | 70% | 71% | +1 |
| Code / HumanEval (unit tests) | 84% | 86% | +2 |
| Reasoning / GSM8K (exact match) | ~100% | — | saturated |

- **What generalizes (structure):** the harness, the free verifier, and the agent loop port to any verifiable
  domain unchanged. That portability is the asset.
- **What varies (benefit):** the agent loop's payoff is **inverse to the model's one-shot accuracy** on the
  task. It is large where the model is weak but its errors are fixable (heterogeneous SQL) and near-zero where
  the model is already strong (code, reasoning). Don't pay for scaffolding where the base model already wins.
- **Verifier taxonomy matters for routing.** A *correctness* verifier (unit tests available at inference) lets
  the local model self-certify answers with no gold — so you can accept the verified-correct locally for free
  and escalate only the residual to a powerful model, reaching near-frontier accuracy at a fraction of the
  cost. An *execution-only* signal ("the query ran") can't certify correctness, so it only catches hard errors.

---

## 9. Cost-optimal routing: the practical accuracy/cost frontier

With a **correctness verifier** (unit tests available at inference), a local model can *self-certify*: accept
any answer that passes the tests (provably correct, no gold needed, no frontier call) and escalate only the
rest. Measured on a 4-bit 8B (agent-loop local tier, n=80):

| Task | Self-certified locally (free) | Escalated | Routed accuracy (frontier≈0.75) | Frontier cost |
|---|---:|---:|---:|---:|
| MBPP | 71% | 29% | 93% | 29% |
| HumanEval | 88% | 12% | 97% | 12% |

The verifier is the moat: acceptance is **safe by construction** (accepted answers pass the tests, so routing
never accepts a wrong one). You reach near-frontier accuracy while paying the frontier for only the small
residual. This is the build-vs-buy frontier with numbers: **local + verifier for the verifiable majority,
escalate the tail.** It applies wherever a correctness verifier exists (code, SQL-with-expected-output,
schema/type checks); an execution-only signal ("it ran") can't self-certify and only catches hard errors.
