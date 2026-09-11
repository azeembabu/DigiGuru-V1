# Digi Guru

A real-time, AI-powered e-learning platform. Students join a live voice classroom where an AI tutor
teaches strictly from their own syllabus, writing and drawing on a dynamic whiteboard as it speaks.

> **Status:** planning / Phase 0. This repository currently holds the implementation plan and the
> agent control center. Application code lands in Phase 1.

## What it does

- **Live voice tutoring** — two-way audio with Gemini 3.1 Live over WebSockets.
- **Whiteboard-first teaching** — every explanation is rendered on the canvas *before* it is spoken.
- **Strictly bounded curriculum** — answers come only from ingested textbook content; anything else
  is refused, never improvised.
- **Multi-role platform** — Super Admin, Sub-Admins (content + student management), Students.
- **Context persistence** — login restores the student's program, semester, and current block.
- **Governed sessions** — 20-minute voice cap, recaps, comprehension checks, anti-jailbreak guards.

## Stack

| Layer | Technology |
|-------|------------|
| Frontend | Next.js 15 |
| Backend | Rust (axum + tokio) |
| Vector DB | Qdrant |
| Primary DB | PostgreSQL 16 |
| Cache | Redis 7 |
| Protocol | WebSockets |
| LLM | Gemini 3.1 Live |

## Documentation

- **[IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md)** — architecture, data model, four-phase
  roadmap, and the acceptance criteria for each phase. Start here.
- **[CLAUDE.md](CLAUDE.md)** — team working agreements and the five non-negotiables.
- **[.claude/rules/](.claude/rules/)** — per-area conventions: RAG, real-time audio, whiteboard
  sync, security, testing, API contracts.

## The five non-negotiables

1. Audio never precedes its whiteboard visual.
2. The welcome greeting plays only on a student's first login.
3. The 20-minute voice cap is server-authoritative and absolute.
4. Outside the retrieved textbook context, the tutor refuses rather than guesses.
5. A detected jailbreak attempt terminates the session immediately.

## Roadmap

| Phase | Focus |
|-------|-------|
| 1 | Database, RBAC, registration, context persistence |
| 2 | PDF ingestion, vector search, cost-optimised retrieval |
| 3 | Gemini Live audio and whiteboard synchronisation |
| 4 | Session control, safety enforcement, analytics |
