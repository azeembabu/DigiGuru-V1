---
name: deploy
description: Use when deploying Digi Guru to staging or production, cutting a release, or running the pre-release verification gate.
---

# Deploy

## Environments

| Env | Stack | Data |
|-----|-------|------|
| `local` | docker-compose: Postgres, Redis, Qdrant, mock Live server | seeded fixtures |
| `staging` | full stack, real Gemini Live, synthetic students | anonymised |
| `prod` | live | real |

## Gate — all must pass, stop on first failure

1. `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings`
2. `pnpm --filter web lint`, `pnpm --filter web test`
3. Migrations reviewed: forward-only, additive, indexes on new FKs and filter columns
4. Eval harness within tolerance (faithfulness, context precision)
5. Non-negotiable evidence:
   - `wb_violation == 0` in the e2e run
   - red-team corpus: 100 % termination, 0 prompt leaks
   - out-of-syllabus corpus: 100 % abstention
6. Secrets resolve from the manager in the target environment
7. Cost dashboard within budget

For prod: confirm a backup within the last 24 h, then ask for explicit human confirmation.

## Order of operations

Migrations first (they are additive, so they are safe ahead of the code), then gateway, then
frontend. Gateway nodes drain WebSockets gracefully — never hard-kill a node with live sessions;
`SIGTERM` starts a drain that finishes in-flight turns and sends `session_end` with reason
`maintenance`.

## Post-deploy verification

Health endpoint, one synthetic classroom session end to end, then five minutes of error rate,
p95 latency, and `wb_violation` metrics. Roll back on any non-zero `wb_violation` or a latency
regression beyond the budget in `.claude/rules/realtime-audio.md`.

Report the deployed SHA and the verification results.
