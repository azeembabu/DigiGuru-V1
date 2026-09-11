---
name: security-review
description: Use before merging changes to auth, RBAC, guardrails, session handling, uploads, logging, or anything touching student data — and whenever a security review of the current branch is requested.
---

# Security Review

Review the branch diff against the threat model in `.claude/rules/security.md`.

## Checklist

**Authorization**
- Every new endpoint declares a capability via the extractor; no role checks buried in business logic.
- Sub-admin scoping enforced — verify the query filters by `sub_admin_scopes`, not just by role.
- No endpoint accepts a `student_id` from the request body where it should come from the token.

**Guardrails (NN-5)**
- Tier 0 still runs before any audio is forwarded upstream. A refactor that moves the tap after the
  Gemini send is a critical finding.
- Tier 1 teardown still writes a `safety_incidents` row and closes with code `4009`.
- Tier 2 output guard still runs before TTS release.

**Curriculum boundary (NN-4)**
- No code path reaches the model without retrieved context, and `Abstain` is handled explicitly.

**Data**
- No PII in logs, traces, metrics, or error messages.
- Only `PublicError` is serialised to clients — no SQL, stack traces, schema names, or prompts.
- New secrets read from the secret manager, not from an env file.

**Input**
- Uploads: type sniffing not extension trust, size cap, virus scan, storage outside the web root.
- All request bodies validated server-side, not only in the client form.
- No string-built SQL — `sqlx` macros only.

**Session**
- Quota still counted server-side, only while voice is active.
- Tokens short-lived, refresh rotation intact, cookies httpOnly + SameSite=Strict.

## Output

Report findings most-severe first with file, line, and a concrete exploit or failure scenario.
Distinguish confirmed issues from suspicions. Do not pad the list — an empty report is a valid
result and is more useful than speculative findings.
