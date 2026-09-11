# Security Rules

## Authorization

- Three roles: `super_admin`, `sub_admin`, `student`. Sub-admins are additionally scoped by
  `sub_admin_scopes` — a sub-admin for one program cannot touch another.
- PDF upload, embedding, and student administration are restricted to sub-admins and super admins.
- Authorization is explicit per handler via a capability extractor. There is no implicit allow, and
  no role check inside business logic.
- Changing a student program, semester, or block during a live session returns `409 SESSION_ACTIVE`
  and forces a session re-init.

## Guardrails (NN-5)

Three tiers, all required:

- **Tier 0 — pre-LLM tap.** A local ASR fork over the upstream audio; partial transcripts run
  through multilingual (en + ml) injection patterns, an obfuscation classifier, and a toxicity
  model. A hit **mutes the upstream mid-utterance** — no further audio reaches Gemini.
- **Tier 1 — Live input-transcription events.** A hit tears the socket down within 300 ms, writes a
  `safety_incidents` row, and moves the FSM to `JAILBREAK_HALT`.
- **Tier 2 — output guard.** Model output is screened before TTS release. A response citing no
  retrieved chunk, or leaking the system prompt, is dropped rather than spoken.

Curriculum boundary: answer only from retrieved context; textbook and chapter names may be
disclosed; with no context, say it is not covered in the textbook and offer the nearest
in-syllabus topic. Never fall back to world knowledge.

## Data handling

- **PII never leaves the process.** A log filter redacts phone, email, and name before export.
  Monitoring and traces carry `student_id` only.
- Passwords: argon2id. Refresh tokens stored hashed.
- Secrets in AWS Secrets Manager / Vault with rotation. No production secrets in env files, ever.
  CI runs secret scanning on every push.
- Transport: TLS 1.3, HSTS, strict CSP, WSS only.
- Rate limiting on `/auth/*`, uploads, and the WS upgrade — per IP and per account.
- Uploads: type sniffing (not extension trust), size cap, virus scan, and storage outside the web root.

## Error exposure

One `PublicError` type reaches clients. Stack traces, SQL, schema names, and system prompts are
logged internally and never serialised outward.

## Backups

Daily `pg_dump` plus WAL archiving to off-site storage, 30-day retention, monthly restore drill
against the documented RTO.
