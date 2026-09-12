# Digi Guru — Team Instructions

Real-time AI e-learning platform: live Gemini voice tutoring over a strict, RAG-bounded syllabus,
synchronised with a dynamic whiteboard.

Read `IMPLEMENTATION_PLAN.md` before starting any task. It is the source of truth for
architecture, schema, and phase scope.

## Stack

| Layer | Tech |
|-------|------|
| Frontend | Next.js 15 (App Router, TypeScript, React 19) |
| Backend | Rust — axum 0.8 + tokio |
| Vector DB | Qdrant |
| Primary DB | PostgreSQL 16 (sqlx, compile-time checked queries) |
| Cache | Redis 7 |
| Protocol | WebSockets (WSS) |
| LLM | Gemini 3.1 Live (bidirectional audio + tool calls) |

## The five non-negotiables

Never weaken these. If a change makes one harder to guarantee, stop and raise it.

1. **NN-1 Whiteboard-first.** Audio for turn *N* is never forwarded until the client ACKs the
   board ops for turn *N* (or the 400 ms hold ceiling expires). Enforcement lives in the gateway
   `SyncGate`, not in the prompt.
2. **NN-2 First-login logic.** The greeting plays only when `students.is_first_login = true`.
3. **NN-3 Hard 20-minute cap.** Server-authoritative, Redis-backed, counted only while voice is
   active. Client clocks are never trusted.
4. **NN-4 RAG-only.** Below the similarity floor the tutor abstains. No world knowledge, ever.
5. **NN-5 Jailbreak termination.** Tier-0 guard mutes the upstream mid-utterance; Tier-1 tears the
   socket down. Incidents are always persisted.

## Repository layout

```
apps/web        Next.js frontend
apps/gateway    Rust binary: REST + WebSocket
crates/         core, db, rag, live, guardrails, quota
workers/ingest  PDF ingestion worker
migrations/     sqlx SQL migrations
evals/          golden question sets + RAGAs harness
infra/          docker-compose, k8s, terraform
```

## Modular rules

Detailed conventions live in `.claude/rules/` — read the one that matches what you are touching:

- `code-style.md` — Rust and TypeScript conventions
- `testing.md` — what must be tested and how
- `api-conventions.md` — REST + WebSocket message contracts
- `rag-pipeline.md` — chunking, metadata, retrieval, cost rules
- `realtime-audio.md` — WebSocket, VAD, buffering, latency budget
- `whiteboard-sync.md` — the board protocol and NN-1 enforcement
- `security.md` — RBAC, secrets, PII, guardrails
- `pedagogy.md` — turn-taking, backchanneling, adaptive difficulty, tone switching, note export

## Git workflow (2-developer team)

Run this before touching prompt engines, `.claude/` rules, or backend logic — and before any push:

1. `git pull origin main --rebase` — always start from latest origin.
2. Diff local `.claude/` and `CLAUDE.md` against the pulled version before editing further — both
   developers must be working from the same rule set.
3. Run the relevant test suite locally (see Commands below) before committing.
4. `git commit -m "feat: <detailed description>"` — no vague messages.
5. `git push origin <branch-name>`.

Never edit prompt instructions or `.claude/rules/*` locally without pushing — a divergent local
rule set is worse than no rule set, since it silently changes agent behavior for only one developer.

## Working agreements

- Do not add a dependency without noting why in the PR description.
- Every new endpoint declares its required capability; there is no implicit authorization.
- Every Qdrant query carries `program_id`, `semester`, and `block_no` filters. No exceptions.
- No `unwrap()` or `expect()` in the audio or request path — propagate typed errors.
- Errors returned to a client are `PublicError` only. Never leak SQL, stack traces, or prompts.
- Migrations are forward-only and reviewed; never edit an applied migration.
- Log student identifiers, never student PII.

## Commands

```bash
cargo test --workspace          # Rust tests
cargo clippy --all-targets -- -D warnings
pnpm --filter web test          # frontend tests
pnpm --filter web lint
docker compose -f infra/docker-compose.yml up -d   # local deps
sqlx migrate run                # apply migrations
cargo run -p evals              # RAGAs golden-set harness
```
