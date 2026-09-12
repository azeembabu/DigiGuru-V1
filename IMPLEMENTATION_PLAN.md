# Digi Guru — Implementation Plan

**Product:** Digi Guru — a real-time, AI-powered e-learning platform delivering live voice lessons
from a strict, RAG-bounded curriculum, synchronised with a dynamic interactive whiteboard.

**Status:** Planning / Phase 0
**Last updated:** 2026-09-12

---

## 0. Executive Summary

Digi Guru replaces a human tutor for a fixed syllabus. A student logs in, the system restores their
academic context (e.g. *BA Malayalam, Semester 1, Block 3*), and an AI tutor teaches a live,
two-way voice lesson. Every spoken explanation is preceded by a whiteboard render. The AI may only
speak from ingested textbook content. Sessions are capped at 20 minutes of active voice.

Four sequential phases, each independently shippable:

```
[ Phase 1: Database & RBAC Setup ]
                 |
                 v
[ Phase 2: Knowledge Ingestion & RAG Optimization ]
                 |
                 v
[ Phase 3: Gemini Live & Real-Time Sync Engine ]
                 |
                 v
[ Phase 4: Session Control, Security & Analytics ]
```

| Phase | Theme | Est. duration | Exit gate |
|-------|-------|---------------|-----------|
| 1 | Multi-role architecture, onboarding, context persistence | 3 weeks | A student can sign up, log in, and have their program/semester/block auto-restored |
| 2 | PDF ingestion, vector store, cost-optimised retrieval | 3 weeks | A sub-admin uploads a PDF; a text-only query returns correctly cited, metadata-filtered chunks |
| 3 | Gemini Live audio + whiteboard-first sync | 4 weeks | A full voice lesson runs end-to-end with zero whiteboard-after-audio violations |
| 4 | Quotas, guardrails, recaps, observability | 3 weeks | Hard 20-min cap, jailbreak termination, and recap engine verified under load |

---

## 1. Non-Negotiables (enforcement contract)

These five rules are architectural invariants. Every PR touching the affected layer must prove them.

| # | Rule | Enforced where | Verified by |
|---|------|----------------|-------------|
| NN-1 | **Whiteboard-first rendering.** Audio must never precede its visual. | Gateway `SyncGate` holds audio frames until the client ACKs the matching board op. | `sync_gate` unit tests + runtime `wb_violation` counter, must be 0 in CI e2e. |
| NN-2 | **First-time vs returning logic.** Greeting plays only when `is_first_login = true`. | `students.is_first_login` (Postgres), flipped in the same transaction that opens the session. | Integration test: two consecutive logins, assert greeting emitted exactly once. |
| NN-3 | **Hard 20-minute ceiling.** Server-side, cumulative active-voice time. | Redis `quota:{student_id}:{yyyymmdd}` ticked by the gateway; middleware force-closes at 1200s. | Load test with a client whose clock is skewed +2h; cap must still fire at 1200s. |
| NN-4 | **RAG-only fallback.** Below the similarity floor, the AI refuses. | Retriever returns `Abstain`; the prompt builder emits a no-context system directive. | Golden-set eval: out-of-syllabus questions must produce the abstention phrase, 100%. |
| NN-5 | **Immediate jailbreak termination.** Guardrail sits between the student audio stream and the LLM input buffer. | `guardrails` crate, two-tier (see §7.3). Terminates the socket, writes an incident row. | Red-team prompt corpus; every hit must close the socket within 300 ms of transcript. |

> **Engineering note on NN-5.** Gemini Live is speech-to-speech: raw audio reaches the model without
> an intermediate text step, so a literal "filter before the LLM sees it" is not achievable with the
> Live API alone. We implement the strongest available equivalent — a two-tier guard (§7.3): a local
> fast ASR tap that can mute the upstream mid-utterance, plus a hard teardown on the Live API input
> transcription events. This is an explicit, documented assumption, not a silent compromise.

---

## 2. Architecture

```
                   +------------------------------------------+
                   |  Next.js 15 (App Router) - apps/web       |
                   |  - Auth pages, admin console              |
                   |  - Classroom: Canvas + AudioWorklet       |
                   +---------------------+--------------------+
                                         | WSS (binary audio + JSON control)
                                         | HTTPS (REST: auth, admin, uploads)
                   +---------------------v--------------------+
                   |  Rust Gateway (axum + tokio)             |
                   |  AuthN/Z | QuotaMiddleware               |
                   |  SyncGate | Guardrails | SessionFSM      |
                   +--+--------+---------+---------+----------+
                      |        |         |         |
          +-----------v--+ +---v-----+ +-v------+ +v-----------------+
          | PostgreSQL16 | | Redis 7 | | Qdrant | | Gemini 3.1 Live  |
          | users, roles | | session | | vector | | bidi audio +     |
          | sessions,logs| | quota   | | + meta | | tool calls       |
          +------^-------+ +---------+ +--------+ +------------------+
                 |
          +------+---------------------+
          | Ingestion Worker (Rust)    |
          | PDF > parse > chunk >      |
          | embed > tag > upsert       |
          +----------------------------+
```

### 2.1 Technology choices

| Layer | Choice | Rationale |
|-------|--------|-----------|
| Frontend | **Next.js 15** (App Router, TypeScript) | SSR for auth-gated pages, React 19 for the classroom client. |
| Backend | **Rust** (axum 0.8 + tokio) | Per-socket async tasks, no GC pauses in the audio path, predictable p99 latency. |
| Vector DB | **Qdrant** | Native payload filtering (program/semester/block), hybrid sparse+dense in one query. |
| Primary DB | **PostgreSQL 16** | Relational RBAC, session ledger, audit trail. `sqlx` with compile-time checked queries. |
| Cache | **Redis 7** | Session state (stateless gateway failover), quota counters, prompt-cache handles. |
| Protocol | **WebSockets** (WSS) | Bidirectional binary audio + JSON control on one multiplexed connection. |
| Embeddings | `gemini-embedding-001` (dense) + BM25 (sparse, Qdrant-native) | Hybrid retrieval preserves exact Malayalam textbook terms. |
| Reranker | Cross-encoder (bge-reranker-v2-m3, self-hosted ONNX) | Cuts context to top-2/3 — the single largest token-cost lever. |

### 2.2 Repository layout

```
Project-Vertion01/
├── CLAUDE.md                  # team instructions (committed)
├── CLAUDE.local.md            # personal overrides (gitignored)
├── IMPLEMENTATION_PLAN.md     # this file
├── .claude/                   # control center - see §12
├── apps/
│   ├── web/                   # Next.js
│   └── gateway/               # Rust binary: REST + WS
├── crates/
│   ├── core/                  # domain types, errors, config
│   ├── db/                    # sqlx models + migrations runner
│   ├── rag/                   # ingest, chunk, embed, retrieve, rerank
│   ├── live/                  # Gemini Live client, SyncGate, board protocol
│   ├── guardrails/            # jailbreak, toxicity, curriculum boundary
│   └── quota/                 # session clock, Redis ledger
├── workers/
│   └── ingest/                # PDF ingestion worker binary
├── migrations/                # *.sql, sqlx-managed
├── evals/                     # golden question sets, RAGAs harness
├── infra/                     # docker-compose, k8s, terraform
└── docs/                      # ADRs, runbooks
```

---

## 3. Data Model

### 3.1 PostgreSQL (core tables)

```sql
-- Academic hierarchy: Program > Semester > Course > Block
-- (carried over from the validated prototype model on origin/master; see §13)

CREATE TYPE user_role     AS ENUM ('super_admin', 'sub_admin', 'student');
CREATE TYPE user_status   AS ENUM ('active', 'inactive', 'suspended');
CREATE TYPE entity_status AS ENUM ('active', 'inactive');

CREATE TABLE users (
  id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  role           user_role   NOT NULL,
  status         user_status NOT NULL DEFAULT 'active',
  email          CITEXT      NOT NULL UNIQUE,
  password_hash  TEXT        NOT NULL,          -- argon2id
  last_login_at  TIMESTAMPTZ,
  created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE admins (                            -- super_admin and sub_admin profile
  id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id   UUID NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
  full_name TEXT NOT NULL
);

CREATE TABLE lscs (                              -- Learner Support Centre (an entity, not a string)
  id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  code     TEXT NOT NULL UNIQUE,
  name     TEXT NOT NULL,
  location TEXT,
  status   entity_status NOT NULL DEFAULT 'active'
);

CREATE TABLE programs (                          -- e.g. "BA Malayalam"
  id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  code        TEXT NOT NULL UNIQUE,
  name        TEXT NOT NULL,
  description TEXT,
  status      entity_status NOT NULL DEFAULT 'active'
);

CREATE TABLE semesters (
  id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  program_id      UUID NOT NULL REFERENCES programs(id) ON DELETE CASCADE,
  semester_number SMALLINT NOT NULL CHECK (semester_number BETWEEN 1 AND 12),
  name            TEXT NOT NULL,
  status          entity_status NOT NULL DEFAULT 'active',
  UNIQUE (program_id, semester_number)
);

CREATE TABLE courses (
  id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  program_id  UUID NOT NULL REFERENCES programs(id)  ON DELETE CASCADE,
  semester_id UUID NOT NULL REFERENCES semesters(id) ON DELETE CASCADE,
  code        TEXT NOT NULL,
  name        TEXT NOT NULL,
  description TEXT,
  UNIQUE (program_id, semester_id, code)
);

CREATE TABLE blocks (                            -- a teaching unit inside a course
  id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  course_id UUID NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
  block_no  SMALLINT NOT NULL,
  title     TEXT NOT NULL,
  UNIQUE (course_id, block_no)
);

CREATE TABLE students (
  id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id          UUID NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
  full_name        TEXT NOT NULL,
  roll_number      TEXT NOT NULL UNIQUE,
  phone_number     TEXT NOT NULL,
  program_id       UUID NOT NULL REFERENCES programs(id)  ON DELETE RESTRICT,
  semester_id      UUID NOT NULL REFERENCES semesters(id) ON DELETE RESTRICT,
  lsc_id           UUID NOT NULL REFERENCES lscs(id)      ON DELETE RESTRICT,
  current_block_id UUID REFERENCES blocks(id),              -- context persistence target
  is_first_login   BOOLEAN NOT NULL DEFAULT TRUE,           -- NN-2
  locale           TEXT NOT NULL DEFAULT 'ml-IN',
  timezone         TEXT NOT NULL DEFAULT 'Asia/Kolkata',
  created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TYPE enrollment_status AS ENUM ('active', 'completed', 'dropped');

CREATE TABLE student_courses (                   -- which courses a student may actually study
  id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  student_id  UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
  course_id   UUID NOT NULL REFERENCES courses(id)  ON DELETE CASCADE,
  status      enrollment_status NOT NULL DEFAULT 'active',
  assigned_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (student_id, course_id)
);

CREATE TABLE sub_admin_scopes (                  -- which programs a sub-admin may touch
  user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  program_id UUID NOT NULL REFERENCES programs(id),
  PRIMARY KEY (user_id, program_id)
);

CREATE TABLE documents (
  id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  block_id       UUID NOT NULL REFERENCES blocks(id),
  uploaded_by    UUID NOT NULL REFERENCES users(id),
  title          TEXT NOT NULL,
  storage_key    TEXT NOT NULL,
  sha256         TEXT NOT NULL UNIQUE,           -- dedupe re-uploads
  page_count     INT  NOT NULL,
  ocr_confidence REAL,                           -- NULL = born-digital, no OCR needed
  status         TEXT NOT NULL DEFAULT 'pending',
                 -- pending|parsing|pending_review|embedded|failed
  created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TYPE session_status AS ENUM ('in_progress', 'completed', 'abandoned');

-- The classroom session. Named `learning_sessions` to keep it distinct from
-- `auth_sessions` (refresh tokens) — the prototype's naming, and worth keeping.
CREATE TABLE learning_sessions (
  id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  student_id      UUID NOT NULL REFERENCES students(id),
  course_id       UUID NOT NULL REFERENCES courses(id),
  block_id        UUID NOT NULL REFERENCES blocks(id),
  started_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  ended_at        TIMESTAMPTZ,
  active_voice_ms BIGINT NOT NULL DEFAULT 0,     -- NN-3, server-authoritative
  status          session_status NOT NULL DEFAULT 'in_progress',
  end_reason      TEXT,                          -- quota|idle|user|jailbreak|error
  resume_summary  TEXT,                          -- feeds the next session recap
  last_topic      TEXT,
  last_page       INT
);

CREATE TABLE auth_sessions (                     -- refresh-token / device sessions
  id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id            UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  refresh_token_hash TEXT NOT NULL UNIQUE,
  device_info        TEXT,
  ip_address         INET,
  created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at         TIMESTAMPTZ NOT NULL,
  revoked_at         TIMESTAMPTZ
);

CREATE TABLE password_resets (
  id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  token_hash TEXT NOT NULL UNIQUE,
  expires_at TIMESTAMPTZ NOT NULL,
  used_at    TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE audit_logs (
  id          BIGSERIAL PRIMARY KEY,
  user_id     UUID REFERENCES users(id) ON DELETE SET NULL,
  action      TEXT NOT NULL,
  ip_address  INET,
  device_info TEXT,
  metadata    JSONB,                             -- must be PII-free (H-43)
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE safety_incidents (
  id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  session_id  UUID REFERENCES learning_sessions(id),
  student_id  UUID NOT NULL REFERENCES students(id),
  kind        TEXT NOT NULL,                     -- jailbreak|toxicity|out_of_scope
  tier        SMALLINT NOT NULL,                 -- 0 = pre-LLM tap, 1 = transcript, 2 = output
  excerpt     TEXT NOT NULL,                     -- PII-redacted
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE board_events (                      -- whiteboard audit / replay
  id         BIGSERIAL PRIMARY KEY,
  session_id UUID NOT NULL REFERENCES learning_sessions(id),
  turn_seq   INT  NOT NULL,
  op         JSONB NOT NULL,
  emitted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  acked_ms   INT                                 -- client ACK latency; NULL = never acked
);
```

**Indexes.** Every foreign key above is indexed, plus the filter columns used on hot paths:
`users(email, role, status)`, `students(roll_number, program_id, semester_id, lsc_id, phone_number)`,
`student_courses(student_id, course_id, status)`, `learning_sessions(student_id, started_at)`,
`auth_sessions(refresh_token_hash, expires_at, revoked_at)`, `audit_logs(user_id, action, created_at)`.

### 3.2 Qdrant collection — `curriculum`

```jsonc
{
  "vectors": { "dense": { "size": 3072, "distance": "Cosine" } },
  "sparse_vectors": { "bm25": {} },
  "payload_schema": {
    "program_id":  "keyword",   // filter: student program        (indexed)
    "semester_no": "integer",   // filter: student semester       (indexed)
    "course_id":   "keyword",   // filter: enrolled course        (indexed)
    "block_no":    "integer",   // filter: current block          (indexed)
    "document_id": "keyword",
    "chapter":     "keyword",
    "topic":       "text",
    "page":        "integer",
    "para_index":  "integer",
    "text":        "text",
    "lang":        "keyword"    // ml | en
  }
}
```

Payload indexes on `program_id`, `semester_no`, `course_id`, and `block_no` are **mandatory** —
they are the mechanism that prevents cross-course context leakage (checklist item E-25). The
`course_id` filter is what makes "strictly this student's syllabus" enforceable: a student who is
enrolled in three courses this semester must not retrieve from a fourth they are not enrolled in,
which a program+semester filter alone would allow.

### 3.3 Redis key map

| Key | Type | TTL | Purpose |
|-----|------|-----|---------|
| `sess:{session_id}` | Hash | 6 h | Session FSM state — enables gateway failover (A-8) |
| `quota:{student_id}:{yyyymmdd}` | String (ms) | until local midnight | NN-3 cumulative voice ledger |
| `ctx:{student_id}` | Hash | 30 d | Persisted program/semester/block ("Remember Me") |
| `pcache:{block_id}` | String | 1 h | Gemini prompt-cache handle for the block static preamble |
| `rl:{ip}:{route}` | String | 60 s | Rate limiting |
| `idle:{session_id}` | String | 120 s | Idle-timeout watchdog (D-22) |

---

## 4. Phase 1 — Multi-Role User Architecture & Onboarding

> **Why first:** every other feature reads from the identity and context tables. Nothing downstream
> can be tested without a real student row carrying program/semester/block.

### 4.1 Scope

1. **Schema + migrations.** All tables in §3.1, `sqlx migrate`, seed script for one program
   (BA Malayalam, semesters 1–6, courses per semester, blocks 1–4 per course). The reference
   prototype's two Prisma migrations (§13) are the starting point for the seed data.
2. **RBAC.** Three roles — and note this is where the prototype diverges: it has only
   `STUDENT | ADMIN`, so the super-admin / sub-admin split is net-new work, not a port.
   Authorization is a middleware extractor in axum returning a typed `Actor` enum; every handler
   declares its required capability. Sub-admins are additionally scoped by `sub_admin_scopes` —
   a sub-admin for BA Malayalam cannot touch BCom content.
3. **Student registration pipeline.** Captures Name, Roll Number, Program, Semester, LSC,
   Phone, Email. Server-side validation via `garde`/`validator` schemas (C-14):
   - roll number: regex per program, unique
   - phone: E.164 after normalising the Indian 10-digit form
   - email: RFC-validated + verification link
   - program / semester / LSC: must resolve to existing rows, not free text
   - **allow-list on update.** Student self-service PATCH accepts only `full_name` and
     `phone_number`; any other key is a validation error, never silently ignored. Academic fields
     (`program_id`, `semester_id`, `lsc_id`) are admin-only. This pattern is proven in the
     prototype's `student.validator.ts` and should be carried over verbatim in spirit.
4. **Auth.** Argon2id password hashing, short-lived access JWT (15 min) + rotating refresh token
   stored hashed in Postgres, httpOnly SameSite=Strict cookies (C-16).
5. **Context persistence ("Remember Me").** On successful login the gateway hydrates `ctx:{id}`
   from Postgres and returns it in the session-init payload, so the classroom opens directly on
   *BA Malayalam / Semester 1 / Block 3* with no picker (C-15).
6. **Profile-update guard.** Changing `program_id`, `semester`, or `current_block_id` while a
   session is live returns `409 SESSION_ACTIVE` and forces a session re-init (C-17).
7. **Admin console (Next.js).** Super Admin: user list, role assignment, program CRUD, platform
   metrics stub. Sub-Admin: scoped student list + upload page shell (wired in Phase 2).

### 4.2 Deliverables

- `migrations/0001_init.sql` … `0006_indexes.sql`
- `crates/db` models, `crates/core::auth` with `Actor`, `Capability`
- `apps/gateway` REST: `/auth/*` (login, signup, refresh, forgot/reset password, session list and
  revoke), `/admin/users`, `/admin/programs`, `/admin/semesters`, `/admin/courses`, `/admin/lscs`,
  `/admin/students`, `/me/context`, `/me/profile`
- `apps/web`: signup, login, forgot/reset password, role-routed dashboards, admin CRUD for
  programs / semesters / courses / LSCs / students
- Audit logging on every admin mutation and every auth event (metadata must be PII-free)
- Integration test suite: role matrix (3 roles × every endpoint; each forbidden combination asserts 403)

### 4.3 Acceptance criteria

- [ ] A student signs up, verifies email, logs in, and lands in the classroom with context preloaded.
- [ ] A sub-admin cannot read or write any program outside their scope (403, asserted in tests).
- [ ] A student cannot reach any `/admin/*` route.
- [ ] `is_first_login` is `TRUE` on the first session and `FALSE` on the second (NN-2 foundation).
- [ ] All registration fields reject malformed input server-side, with field-level error messages.

**Checklist coverage:** C-13, C-14, C-15, C-16, C-17, H-44 (rate limiting on `/auth/*`), H-46.

---

## 5. Phase 2 — RAG Pipeline & Content Ingestion

> **Why second:** the tutor cannot speak before there is a corpus. This phase is also where the
> long-run token bill is decided.

### 5.1 Ingestion workflow

```
Sub-Admin upload (PDF, block-scoped)
   -> virus scan + sha256 dedupe -> object storage
   -> job enqueued (Postgres outbox -> worker)
   -> LAYOUT PARSE      (pdfium text + layout boxes; OCR fallback for scanned Malayalam)
   -> CLEAN             (strip headers/footers/page numbers/watermarks)   [E-29]
   -> PARAGRAPH CHUNK   (paragraph-level; never split a sentence or table) [E-23]
   -> ENRICH            (chapter, topic, page, para_index, lang)
   -> EMBED             (dense: gemini-embedding-001; sparse: BM25)
   -> UPSERT to Qdrant with full metadata payload                          [E-24]
   -> documents.status = 'embedded'; emit ingest report to sub-admin
```

**Chunking rules.** Target 250–450 tokens per chunk with a one-sentence overlap. A paragraph
shorter than 80 tokens merges forward. Tables and verse/poetry blocks are kept atomic and tagged
`kind: table|verse` so the whiteboard can render them structurally rather than as prose.

### 5.2 Retrieval pipeline (per student question)

```
question
  -> normalise + language detect (ml/en)
  -> HYBRID SEARCH in Qdrant: dense + BM25, RRF fusion, top_k = 20      [E-26]
     with hard filter: program_id AND semester_no AND course_id AND block_no  [E-25]
  -> CROSS-ENCODER RERANK -> top 2..3                                   [E-27]
  -> SIMILARITY FLOOR CHECK
       if best_rerank_score < ABSTAIN_THRESHOLD -> return Abstain       [NN-4 / E-30]
  -> build prompt: [cached static preamble] + [2-3 chunks] + [turn state]
```

### 5.3 Cost controls

| Lever | Mechanism | Expected saving |
|-------|-----------|-----------------|
| Prompt caching (E-28) | Tutor system instruction + block syllabus outline are cached per block; handle in `pcache:{block_id}` | up to ~80 % on the static prefix |
| Top-2/3 reranking (E-27) | Never send 20 chunks to the model | ~70 % of retrieval tokens |
| Context trimming (E-29) | Headers/footers/page furniture stripped at ingest | ~10–15 % of chunk tokens |
| Polite-phrase shortcut (F-36) | "Thank you" / "നന്ദി" answered by a local intent matcher, no retrieval, no LLM round trip | removes a whole turn |
| Abstention (E-30) | No generation when there is no context | removes wasted turns |

### 5.4 Deliverables

- `crates/rag`: `ingest`, `chunk`, `embed`, `retrieve`, `rerank`, `abstain`
- `workers/ingest` binary + retry/DLQ
- Sub-admin upload UI with live ingest progress and a per-document chunk preview
- `evals/golden/` — 150 Q&A pairs per block: in-syllabus, edge, and out-of-syllabus
- RAGAs harness: context precision, context recall, faithfulness, answer relevancy

### 5.5 Acceptance criteria

- [ ] A 300-page Malayalam PDF ingests with zero mid-sentence chunk boundaries (spot-check 50 chunks).
- [ ] A Semester-1 student query never retrieves a Semester-2 chunk (assert over 1 000 randomised queries).
- [ ] Out-of-syllabus questions return the abstention response in 100 % of golden-set cases.
- [ ] Median retrieval latency (search + rerank) < 300 ms at p50, < 600 ms at p95.
- [ ] Measured prompt-cache hit rate ≥ 90 % for repeat sessions on the same block.

**Checklist coverage:** E-23 … E-30, I-50.

---

## 6. Phase 3 — Gemini Live Audio & Dynamic Whiteboard Sync

> **Why third:** it depends on both identity context and the corpus. This is the phase that makes
> or breaks the product feel.

### 6.1 Transport

- One WSS connection per session, multiplexed: binary frames = audio, text frames = JSON control.
- Client captures mic through an `AudioWorklet` at 16 kHz mono PCM16, 20 ms frames, with a
  jitter buffer and clean frame alignment before upstream (A-2).
- Client-side VAD (`@ricky0123/vad-web` or WebRTC VAD via WASM) with a tuned noise floor and a
  300 ms hangover, so ambient noise does not open a turn (A-3).
- Auto-reconnect with exponential backoff + jitter (250 ms → 8 s, cap 6 attempts); session state is
  restored from `sess:{id}` in Redis so the reconnect resumes rather than restarts (A-1, A-8).
- Codec negotiation: Opus preferred, raw PCM16 fallback for low-end mobile browsers (A-5).
- Explicit teardown on route change / tab close (`beforeunload` + React cleanup) so sockets never
  leak (A-6).

### 6.2 The whiteboard-first protocol (NN-1) — core design

The AI is given a tool, `board_ops`, and the system instruction requires it to call that tool
**before** narrating anything visual. The gateway then enforces it mechanically rather than trusting
the model:

```
Model turn N
  |- tool_call board_ops{ seq: N, ops: [ {write_text|draw|math|highlight|clear} ] }
  |- audio chunks for turn N
         |
         v
   +----------------- SyncGate (gateway) ---------------------+
   |  1. On board_ops: send control frame to client, start     |
   |     timer, mark turn N as PENDING_ACK.                    |
   |  2. Audio frames for turn N are buffered, NOT forwarded.  |
   |  3. On client {type:"board_ack", seq:N}: release buffered  |
   |     audio, mark turn N OPEN.                              |
   |  4. If no ACK within HOLD_MAX (400 ms): release audio      |
   |     anyway, increment wb_violation, log with turn id.     |
   |  5. If the client reports a render exception: release      |
   |     audio, degrade to text-only board, keep teaching.      |
   +-----------------------------------------------------------+
```

Properties this gives us:

- Audio **can never** precede its visual under normal operation — the buffer is the enforcement.
- The 400 ms ceiling means a slow client degrades into "simultaneous" rather than a stalled lesson,
  which still satisfies the stated rule ("before or simultaneously with").
- A canvas crash cannot kill the voice stream (B-10): exceptions are caught in an error boundary
  that reports `board_error` and the lesson continues in text-fallback mode.
- Every op and its ACK latency is persisted to `board_events` for audit and replay.

### 6.3 Board op schema

```jsonc
{
  "type": "board_ops",
  "seq": 42,
  "clear_first": false,
  "ops": [
    { "op": "heading", "text": "അധ്യായം 3 — ഭാഷാചരിത്രം", "page": 57 },
    { "op": "bullets", "items": ["...", "..."] },
    { "op": "math",   "latex": "a^2 + b^2 = c^2" },
    { "op": "draw",   "shape": "arrow", "from": [120, 200], "to": [340, 200] },
    { "op": "image",  "ref": "doc:UUID#p57-fig2" },
    { "op": "highlight", "target": "bullet:1" }
  ]
}
```

### 6.4 Canvas implementation

- Fabric.js on a `<canvas>` with an offscreen buffer; batched `requestAnimationFrame` commits so a
  burst of ops is one paint, holding 60 FPS on low-spec mobile (B-11).
- Malayalam text rendered with a bundled webfont (Noto Sans Malayalam) subset to the script range.
- Board state is reset and archived on block/chapter change and on session start (B-12); the archive
  is a JSON op-log, so any past board is replayable without storing images.

### 6.5 Latency budget (A-4) — target < 1.5 s speech-end → audio-start

| Segment | Budget |
|---------|--------|
| Client VAD end-of-speech detection | 300 ms |
| Upstream network to gateway | 60 ms |
| Guardrail Tier-0 check | 20 ms |
| Retrieval (hybrid + rerank, cache-warm) | 300 ms |
| Gemini Live first audio token | 500 ms |
| SyncGate board ACK hold | ≤ 250 ms typical |
| Downstream + client jitter buffer | 70 ms |
| **Total** | **~1.5 s** |

Each segment is a histogram in the trace (I-47); a regression on any one of them is a CI failure.

### 6.6 Deliverables

- `crates/live`: Live API client, turn FSM, `SyncGate`, board-op validator
- `apps/web/classroom`: audio worklet, VAD, canvas renderer, ACK emitter, reconnect logic
- Load test harness (k6 + a synthetic audio client) for N concurrent sockets (A-7)

### 6.7 Acceptance criteria

- [ ] 30-minute soak with 200 concurrent synthetic sessions: `wb_violation` count = 0.
- [ ] Forced canvas exception mid-lesson: audio continues, lesson completes, incident logged.
- [ ] Network cut for 5 s: client reconnects and resumes the same session, not a new one.
- [ ] p95 speech-end → audio-start < 1.5 s at 200 concurrent sessions.
- [ ] Gateway holds 200 sockets with no tokio task blocking (no `block_in_place` in the audio path).

**Checklist coverage:** A-1 … A-8, B-9 … B-12, I-47.

---

## 7. Phase 4 — Session Management, Safety & Analytics

### 7.1 Session lifecycle FSM

```
INIT -> (is_first_login ? GREETING : RECAP_OR_SKIP) -> TEACHING
  TEACHING <-> COMPREHENSION_CHECK
  TEACHING -> QUOTA_WARNING (at 18:00) -> QUOTA_TEARDOWN (at 20:00) -> CLOSED
  any -> IDLE_TIMEOUT (120 s silence) -> CLOSED
  any -> JAILBREAK_HALT -> CLOSED
```

**Pedagogical flow rules encoded in the FSM and system prompt:**

- **First login (NN-2, F-31):** "Welcome to Digi Guru" + course introduction, then straight into
  Block 1. Subsequent logins skip all of it.
- **Recap (F-32, F-33):** on entry, a 30–60 s recap generated from `sessions.resume_summary` of the
  last session — not re-retrieved from the corpus, so it is cheap and accurate. Voice overrides
  ("skip recap", "start lesson", "തുടങ്ങൂ") are matched by the Tier-0 intent classifier and abort the
  recap stream immediately, without an LLM round trip.
- **Citation (F-34):** every explanation opens with chapter, topic, and page — and those values come
  from the retrieved chunk payload, injected into the turn state, so they cannot be hallucinated.
- **Comprehension gate (F-35):** after each paragraph the tutor asks a verification question and the
  FSM will not advance `para_index` until an affirmative or a corrected answer is received.
- **Polite reciprocity (F-36):** gratitude intents get a short acknowledgement from a local phrase
  table in the first seconds of the turn, then the lesson resumes — no retrieval, no LLM call.

### 7.2 Quota enforcement (NN-3)

- The gateway runs a 1-second ticker per session; a tick increments `quota:{student}:{date}` **only
  while voice is active** (student speaking or AI speaking), so silence is free (D-19).
- The counter is server-authoritative; the client clock is never consulted (D-21).
- At 18:00 elapsed the AI is instructed to begin wrapping up; at 20:00 it delivers the polite quota
  message and the gateway closes the socket 3 s later regardless of model state (D-20).
- Daily reset by a cron job that sweeps per-student timezone boundaries (D-22).
- Idle watchdog closes a socket after 120 s of total silence (D-23).

### 7.3 Guardrails (NN-5)

**Tier 0 — pre-LLM tap (target < 20 ms).** The gateway forks the upstream audio to a small local
ASR (whisper-small ONNX / faster-whisper). Partial transcripts run through:
- a compiled regex/Aho-Corasick set of injection patterns (multilingual: en + ml),
- a small classifier for obfuscated attempts,
- a toxicity model for abuse (G-41).

A Tier-0 hit **mutes the upstream mid-utterance** — no further audio is forwarded to Gemini — and
escalates. This is the layer that literally sits between the student audio and the LLM input buffer.

**Tier 1 — Live input-transcription events.** The Live API emits its own input transcription; a hit
here triggers immediate socket teardown (< 300 ms), an incident row, and a `JAILBREAK_HALT` state.

**Tier 2 — output guard.** Model output is screened before the TTS stream is released; a response
that cites no retrieved chunk, or that leaks the system prompt, is dropped rather than spoken (G-42, G-40).

**Curriculum boundary (G-38, G-42).** The system prompt states: answer only from the provided
context; never use outside/world knowledge; textbook and chapter names may be disclosed; on no
context, say "This is not covered in your textbook" and offer the nearest in-syllabus topic.

**Sanitised errors (G-40).** A single `PublicError` type is the only thing serialised to the client;
stack traces, SQL, and system prompts are logged internally and never rendered.

### 7.4 Security & infrastructure

| Item | Implementation |
|------|----------------|
| Backups (H-42) | Daily `pg_dump` + WAL archiving to off-site object storage, 30-day retention, monthly restore drill |
| PII redaction (H-43) | A log filter redacts phone, email, and name before anything leaves the process; monitoring receives `student_id` only |
| Rate limiting (H-44) | Per-IP and per-account token buckets on `/auth/*`, upload, and WS upgrade |
| Secrets (H-45) | No secrets in env files in production; AWS Secrets Manager / Vault with rotation; CI secret scanning on every push |
| Connection pooling (H-46) | PgBouncer in transaction mode; `sqlx` pool sized to PgBouncer, not to Postgres |
| Transport | TLS 1.3 everywhere, HSTS, strict CSP, WSS only |

### 7.5 Observability (I-47 … I-50)

- **Tracing:** OpenTelemetry spans stitched across `ws.turn → guardrail → rag.retrieve →
  rag.rerank → live.request → board.emit → board.ack → audio.release`, one trace id per turn,
  exported to Tempo/Jaeger.
- **Cost:** per-session token and audio-minute counters; Grafana alerts at 70 / 90 / 100 % of the
  daily budget; a kill switch that degrades to text-only mode at 100 %.
- **Audit:** transcripts + board op-logs retained (PII-redacted) for human review, with an explicit
  retention policy and a student-facing disclosure in the terms.
- **Regression evals:** the RAGAs golden-set harness runs on every system-prompt or retrieval change;
  a drop in faithfulness or context precision beyond tolerance blocks the merge.

### 7.6 Acceptance criteria

- [ ] Cumulative voice time hits exactly 1200 s → polite message → socket closed, verified with a skewed client clock.
- [ ] Red-team corpus (200 prompts, en + ml): 100 % terminate, zero system-prompt leaks.
- [ ] Out-of-syllabus corpus: 100 % abstention, zero world-knowledge answers.
- [ ] No PII appears in any exported log or trace (automated scan over a 24 h export).
- [ ] Restore drill: a full Postgres restore from backup completes within the documented RTO.

**Checklist coverage:** D-18 … D-23, F-31 … F-36, G-37 … G-41, H-42 … H-46, I-47 … I-50.

---

## 8. Traceability — the 50 items

| Group | Items | Phase |
|-------|-------|-------|
| A. Real-time audio & WebSockets | 1–8 | 3 |
| B. Whiteboard synchronisation | 9–12 | 3 |
| C. Auth, RBAC, profile persistence | 13–17 | 1 |
| D. Session control & quota | 18–23 | 4 |
| E. RAG ingestion & vector search | 23–30 | 2 |
| F. Pedagogical flow & greetings | 31–36 | 4 (prompt) / 1 (flags) |
| G. Safety, guardrails, anti-jailbreak | 37–41 | 4 |
| H. Database, infra, security | 42–46 | 1 (schema, RL) / 4 (hardening) |
| I. Monitoring, observability, QA | 47–50 | 2 (evals) / 4 (tracing, cost) |

---

## 9. Environments & Rollout

| Env | Purpose | Data |
|-----|---------|------|
| `local` | docker-compose: Postgres, Redis, Qdrant, mock Live server | seeded fixtures |
| `staging` | full stack, real Gemini Live, synthetic students | anonymised |
| `prod` | live | real |

**Rollout:** pilot with one program (BA Malayalam, Semester 1) and ~50 students for two weeks with
daily transcript review, then widen by program. Feature flags: `greeting_v2`, `recap_enabled`,
`hybrid_search`, `rerank_enabled`, `text_only_fallback`.

---

## 10. Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Live API latency spikes | Lesson feels broken | Latency budget alarms; text-fallback mode; pre-warmed sessions |
| Malayalam OCR quality on scanned textbooks | Garbage chunks | Per-document confidence report; sub-admin must approve low-confidence docs before they go live |
| Model narrates before calling `board_ops` | NN-1 violation | Enforcement is in the gateway buffer, not the prompt — the model physically cannot outrun the board |
| Token cost overrun | Budget | Prompt caching + top-3 rerank + abstention + hard daily kill switch |
| Speech-to-speech bypasses text guardrails | Safety | Two-tier guard (§7.3); Tier 0 mutes upstream mid-utterance |
| Cross-course context leakage | Wrong teaching | Mandatory Qdrant payload filters + a 1 000-query assertion test in CI |

---

## 11. Definition of Done (release gate)

A release ships only when all five non-negotiables have green automated evidence, the acceptance
criteria of every phase pass in staging, the red-team and out-of-syllabus corpora are at 100 %, a
backup restore drill has been completed within RTO, and the cost dashboard shows a stable
per-session cost within budget.

---

## 12. `.claude/` Control Center

The repository carries its own agent configuration, committed alongside the code:

```
CLAUDE.md              team instructions, committed
CLAUDE.local.md        personal overrides, gitignored
.claude/
├── settings.json        permissions + config, committed
├── settings.local.json  personal permissions, gitignored
├── commands/            custom slash commands
│   ├── review.md            -> /project:review
│   ├── fix-issue.md         -> /project:fix-issue
│   ├── deploy.md            -> /project:deploy
│   ├── new-migration.md     -> /project:new-migration
│   └── eval.md              -> /project:eval
├── rules/               modular instruction files
│   ├── code-style.md
│   ├── testing.md
│   ├── api-conventions.md
│   ├── rag-pipeline.md
│   ├── realtime-audio.md
│   ├── whiteboard-sync.md
│   └── security.md
├── skills/              auto-invoked workflows
│   ├── rag-ingest/SKILL.md
│   ├── whiteboard-sync/SKILL.md
│   ├── security-review/SKILL.md
│   └── deploy/SKILL.md
└── agents/              subagent personas
    ├── code-reviewer.md
    ├── security-auditor.md
    ├── rag-evaluator.md
    └── realtime-debugger.md
```

---

## 13. Reference Prototype (`origin/master`)

The `master` branch holds a working Phase-1 prototype of Digi Guru on a **different stack**
(Node/Express + Prisma + Vite React). The decision is to build this plan on the specified stack
(Rust + Next.js + Qdrant) and treat `master` as a **reference prototype, not a codebase to extend**.

It is not dead weight — it is a validated requirements artefact. Mine it, do not port it.

### 13.1 What to carry over

| Asset on `master` | Why it matters |
|---|---|
| `backend/prisma/schema.prisma` | A data model already validated against the requirements. Its hierarchy (Program > Semester > Course > Block), `lscs` as an entity, and the `learning_sessions` / `auth_sessions` split are all reflected in §3.1. |
| `backend/prisma/migrations/` | Seed and reference data; the shape of the initial and Phase-3 migrations |
| `backend/src/validators/*.ts` | Zod schemas encoding the real field rules. The `.strict()` allow-list on student self-service PATCH is the pattern to reproduce in the Rust validators. |
| `backend/src/utils/` | `loginThrottle`, `tokens`, `cookies`, `audit`, `password` — the auth decisions are already made and reviewed; re-implement the same semantics. |
| `admin-app/src/pages/` | Working admin UX for Programs, Semesters, LSCs, Students, Settings — use as the spec for the Next.js admin console. |
| `student-app/src/pages/` | Signup, login, forgot/reset password, dashboard, courses, profile — the student flows, already designed. |
| `REQUIREMENT.md`, `ROADMAP.md` | Original requirement capture; cross-check against this plan before Phase 1 starts. |

### 13.2 Where the prototype falls short

These are gaps to close, not regressions to preserve:

- **Roles.** `Role` is only `STUDENT | ADMIN`. The required super-admin / sub-admin split, and
  `sub_admin_scopes` program scoping, do not exist yet.
- **Authorization granularity.** `requireRole(...roles)` is a role check, not a capability check,
  and carries no scope join — a sub-admin would be able to touch any program.
- **No RAG layer.** No vector store, no ingestion, no documents table.
- **No realtime layer.** No WebSocket gateway, no Gemini Live, no whiteboard, no SyncGate.
- **No quota or guardrails.** `learning_sessions.duration_seconds` exists, but nothing enforces
  NN-3, and there is no safety pipeline at all.

### 13.3 Branch policy

- `main` — this plan and the new implementation. The branch that ships.
- `master` — frozen reference. Do not merge it into `main`; do not delete it until Phase 1 is
  complete and the requirements it encodes have been fully transferred.
