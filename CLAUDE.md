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
3. **NN-3 Daily 20-minute voice quota.** Twenty minutes of *active voice* per student per
   calendar day, server-authoritative and Redis-backed, counted only while voice is active.
   The day boundary is midnight in the **student's own timezone** (`students.timezone`), not
   UTC and not the server's — a student in a different zone must not lose minutes to a reset
   that happens mid-afternoon for them. Client clocks are never trusted for either the elapsed
   time or the date.

   *Changed 2026-09-13 from a per-session hard cap to a daily resetting quota, on the product
   owner's instruction. The enforcement point is unchanged: the ledger is server-side, and a
   session still ends with `end_reason = 'quota'` when the remaining allowance reaches zero.*
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

## The `master` branch

`master` holds a Node/Express + Prisma + Vite React prototype of Phase 1. It is a **frozen
reference, not a codebase to extend** — read it for requirements, validated field rules, and admin
UX, then implement on this stack. See `IMPLEMENTATION_PLAN.md` §13 for what to mine and what it is
missing. Never merge `master` into `main`.

The academic hierarchy is **Program > Semester > Course > Block**. LSC is an entity with its own
table, not a text field on the student. A student reaches content only through `student_courses`.

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

## MANDATORY: sync CLAUDE.md / `.claude/` on every prompt or rule change

This is not optional. Any change to the agent's instructions — `CLAUDE.md`, anything under
`.claude/rules/`, `.claude/agents/`, `.claude/commands/`, `.claude/skills/`, or `.claude/settings.json`
— must go through this exact sequence before the change is considered done:

1. `git pull origin main --rebase` first, so the edit is applied on top of the other developer's
   latest rules, not a stale copy — this is what prevents the two machines' rule sets from
   silently diverging and causing conflicting agent behavior.
2. Make the edit.
3. Commit with a message that names which rule/file changed and why, e.g.
   `git commit -m "docs(claude): tighten RBAC capability rule after sub-admin scope bug"`.
4. `git push origin main` (or the branch, if the change rides with a feature branch) **immediately**
   — an unpushed prompt change is treated as not having happened. Never leave a `.claude/`/`CLAUDE.md`
   edit uncommitted or unpushed at the end of a session.
5. Tell the other developer (or leave it visible in the commit) that the rule set changed, so their
   next `git pull --rebase` picks it up before their next task.

A rule change that lives on only one computer is the single fastest way for the two of you to get
conflicting agent behavior — treat every `.claude/`/`CLAUDE.md` edit as incomplete until it is pushed.

## Machine ownership (2-developer split)

**This machine owns the whole repo, frontend included** (changed 2026-09-13 by the backend
developer, deliberately, so one machine can ship a vertical slice end to end without waiting on
a contract hand-off). The split below is therefore currently *inactive*. Restoring it is a
two-person decision: edit this section, push, and both developers switch together.

| Machine | Owns | Must not touch |
|---|---|---|
| **This machine (backend + frontend)** | everything: `apps/gateway/`, `apps/web/`, `crates/*`, `migrations/`, `infra/` | — |
| **Other machine (frontend)** | coordinate before editing `apps/web/` — pull first, and say what you are taking | — |

Because both halves now land from one machine, the coordination cost moves to the merge:

- `.claude/rules/api-conventions.md` is still the contract of record. Update it in the **same
  commit** as the change it describes — it is now documentation of what shipped rather than a
  hand-off artifact, and a stale contract is worse than none.
- The other developer must `git pull origin main --rebase` before touching `apps/web/`, since
  frontend commits can now arrive from either machine.
- If a `.claude/settings.local.json` `permissions.deny` guard on `apps/web/` was added on this
  machine under the old split, remove it — it will block legitimate edits now.

## Working agreements

- Do not add a dependency without noting why in the PR description.
- Every new endpoint declares its required capability; there is no implicit authorization.
- Every Qdrant query carries `program_id`, `semester_no`, `course_id`, and `block_no` filters.
  No exceptions.
- No `unwrap()` or `expect()` in the audio or request path — propagate typed errors.
- Errors returned to a client are `PublicError` only. Never leak SQL, stack traces, or prompts.
- Migrations are forward-only and reviewed; never edit an applied migration.
- Log student identifiers, never student PII.

## Commands

```bash
# Run everything in one go: local deps (docker compose --wait), migrations,
# gateway on :8080 and the web app on :3000, streaming into one window.
# Ctrl-C stops both processes; the containers are left up deliberately.
./dev.ps1
./dev.ps1 -WebOnly      # frontend only - no Docker, no Rust toolchain needed
./dev.ps1 -NoDeps       # deps already running
# Prerequisite for anything but -WebOnly: Docker Desktop installed and its engine
# reachable. dev.ps1 preflights both and stops with the fix rather than letting the
# gateway panic later on a Postgres connect.

cargo test --workspace          # Rust tests
cargo clippy --all-targets -- -D warnings
pnpm --filter web test          # frontend tests
pnpm --filter web lint
docker compose -f infra/docker-compose.yml up -d   # local deps
sqlx migrate run                # apply migrations

# First login: create the bootstrap super-admin. Nothing in migrations/ or
# migrations/seed/ inserts an admin — a committed credential is a published
# credential (security.md) — so the admin console cannot be signed into until
# this is run once. Prompts for the password (twice, not echoed); never takes
# it as an argument, so it cannot land in shell history.
cargo run -p gateway --bin create_admin -- \
  --email you@example.com --full-name "Your Name" --role super_admin

# A scoped sub-admin. --scope is repeatable and takes a program UUID; it
# refuses to create a sub-admin with no scopes (which would see nothing).
cargo run -p gateway --bin create_admin -- \
  --email sub@example.com --full-name "Sub Admin" --role sub_admin --scope <PROGRAM_UUID>

# Non-interactive (CI, containers): supply the password out of band, never
# as an argument.
ADMIN_PASSWORD=... cargo run -p gateway --bin create_admin -- ...   # or --password-stdin

cargo run -p evals              # RAGAs golden-set harness
```
