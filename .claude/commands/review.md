---
description: Review the current diff against Digi Guru non-negotiables and rules
---

Review the uncommitted changes (or `$ARGUMENTS` if a branch, PR number, or path is given).

Run `git diff` first to see what actually changed, then read the relevant rule files in
`.claude/rules/` for the areas the diff touches.

Check, in priority order:

1. **Non-negotiables.** Does anything weaken NN-1 (whiteboard-first), NN-2 (first-login flag),
   NN-3 (server-side 20-minute cap), NN-4 (RAG-only abstention), or NN-5 (jailbreak termination)?
   Any weakening is a blocking finding regardless of how small the diff is.
2. **Metadata filtering.** Every Qdrant query carries `program_id`, `semester`, and `block_no`.
3. **Authorization.** Every new endpoint declares a capability. No implicit allow.
4. **Error exposure.** Nothing but `PublicError` reaches a client.
5. **Audio path hygiene.** No `unwrap()`, no blocking calls, no unbounded buffers.
6. **PII.** No phone, email, or name in any log or trace.
7. Correctness bugs, then reuse and simplification opportunities.

Report findings most-severe first, each with file, line, and a concrete failure scenario. If
nothing is wrong, say so plainly rather than inventing findings.
