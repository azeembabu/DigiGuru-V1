---
description: Run the pre-deploy gate and deploy to an environment
---

Deploy to `$ARGUMENTS` (`staging` or `prod`).

**Gate — every item must pass before anything is deployed. Stop and report on the first failure.**

1. `cargo test --workspace` and `cargo clippy --all-targets -- -D warnings`
2. Frontend: `pnpm --filter web lint` and `pnpm --filter web test`
3. Migrations: review pending SQL, confirm each is forward-only and additive; no edits to applied
   migrations
4. Eval harness: golden sets pass tolerance for faithfulness and context precision
5. Non-negotiable evidence: `wb_violation == 0` in the e2e run, red-team corpus at 100 %
   termination, out-of-syllabus corpus at 100 % abstention
6. Secrets: confirm the target environment resolves every secret from the manager — no env-file
   fallbacks in prod
7. Cost dashboard: current per-session cost within budget

For `prod`, additionally confirm a successful backup within the last 24 hours, then ask for explicit
confirmation before proceeding.

Deploy, then verify: health endpoint, one synthetic classroom session end to end, and the first
five minutes of error and latency metrics. Report the deployed SHA and the verification results.
