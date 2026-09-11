---
description: Investigate and fix a GitHub issue end to end
---

Fix issue `$ARGUMENTS`.

1. `gh issue view $ARGUMENTS` — read the report and any linked discussion.
2. Reproduce it. If you cannot reproduce, say so and ask for what is missing before changing code.
3. Locate the root cause. Read `IMPLEMENTATION_PLAN.md` for the intended behaviour of the area, and
   the matching `.claude/rules/` file for its conventions.
4. Write a failing test first.
5. Fix the cause, not the symptom.
6. Run `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings`, and the frontend tests
   if the change touches `apps/web`.
7. If the change touches retrieval, chunking, or a system prompt, run the eval harness and report
   the before/after numbers.
8. Commit on a branch named `fix/<issue-number>-<slug>`, referencing the issue in the message.

Report what the root cause actually was, not just that it is fixed.
