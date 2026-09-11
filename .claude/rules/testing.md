# Testing

## What must always be tested

| Area | Required test |
|------|---------------|
| RBAC | Full role × endpoint matrix; every forbidden combination asserts 403 |
| NN-1 whiteboard-first | `SyncGate` unit tests: audio never released before ACK or hold expiry |
| NN-2 first login | Two consecutive logins emit the greeting exactly once |
| NN-3 quota | Cap fires at 1200 s of active voice with a skewed client clock |
| NN-4 RAG-only | Out-of-syllabus golden set returns abstention 100 % of the time |
| NN-5 guardrails | Red-team corpus (en + ml) terminates the session every time |
| Metadata isolation | 1 000 randomised queries never retrieve another program or semester |

## Layers

- **Unit** — pure logic: chunker, `SyncGate`, quota ledger, intent matcher, guardrail patterns.
- **Integration** — `sqlx::test` against a real ephemeral Postgres; Qdrant and Redis via testcontainers.
  No mocking of the database.
- **Contract** — WebSocket message schemas validated against the JSON Schema in
  `.claude/rules/api-conventions.md`, from both sides.
- **E2E** — Playwright drives a real classroom session against a mock Live server that replays
  recorded turns; asserts `wb_violation == 0`.
- **Load** — k6 plus a synthetic audio client; 200 concurrent sockets, 30-minute soak.
- **Eval** — RAGAs golden sets. Runs on every change to a system prompt, chunker, or retrieval path.
  A drop in faithfulness or context precision beyond tolerance blocks the merge.

## Conventions

- Test names state the behaviour: `releases_audio_only_after_board_ack`, not `test_sync_gate_2`.
- No sleeps for synchronisation — use channels, notifications, or a controllable clock.
- Time-dependent logic takes an injected clock so quota and timeout tests run instantly.
- A bug fix lands with a test that fails before the fix.
