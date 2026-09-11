---
description: Run the RAG golden-set evaluation and report regression
---

Run the evaluation harness for `$ARGUMENTS` (a block id, a program, or `all`).

1. `cargo run -p evals -- --scope $ARGUMENTS`
2. Report these metrics, current vs. the last recorded baseline:
   - context precision
   - context recall
   - faithfulness
   - answer relevancy
   - abstention accuracy on the out-of-syllabus set (must be 100 %)
   - median tokens per turn and estimated cost per session
3. For every regression beyond tolerance, identify which change caused it — chunker, embedder,
   filter, reranker, threshold, or system prompt — and quote the specific failing questions.
4. If everything passes, record the run as the new baseline.

Never report a pass without showing the numbers. If the harness itself fails to run, say that
plainly rather than reporting partial results as success.
