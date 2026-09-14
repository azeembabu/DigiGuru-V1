# API Conventions

## REST

- Base: `/api/v1`. Version in the path; never break a shipped version.
- `snake_case` JSON fields, matching the database.
- Auth: short-lived access JWT (15 min) in an httpOnly, SameSite=Strict cookie; rotating refresh
  token stored hashed in Postgres.
- Every handler declares its capability via an axum extractor:
  `async fn upload(actor: Actor<RequireCap<{ Cap::UploadDocument }>>, ...)`.
- Errors are always this shape — never anything else:

```json
{ "error": { "code": "SESSION_ACTIVE", "message": "Cannot change program during an active session" } }
```

  `code` is a stable enum. `message` is safe for display. No details, no stack traces, no SQL.

### Status codes

`200` ok · `201` created · `400` validation · `401` unauthenticated · `403` capability denied ·
`409` state conflict (e.g. `SESSION_ACTIVE`) · `413` body over the route's size cap
(`PAYLOAD_TOO_LARGE`) · `422` semantically invalid · `429` rate limited ·
`503` upstream (Gemini/Qdrant) unavailable.

A framework rejection must never reach a client as-is. axum answers its own
extractor rejections with a plaintext body, which is not the envelope and which an
envelope-parsing client can only render as a generic failure — so any route whose
extractor can be rejected (notably a body-limit rejection on an upload) wraps it and
returns a `PublicError` instead. Map by status code, not by matching the rejection
enum: those enums are `#[non_exhaustive]`.

Two extractors exist for this and every route uses one of them:
`extractors::UploadBody` for raw bytes, and `extractors::JsonBody<T>` for a JSON
body. **`axum::Json<T>` must never appear in the argument position** — only as a
response type. A bare `axum::Json` answers a body it cannot deserialise with
plaintext naming the request struct's field and a line/column position
(`Failed to deserialize the JSON body into the target type: missing field
`course_id` at line 1 column 11`), which is both outside the envelope and a
disclosure: `/auth/*` is unauthenticated, so that text let anyone enumerate the
shape of the login and signup payloads.

A body that does not deserialise — malformed JSON, a missing field, a mistyped
field — is `400 VALIDATION_ERROR` on field `body`, with a message that says the
body was unacceptable and nothing more. The `422` in the table above is for a
request that parsed and was then semantically rejected by a handler (e.g.
`EXAM_NOT_MCQ`), never for one that never parsed. A missing or wrong
`Content-Type` is the same `400`; an oversize body is `413 PAYLOAD_TOO_LARGE`
without quoting a megabyte figure, since the per-route upload cap is not a JSON
endpoint's limit.

### Pagination and `X-Total-Count`

Paginated list endpoints (`/admin/programs`, `/admin/lscs`, `/admin/students`,
`/admin/users`) take `limit` and `offset` query parameters and return a bare JSON
**array** as the body. The total before pagination is returned in an `X-Total-Count`
response header, never in the body — these routes shipped with an array body, and
wrapping it in an envelope would break every existing caller. A client that needs a
page count reads the header.

- `limit`: 1-200. Out of range is `400 VALIDATION_ERROR` on field `limit`, never a
  silent clamp — a clamped page cannot be reconciled with `X-Total-Count`.
- `offset`: 0 or greater. Negative is `400 VALIDATION_ERROR`.
- Default `limit` is 50 for `/admin/students` and `/admin/users`, and 200 for
  `/admin/programs` and `/admin/lscs` (those two shipped unpaginated; the higher
  default keeps an existing caller whole).
- `q` is a case-insensitive substring search, trimmed; blank means no filter.
- CORS: `X-Total-Count` must stay on the gateway's `expose_headers` allow-list, or a
  browser client cannot read it.

## Student self-registration

`POST /api/v1/auth/signup` is the **only** way a student account is created —
`POST /admin/users` rejects `role: "student"` — and it completes onboarding on
its own. **No administrator approves a registration.** `users.status` defaults
to `active`, so there is no pending state to clear.

Registration does three things, not one, because any of them left undone leaves
the student unable to study:

1. Creates the `users` + `students` rows.
2. Enrols the student in **every course of the semester they chose**. A student
   reaches content only through `student_courses` (`CLAUDE.md`), so without this
   the dashboard has no courses and no classroom may be entered.
3. Sets `students.current_block_id` to the first active block of that semester —
   the lowest `block_no` of the lowest `code` course. This is what `/me/context`
   returns as `current_block` and what the classroom opens.

The response carries the **same session cookies `/auth/login` issues** (`dg_access`,
`dg_refresh`, both httpOnly/SameSite=Strict), so the client routes straight to the
dashboard rather than to a login form. `201 Created`:

```json
{ "user_id": "uuid", "student_id": "uuid", "role": "student",
  "is_first_login": true, "enrolled_courses": 3 }
```

`role` and `is_first_login` mirror `LoginResponse` so post-auth routing is one
code path on both. `enrolled_courses` lets a client distinguish an empty
dashboard caused by an empty semester from one caused by a bug; `0` means the
semester has no courses yet.

Steps 2 and 3 are **best-effort**: they are logged at error level on failure and
do not fail the request. The account already exists by then, and returning an
error for an email that is now registered would leave the student able neither to
retry nor to sign in.

This is not a relaxation of authorization. `student_courses` remains the single
enforcement point for content access; only the writer of the row changed. An
admin retains control afterwards — enrolments can be updated or dropped, and
`users.status` set to `inactive`/`suspended`, which `/auth/login` already
refuses with `403`.

## Admin console endpoints

| Method | Path | Capability |
|---|---|---|
| GET | `/api/v1/me` | authenticated, any role (self-only) |
| POST | `/api/v1/admin/blocks` | `ManagePrograms`, scoped via `course -> program_id` |
| GET | `/api/v1/admin/courses/{course_id}/blocks` | `ManagePrograms`, scoped via `course -> program_id` |
| PATCH | `/api/v1/admin/blocks/{id}` | `ManagePrograms`, scoped via `block -> course -> program_id` |
| GET | `/api/v1/admin/blocks/{block_id}/documents` | `UploadDocuments`, scoped the same way |
| GET | `/api/v1/admin/documents/{id}` | `UploadDocuments`, scoped the same way |
| GET | `/api/v1/admin/semesters/{id}` | `ManagePrograms`, scoped via the semester's `program_id` |
| GET | `/api/v1/admin/courses/{id}` | `ManagePrograms`, scoped via the course's `program_id` |
| GET | `/api/v1/admin/blocks/{id}` | `ManagePrograms`, scoped via `block -> course -> program_id` |
| GET | `/api/v1/admin/programs/{program_id}/courses` | `ManagePrograms`, scoped against the path `program_id` |
| GET | `/api/v1/admin/stats` | `ManagePrograms`; counts scoped for a sub-admin |

Each single-resource `GET` returns the **same** struct its create/list siblings return
for that resource — there is no second, detail-only shape to code against.

`GET /api/v1/me` — caller identity for **any** authenticated role. `/me/context` is
the student academic payload and `404`s for an admin; this is what an admin shell
reads instead. `scopes` is `[]` unless the caller is a `sub_admin`.

```json
{ "id": "uuid", "email": "string", "full_name": "string|null",
  "role": "super_admin|sub_admin|student", "status": "active|inactive|suspended",
  "scopes": [ { "program_id": "uuid", "code": "string", "name": "string" } ] }
```

`POST /api/v1/admin/blocks` — body `{ "course_id": "uuid", "block_no": 1..999,
"title": "2-200 chars", "description": "string|null" }`. `PATCH /api/v1/admin/blocks/{id}`
is partial — `{ "title"?, "description"?, "status"? }`, omitted fields untouched;
`course_id`/`block_no` are not editable (moving a block between courses would move it
past the scope check it was authorized under). Block responses:

```json
{ "id": "uuid", "course_id": "uuid", "block_no": 1, "title": "string",
  "description": "string|null", "status": "active|inactive" }
```

### Document upload limits

`POST /api/v1/admin/blocks/{block_id}/documents` takes the raw PDF bytes and caps the
body at **64 MiB**, set per route (`MAX_UPLOAD_BYTES` overrides it). The cap is *not*
global: axum's 2 MiB default stays in force on every JSON endpoint, because raising it
everywhere would widen the DoS surface on `/auth/*` for no benefit.

Over the cap returns `413`:

```json
{ "error": { "code": "PAYLOAD_TOO_LARGE",
             "message": "The uploaded file is larger than the 64 MB limit." } }
```

The message names the limit so a console can tell the admin how much to cut. The body
is also sniffed: it must begin with the `%PDF-` signature, or the request is rejected
`400 VALIDATION_ERROR` on field `file` — the declared `Content-Type` is not trusted.

Document responses carry the `documents` row plus the status and error of its most
recent `ingestion_jobs` attempt (`documents` has no error column of its own).
`storage_key` is never on the wire — it is a server-side path.

```json
{ "id": "uuid", "block_id": "uuid", "uploaded_by": "uuid", "title": "string",
  "sha256": "string", "page_count": 0, "ocr_confidence": 0.98,
  "status": "pending|parsing|pending_review|embedded|failed",
  "created_at": "RFC3339",
  "job_status": "pending|processing|completed|failed|null",
  "last_error": "string|null" }
```

`GET /api/v1/admin/stats` — dashboard counts. A sub-admin's figures are restricted to
its `sub_admin_scopes` programs; `lscs` is platform-wide for both roles because an LSC
has no program and every admin may already list them all.

```json
{ "programs": 0, "semesters": 0, "courses": 0, "blocks": 0, "lscs": 0, "students": 0,
  "documents": { "total": 0, "pending_review": 0, "embedded": 0, "failed": 0 } }
```

### `GET /api/v1/admin/analytics` — dashboard metrics and chart series

Capability `ManagePrograms`. **Additive: `GET /admin/stats` is unchanged and still
served**, because the existing dashboard tiles read it; this is a second, richer
endpoint rather than a breaking expansion of the first.

Scoping follows `/admin/stats` exactly — the caller's role decides *which query
runs*, never a filter applied to a platform-wide result. A `sub_admin` sees only
its `sub_admin_scopes` programs, reached by `blocks -> courses -> program_id`,
`documents -> blocks -> courses -> program_id`, `learning_sessions -> courses ->
program_id`, `board_events -> learning_sessions -> courses -> program_id`, and
`safety_incidents -> students -> program_id`. `lscs` stays platform-wide for both
roles, for the reason given under `/admin/stats`.

A sub-admin with no scopes gets zeroes and empty series — the correct answer, not
a reason to fall back to the unscoped query.

Every counter is a non-negative integer and every series is present (possibly
empty), so a client never has to null-check a metric. Averages and percentiles
are `null` when there is nothing to average — `0` would read as a real measured
zero, which is a different and wrong claim.

```json
{
  "catalogue": { "programs": 0, "semesters": 0, "courses": 0, "blocks": 0,
                 "blocks_active": 0, "blocks_inactive": 0, "lscs": 0,
                 "blocks_without_documents": 0, "courses_without_blocks": 0,
                 "semesters_without_courses": 0 },

  "students":  { "total": 0, "active": 0, "inactive": 0, "suspended": 0,
                 "first_login_pending": 0, "new_last_30d": 0,
                 "without_enrollment": 0, "enrollments_active": 0,
                 "enrollments_completed": 0, "enrollments_dropped": 0 },

  "documents": { "total": 0, "pending": 0, "parsing": 0, "pending_review": 0,
                 "embedded": 0, "failed": 0, "total_pages": 0,
                 "avg_ocr_confidence": null,
                 "jobs_pending": 0, "jobs_processing": 0, "jobs_completed": 0,
                 "jobs_failed": 0, "jobs_retried": 0 },

  "sessions":  { "total": 0, "in_progress": 0, "completed": 0, "abandoned": 0,
                 "last_7d": 0, "active_voice_ms_total": 0,
                 "active_voice_ms_avg": null,
                 "ended_quota": 0, "ended_idle": 0, "ended_user": 0,
                 "ended_jailbreak": 0, "ended_error": 0 },

  "whiteboard": { "ops_total": 0, "ops_acked": 0, "ops_unacked": 0,
                  "ack_p50_ms": null, "ack_p95_ms": null,
                  "violation_rate": null },

  "safety":    { "total": 0, "tier0": 0, "tier1": 0, "tier2": 0, "last_7d": 0,
                 "jailbreak": 0, "toxicity": 0, "out_of_scope": 0 },

  "series": {
    "sessions_daily":      [ { "day": "2026-09-13", "count": 0, "voice_ms": 0 } ],
    "students_daily":      [ { "day": "2026-09-13", "count": 0 } ],
    "documents_daily":     [ { "day": "2026-09-13", "count": 0 } ],
    "incidents_daily":     [ { "day": "2026-09-13", "count": 0 } ],
    "documents_by_status": [ { "label": "embedded", "count": 0 } ],
    "session_end_reasons": [ { "label": "quota", "count": 0 } ],
    "ack_latency_buckets": [ { "label": "0-100ms", "count": 0 } ],
    "top_blocks":          [ { "label": "Block 3 - Prosody", "count": 0 } ]
  }
}
```

Rules the series obey, so a chart can render them without post-processing:

- The four `*_daily` series are **gap-filled**: exactly 30 rows, oldest first,
  ending today in UTC, with `count: 0` for days that had no rows. A chart that
  has to infer missing days draws a misleading line.
- `documents_by_status`, `session_end_reasons` and `ack_latency_buckets` return a
  **fixed set of labels in a fixed order**, zeros included, so a legend and its
  colours stay stable between refreshes instead of reordering as data arrives.
- `ack_latency_buckets` are `0-100ms`, `100-250ms`, `250-400ms`, `>400ms` —
  the last bucket is past the NN-1 `HOLD_MAX` ceiling, so a non-zero count there
  is a whiteboard-first violation and the UI marks it as such.
- `whiteboard.violation_rate` is `ops_unacked / ops_total` as a 0..1 float, the
  same quantity `wb_violation` tracks in CI.
- `top_blocks` is at most 10 rows, descending by `count`.

### Drill-down list endpoints

The dashboard's metrics are clickable: each opens the records behind the number.
Four groups had no list API, so these add one each. All four follow the existing
paginated-list conventions exactly — capability `ManagePrograms`, a bare JSON
**array** body, the total before pagination in `X-Total-Count`, `limit` 1-200
(default 50) and `offset` >= 0 validated rather than clamped, and the caller's
role selecting the scoped or unscoped query as `/admin/analytics` does.

Scoping paths are the same as `/admin/analytics`: enrolments via
`student_courses -> courses -> program_id`, sessions via
`learning_sessions -> courses -> program_id`, board events via
`board_events -> learning_sessions -> courses -> program_id`, incidents via
`safety_incidents -> students -> program_id`. A sub-admin with no scopes gets an
empty array, never a platform-wide fallback.

Every row is denormalised enough to render without a second request — a drill-down
that has to re-fetch a name per row is an N+1 in the browser.

#### `GET /api/v1/admin/enrollments`

Filters: `status` (`active|completed|dropped`), `course_id`, `student_id`,
`q` (student name, roll number, course code, course name). Ordered by
`assigned_at` descending.

```json
[ { "id": "uuid", "student_id": "uuid", "student_name": "string",
    "roll_number": "string", "course_id": "uuid", "course_code": "string",
    "course_name": "string", "semester_number": 1,
    "status": "active|completed|dropped", "assigned_at": "RFC3339" } ]
```

#### `GET /api/v1/admin/sessions`

Filters: `status` (`in_progress|completed|abandoned`), `end_reason`
(`quota|idle|user|jailbreak|error`), `student_id`, `block_id`, `course_id`,
`from`/`to` (RFC3339, filtering `started_at`). Ordered by `started_at`
descending.

`active_voice_ms` is the NN-3 server-authoritative figure and never exceeds
1200000. `end_reason` is `null` while a session is still running.

```json
[ { "id": "uuid", "student_id": "uuid", "student_name": "string",
    "roll_number": "string", "course_id": "uuid", "course_code": "string",
    "block_id": "uuid", "block_no": 1, "block_title": "string",
    "started_at": "RFC3339", "ended_at": "RFC3339|null",
    "active_voice_ms": 0, "status": "in_progress|completed|abandoned",
    "end_reason": "quota|idle|user|jailbreak|error|null",
    "last_topic": "string|null", "last_page": 0,
    "board_ops": 0, "board_violations": 0 } ]
```

`board_ops` and `board_violations` are per-session roll-ups so the list can flag
a bad session without opening it. A violation is an op with `acked_ms` NULL or
above the 400 ms `HOLD_MAX`.

#### `GET /api/v1/admin/board-events`

Filters: `session_id`, `violations_only` (`true` = `acked_ms IS NULL OR
acked_ms > 400`), `bucket` (`0-100ms|100-250ms|250-400ms|>400ms`, matching the
analytics buckets). Ordered by `emitted_at` descending, or by `turn_seq`
ascending when `session_id` is given — a single session reads as a transcript,
not a reverse feed.

`op_kind` is lifted out of the `op` JSONB so a list can render without parsing
it; `op` carries the full validated payload for a detail view.

```json
[ { "id": 1, "session_id": "uuid", "turn_seq": 1,
    "op_kind": "heading|bullets|math|draw|image|highlight",
    "op": { }, "emitted_at": "RFC3339", "acked_ms": 148,
    "is_violation": false } ]
```

#### `GET /api/v1/admin/safety-incidents`

Filters: `tier` (`0|1|2`), `kind` (`jailbreak|toxicity|out_of_scope`),
`student_id`, `session_id`, `from`/`to`. Ordered by `created_at` descending.

`excerpt` is already PII-redacted at write time (`security.md`) and is returned
as stored — this endpoint neither re-redacts nor un-redacts it.

```json
[ { "id": "uuid", "session_id": "uuid|null", "student_id": "uuid",
    "student_name": "string", "roll_number": "string",
    "kind": "jailbreak|toxicity|out_of_scope", "tier": 0,
    "excerpt": "string", "created_at": "RFC3339" } ]
```

### Course responses

`CourseResponse` carries `semester_number` and `semester_name` alongside the
`semester_id` it always had, on **every** endpoint that returns a course — create,
update, both list routes, and `GET /admin/courses/{id}`. Purely **additive**: no
existing field changed or moved, so no caller breaks. The reason is the same as on
`StudentResponse` — a course list spanning semesters would otherwise have to re-fetch
the semester list just to label its rows.

```json
{ "id": "uuid", "program_id": "uuid", "semester_id": "uuid",
  "semester_number": 1, "semester_name": "string",
  "code": "string", "name": "string", "description": "string|null" }
```

`GET /api/v1/admin/programs/{program_id}/courses` returns every course in a program
across all of its semesters — the flattened content list the console drills into from
a program. Ordered by `semester_number` ascending, then course `code` ascending, so a
caller grouping by semester renders straight down the list without re-sorting.
Supports `q` (code, name), `limit`, `offset`, and `X-Total-Count` like every other
paginated admin list. `404 NOT_FOUND` if the program does not exist, matching
`GET /admin/programs/{program_id}/semesters`.

This is a read-side convenience only. The semester model is unchanged: semesters keep
their own routes, every course row still carries its `semester_id`, and
`GET /admin/semesters/{semester_id}/courses` stays exactly as it was.

### Student responses

`StudentResponse` (list and `GET /admin/students/{id}`) carries, in addition to the
`students` columns: `status` — the linked `users.status`
(`active|inactive|suspended`), the same field the `status` query parameter filters on
— plus `semester_number` and `semester_name`, denormalised from the join the query
already makes. Semesters are otherwise listable only per program, so naming each
row's semester client-side would mean fanning out across the whole catalogue for one
page of students; there is deliberately **no** flat `GET /admin/semesters`.

### Breaking change: `GET /admin/users/{id}/scopes`

This endpoint returned a bare `["uuid", ...]`. It now returns the same scope shape as
`GET /api/v1/me`:

```json
[ { "program_id": "uuid", "code": "string", "name": "string" } ]
```

Deliberately breaking, while the endpoint is unreleased and has a single caller: bare
ids forced a client-side join against the programs list, and a scope whose program had
not been loaded could not be named at all.

### Removing academic content

`DELETE` on a programme, course, block or unit, each paired with a
`deletion-impact` preview. Capability `ManagePrograms`, scoped exactly as the
matching `GET` is — a sub-admin may remove only inside its own programmes.

| Method | Path | Capability |
|---|---|---|
| GET | `/api/v1/admin/programs/{id}/deletion-impact` | `ManagePrograms`, scoped against the path `program_id` |
| DELETE | `/api/v1/admin/programs/{id}` | same |
| GET | `/api/v1/admin/courses/{id}/deletion-impact` | `ManagePrograms`, scoped via `course -> program_id` |
| DELETE | `/api/v1/admin/courses/{id}` | same |
| GET | `/api/v1/admin/blocks/{id}/deletion-impact` | `ManagePrograms`, scoped via `block -> course -> program_id` |
| DELETE | `/api/v1/admin/blocks/{id}` | same |
| GET | `/api/v1/admin/documents/{id}/deletion-impact` | `ManagePrograms`, scoped via `document -> block -> course -> program_id` |
| DELETE | `/api/v1/admin/documents/{id}` | same |

`documents` is the route segment for what the console calls a **unit**.

#### These deletions destroy student data, by design

`blocks -> exams -> exam_attempts -> exam_attempt_answers` is `ON DELETE
CASCADE` the whole way down, so removing a block permanently deletes every
student's marks for that block's exams; removing the course or programme above
it does the same, for more of them. `question_pool` and `flashcards` go the same
way. There is no archive fallback on this path and no undo.

That is why the preview exists. `deletion-impact` returns the counts a
confirmation dialogue needs, and the console requires the item's own name to be
typed back before it will call `DELETE`. The preview is **advisory, not a
lock**: no server-side token ties one call to the other, and a script can skip
it. The real protections are the capability check, the scope check and the audit
row (`admin.{program,course,block,unit}.delete`).

```json
{ "semesters": 1, "courses": 2, "blocks": 3, "units": 4, "exams": 5,
  "exam_attempts": 6, "questions": 7, "flashcards": 8,
  "learning_sessions": 9, "board_events": 10, "enrollments": 11,
  "students_affected": 12,
  "can_delete": true, "blocked_reason": null }
```

Counts only, never ids: the dialogue reports scale, and a list of affected
students would be a PII export from a screen that does not need one.
`students_affected` is distinct students, not a sum — one student may lose an
attempt *and* a session.

`DELETE` answers `{ "deleted": true, "impact": { ... } }` with the same shape, so
a console renders "this is what went" with the code that rendered "this is what
will go".

#### What removal never touches

**Student accounts.** `students.program_id` is `NOT NULL ON DELETE RESTRICT` and
that restriction is kept deliberately: removing a programme is a content
operation, and deleting the people registered on it is a different decision.
A programme with students on it answers `409 PROGRAM_HAS_STUDENTS` and changes
nothing; its preview returns `can_delete: false` with `blocked_reason` set, so
the console can disable the button and explain rather than offer an action that
will fail.

**Safety incidents.** NN-5 requires them persisted, so a removed session's
incidents survive with `session_id` nulled.

Everything else that would otherwise block the delete is unwound leaf-first
inside one transaction: board events, sessions, uploaded units, and
`students.current_block_id` (nulled — choosing a different block for a student
would be inventing a curriculum decision).

Qdrant vectors for removed units are purged **after** the Postgres transaction
commits. The other order would leave a document row the tutor can no longer
retrieve for if the transaction rolled back, which is an NN-4 violation no test
would catch; this way the worst case is orphan vectors, which every query filters
out by `block_no` anyway.

### Exams and attempts

An exam belongs to a **block** — the teachable unit — and reaches its program,
semester and course through the existing hierarchy; it carries no `program_id`
of its own. Admin scope is therefore resolved `exam -> block -> course ->
program_id`, and the owning program is resolved *before* the capability check,
so a sub-admin outside the scope cannot tell an existing exam from a missing
one. Scores are always **numbers** on the wire (`score`, `max_score`); a
rendered "18/20" is the client's job.

| Method | Path | Capability |
|---|---|---|
| POST | `/api/v1/admin/exams` | `ManagePrograms`, scoped via `block -> course -> program_id` |
| GET | `/api/v1/admin/blocks/{block_id}/exams` | `ManagePrograms`, scoped the same way |
| GET | `/api/v1/admin/exams/{id}` | `ManagePrograms`, scoped via `exam -> block -> course -> program_id` |
| GET | `/api/v1/admin/exams/{exam_id}/attempts` | `ManagePrograms`, scoped the same way |
| GET | `/api/v1/student/exam-attempts` | `ViewOwnExams` — self-only |
| GET | `/api/v1/student/exam-attempts/{id}` | `ViewOwnExams` — self-only |

`POST /api/v1/admin/exams` — body `{ "block_id": "uuid", "title": "2-200 chars",
"description": "string|null", "max_score": 0.01..10000, "duration_minutes":
1..600|null, "status": "draft|published|archived"|null }`. An omitted `status`
is `draft`: an exam is never visible to students until an admin publishes it.
`UNIQUE (block_id, title)`. Exam responses carry the flat ancestry of their
block, so a console renders a breadcrumb from one response:

```json
{ "id": "uuid", "block_id": "uuid", "block_no": 1, "block_title": "string",
  "course_id": "uuid", "course_code": "string", "semester_id": "uuid",
  "program_id": "uuid", "title": "string", "description": "string|null",
  "max_score": 20.0, "duration_minutes": 45,
  "status": "draft|published|archived",
  "created_by": "uuid", "created_at": "RFC3339" }
```

`GET /api/v1/admin/blocks/{block_id}/exams` supports `q` (title), `limit`,
`offset` and `X-Total-Count` like every other paginated admin list; default
`limit` is 50. Ordered by `title` ascending.

`GET /api/v1/admin/exams/{exam_id}/attempts` — every student's attempts at one
exam, newest first, paginated the same way (`limit`, `offset`,
`X-Total-Count`, default 50):

```json
{ "id": "uuid", "exam_id": "uuid", "student_id": "uuid",
  "student_name": "string", "roll_number": "string", "attempt_no": 1,
  "score": 18.0, "max_score": 20.0,
  "status": "in_progress|submitted|graded|abandoned",
  "started_at": "RFC3339", "submitted_at": "RFC3339|null" }
```

`score` is `null` until the attempt is marked; only `graded` guarantees it is
present.

#### Student routes are self-only

`/api/v1/student/*` is a new namespace for a student's own accumulated
records, as distinct from `/me/*` (identity, academic context, profile).
Neither exam route takes a `student_id` in its path, query or body: the
subject is resolved from the caller's own access token and bound into the
query, so **there is no request a client can make for another student's
attempts**. An attempt id belonging to another student returns `404 NOT_FOUND`,
never `403` — a `403` would confirm the id exists.

`GET /api/v1/student/exam-attempts` — the dashboard's "recent exams" cards,
newest first (`submitted_at` descending, then `started_at`, then `attempt_no`,
so the ordering is total and the leading card is unambiguously the most recent
attempt). Paginated exactly like the admin lists: bare array body, `limit`
1-200 default 50, `offset`, `X-Total-Count`, validated and never clamped.
`GET /api/v1/student/exam-attempts/{id}` returns the **same** object for one
attempt, so a review screen codes against a single card type.

This list deliberately includes `in_progress` attempts, and that is load-bearing:
it is how a client recovers from `409 EXAM_ATTEMPT_ACTIVE` (see "Sitting an
exam"). The conflict body cannot carry the live attempt's id — the error
envelope has no details field — so the client reads the `in_progress` row's `id`
from here and resumes at `GET /api/v1/student/exams/attempts/{id}`. Filtering
`in_progress` out of this list would break exam resume.

The card carries everything it renders, denormalised, so a page of cards is
one request. It deliberately has no `student_id`, `student_name` or
`roll_number` field at all:

```json
{ "id": "uuid", "exam_id": "uuid", "exam_title": "string",
  "block_id": "uuid", "block_no": 1, "block_title": "string",
  "course_id": "uuid", "course_code": "string", "course_name": "string",
  "attempt_no": 2, "score": 18.0, "max_score": 20.0,
  "status": "in_progress|submitted|graded|abandoned",
  "started_at": "RFC3339", "submitted_at": "RFC3339|null" }
```

### The question pool

The MCQ bank the exam module samples from. A pool belongs to a **course**: the
academic taxonomy is Program > Semester > Course > Unit/Module, and a pool is
pre-populated per course with the unit/module link explicitly **optional**. So
`course_id` is required on every write and `block_id` is the optional module
pointer. Program and semester are reached through
`question_pool -> courses` and are not duplicated on the row. Admin scope is
resolved `question -> course -> program_id`, and the owning program is resolved
*before* the capability check, so a sub-admin outside the scope cannot tell an
existing question from a missing one.

| Method | Path | Capability |
|---|---|---|
| POST | `/api/v1/admin/question-pool` | `ManagePrograms`, scoped via `course -> program_id` |
| POST | `/api/v1/admin/question-pool/bulk` | `ManagePrograms`, scoped per row the same way |
| GET | `/api/v1/admin/question-pool` | `ManagePrograms`; the list is scoped for a sub-admin |
| PATCH | `/api/v1/admin/question-pool/{id}` | `ManagePrograms`, scoped via `question -> course -> program_id` |
| GET | `/api/v1/admin/programs/{program_id}/question-pool-counts` | `ManagePrograms`, scoped against the path `program_id` |

When `block_id` is present it must belong to the given `course_id`, or the write
is `400 VALIDATION_ERROR` on field `block_id`. That is checked in the handler,
not by a CHECK constraint: the constraint would need a `blocks` subquery and
cannot have one. Doing it in the handler also lets the bulk path name the
offending **row number**, which a trigger could not.

Every paper is exactly **A/B/C/D**: `options` is four non-blank strings and
`correct_option_index` is `0..=3`. `explanation` is **mandatory** — it drives
the post-exam feedback screen, so it is `"string"` on every admin response and
on every review payload, never `null`. An admin response carries
`correct_option_index` and `explanation` because an author has to see the key
they are authoring; the student-facing paper is a different type that has no
field for either (see "Sitting an exam" below).

```json
{ "id": "uuid", "course_id": "uuid", "course_code": "string",
  "semester_id": "uuid", "program_id": "uuid",
  "block_id": "uuid|null", "block_no": 1, "block_title": "string|null",
  "topic": "string", "question_text": "string",
  "options": ["A","B","C","D"], "correct_option_index": 1,
  "explanation": "string",
  "assessment_type": "assignment|mid_term_quiz|semester_exam",
  "difficulty_level": "beginner|intermediate|advanced",
  "status": "active|retired",
  "created_by": "uuid", "created_at": "RFC3339" }
```

The three module fields are `null` **together**: a course-wide question is the
normal shape, not a row with a missing value. `course_id` and `course_code` are
never null. There is deliberately no `course_name` or `semester_number` on this
response — a row is labelled from `course_code`, and the console already holds
the course list it navigated through to reach the pool.

`POST /api/v1/admin/question-pool` — body is that shape without the server-set
fields: `{ "course_id", "block_id"?, "topic", "question_text", "options",
"correct_option_index", "explanation", "assessment_type", "difficulty_level"? }`.
An omitted `block_id` is a course-wide question; an omitted `difficulty_level`
is `beginner`, matching the column default and the tutor's tier 0
(`pedagogy.md`).

`PATCH /api/v1/admin/question-pool/{id}` is partial — `{ "block_id"?, "topic"?,
"question_text"?, "options"?, "correct_option_index"?, "explanation"?,
"assessment_type"?, "difficulty_level"?, "status"? }`, omitted fields
untouched. `block_id` **is** editable: it only re-files a question within its
own course, and the new module is validated against that course, read back from
the stored row rather than taken from the request. `course_id` is **not**
editable — moving a question to another course would move it past the scope
check it was authorized under *and* into a different paper's pool. A question is
retired with `status: "retired"`, never deleted: attempts that already asked it
keep referencing the row, so a graded paper stays reviewable.

`GET /api/v1/admin/question-pool` is a plain paginated admin list: bare array
body, `X-Total-Count`, `limit` 1-200 (default 50) and `offset` validated rather
than clamped. Filters: `program_id`, `semester_id`, `course_id`, `block_id`,
`assessment_type`, `difficulty_level`, `status`, and `q` (topic, question text).
Filtering on `block_id` returns only the questions filed under that module —
never the course-wide ones, which is usually most of the pool. The caller's role
selects the scoped or unscoped query as `/admin/analytics` does; a sub-admin with
no scopes gets an empty array, never a platform-wide fallback.

`GET /api/v1/admin/programs/{program_id}/question-pool-counts` — every course in
one program with its active question count, **including courses with none**.
That is the point of the endpoint: a Program -> Semester -> Course navigation has
to show which pools are still empty, which a `GROUP BY` over the pool would
hide. Not paginated — a program has tens of courses, and the screen is one tree.
Ordered by `semester_number` then `course_code`.

```json
[ { "course_id": "uuid", "course_code": "string", "course_name": "string",
    "semester_id": "uuid", "semester_number": 1, "active_questions": 0 } ]
```

#### Bulk import

`POST /api/v1/admin/question-pool/bulk` takes any of **four** formats. The body
is capped at **8 MiB** on this route alone (axum's 2 MiB default stays in force
on every other JSON endpoint) and at **500 rows**.

The format is decided by **sniffing the bytes**, never by the declared
`Content-Type` or a filename — the same posture the PDF upload takes with
`%PDF-`. `.xlsx` and `.docx` are *both* ZIP containers opening `PK\x03\x04`, so
the magic bytes only get as far as "a ZIP"; which one it is is decided by the
OOXML part inside (`xl/workbook.xml` vs `word/document.xml`). A ZIP carrying
neither is refused.

**The authoritative tabular header**, shared by CSV, `.xlsx` and `.docx`, in any
order and case-insensitive:

```
course_id,block_id,topic,question_text,option_a,option_b,option_c,option_d,correct_option,explanation,assessment_type,difficulty_level
```

- `course_id` is **required** — the pool is per course.
- `block_id` is the **optional** unit/module. A blank cell is a course-wide
  question, not an error.
- `difficulty_level` is the other optional column; omitting it makes every row
  `beginner`.
- `correct_option` is a **letter**: `A`, `B`, `C` or `D`, case-insensitive. A
  **number is refused**, not interpreted — `1` could mean "the first option" or
  "index 1", and guessing wrong silently marks the wrong option correct. That is
  the one failure mode in this route worth being rigid about, so the rejection
  message says why. The column may also be spelled `correct_option_index`, so a
  spreadsheet built from a JSON export imports without a rename; the *value* is
  still a letter either way.
- A column named twice is rejected rather than resolved. A merged header cell in
  a spreadsheet is one of the ways that happens.
- The JSON channel is the exception and keeps the machine-readable zero-based
  `correct_option_index` integer, where the base is unambiguous because it is
  typed rather than typed in.

Per format:

- **JSON** — an array of the create body. Rows are read individually so a bad
  one reports as `row N` rather than as a byte offset.
- **CSV** — UTF-8, header row first. Quoted fields may contain commas, escaped
  quotes (`""`) and newlines.
- **`.xlsx`** — the **first worksheet only**, exact header row in row 1. Renamed
  or duplicated columns are rejected rather than guessed at. Trailing blank rows
  left by deleted content are ignored, not imported as empty questions.
- **`.docx`** — exactly **one** supported layout: the **first table** in the
  document, whose first row is that same header row and whose remaining rows are
  one question each. Free-form Word text is **not parsed at all** — no `Q:` /
  `A)` / `Ans:` heuristics, no guessing at paragraph structure — because
  misreading an answer key is far worse than refusing a file. Anything else is a
  `VALIDATION_ERROR` that quotes the expected header and points the author at
  `.csv`/`.xlsx`. A cell's text is every run inside it concatenated (Word splits
  a typed sentence across runs), and a nested table's text is never folded into
  the enclosing cell.

**Every row is validated before any row is written**, and the write is a single
statement, so the import is atomic in both the handler and the database: a
half-imported pool is worse than a rejected one, because nobody can tell which
half arrived. Every distinct `course_id` is authorized and every `block_id`
checked against its course before the insert — a sub-admin slipping one
out-of-scope course into a 500-row import has the whole import rejected, not 499
questions written.

Per-row failures come back in the ordinary envelope — there is no second error
shape. The `VALIDATION_ERROR` message is `row N: ...` clauses joined by `"; "`,
ending with its own `nothing was imported` clause, so a console splits on
`"; "` and renders each clause as one checklist line (at most 20 rows are
quoted; the rest are counted):

```json
{ "error": { "code": "VALIDATION_ERROR",
             "message": "questions: row 3: exactly 4 options are required (A, B, C and D), got 3; row 7: explanation is required; nothing was imported" } }
```

Success is `{ "imported": 42, "format": "json|csv|xlsx|docx" }`. `imported`
always equals the rows submitted; a partial count is not a state this route can
return. `format` echoes what was sniffed, so an author who meant to send a
spreadsheet and sent something else can see what happened.

`exams` gained two nullable columns that pair: `question_count` (1-200) and
`assessment_type`. Both `null` means **this is not an MCQ exam** — a written or
oral assessment marked by hand, which is what every exam created before the
question pool is. `POST /admin/exams` accepts both and rejects one without the
other as `400 VALIDATION_ERROR` on `question_count`; `ExamResponse` carries
both, purely additively.

### The student dashboard

`GET /api/v1/student/dashboard` — capabilities `ViewOwnContext` **and**
`ViewOwnExams`, because the payload is both the academic context and the
student's own exam record. An admin holds the first but not the second and is
`403`. Self-only with no parameters at all: there is no request shape that
could name another student.

One request hydrates the whole screen. Six separate calls would paint the page
in six stages and each would re-resolve the same `students` row.

```json
{ "student_info": { "name": "string", "roll_number": "string",
    "program": "string", "program_id": "uuid", "semester": 1,
    "avatar_url": null },
  "continue_learning": { "session_id": "uuid", "course_id": "uuid",
    "course_title": "string", "block_id": "uuid", "block_no": 1,
    "block_title": "string", "chapter_name": "string|null",
    "page_number": 0, "para_index": null,
    "progress_percentage": 0.0, "resume_summary": "string|null",
    "last_active_at": "RFC3339" },
  "quota_status": { "minutes_used": 0, "minutes_max": 20,
    "ms_used": 0, "ms_remaining": 1200000,
    "is_locked": false, "resets_at": "RFC3339", "timezone": "string" },
  "exam_history": [ { "attempt_id": "uuid", "exam_id": "uuid",
    "exam_title": "string", "score": 8.0, "max_score": 10.0,
    "percentage": 80.0, "status": "in_progress|submitted|graded|abandoned",
    "submitted_at": "RFC3339|null", "weak_topics": ["string"] } ],
  "saved_resources": { "preserved_notes_count": 0,
    "flashcards_total": 0, "flashcards_due_for_review": 0,
    "revisions_due_count": 0 } }
```

- `continue_learning` is `null` for a student with no sessions. A first-login
  dashboard renders an empty state; it does not fail.
- `progress_percentage` is distinct blocks of that course with a `completed`
  session, over blocks in the course, as 0..100. Counted distinctly so
  re-studying a block cannot push it past 100; a course with no blocks is `0.0`.
- `para_index` is always `null`. Paragraph position lives in the Redis session
  state (`sess:{session_id}`), not in Postgres, and resume targets
  course -> block -> chapter -> page.
- `quota_status` is a **read** of the NN-3 ledger and never charges or extends
  it — only the socket task holding a live session may. `minutes_max` is
  derived from the ledger's own 20-minute constant, and `resets_at` is the next
  midnight in `students.timezone`, so a student is never told their allowance
  resets at a UTC hour that is mid-afternoon for them. `timezone` is echoed so
  a client can render "resets in N hours" without guessing.
- `exam_history` is the three most recent attempts. `weak_topics` is empty for
  an `in_progress` attempt: before submission, which answers are wrong is part
  of the answer key.
- `avatar_url` is always `null` — there is no avatar column and no upload
  route; the field exists so the header does not change shape when one lands.
- `preserved_notes_count` has no list endpoint, deliberately. A note export is
  a client-side render of the board op-log (`pedagogy.md`), so `note_reminders`
  is the only server-side trace and the count is all there is to serve.

`GET /api/v1/student/revisions` — capability `ViewOwnContext`. The due
`note_reminders` for the caller, most recently due first, paginated like every
other list (bare array, `X-Total-Count`, `limit` 1-200 default 50, `offset`
validated not clamped). "Due" is resolved against the student's **own**
calendar date, not the server's — a reminder set for tomorrow in Kochi must not
surface because a UTC server has already rolled over.

```json
[ { "id": "uuid", "session_id": "uuid",
    "event_from_id": 1, "event_to_id": 9, "board_ops_count": 9,
    "course_id": "uuid", "course_code": "string",
    "block_id": "uuid", "block_no": 1, "block_title": "string",
    "topic": "string|null", "remind_at": "2026-09-13",
    "surfaced_at": "RFC3339|null", "created_at": "RFC3339" } ]
```

`board_ops_count` is counted, not derived from the id span: `board_events.id`
is a global sequence shared with every other session, so `to - from` is not a
row count.

### Flashcards

| Method | Path | Capability |
|---|---|---|
| GET | `/api/v1/student/flashcards` | `ViewOwnContext` — self-only |
| POST | `/api/v1/student/flashcards/{id}/review` | `ViewOwnContext` — self-only |

There is deliberately **no create route**. Cards are written by the live
session through the internal `crates/db` path when a turn produces one; nothing
on this path calls an LLM, and a student cannot author a card into their own
notebook from the browser.

`GET` takes `due_only` (`true` narrows to cards due on or before the student's
local today), `block_id`, `limit`, `offset`, and returns a bare array with
`X-Total-Count`. `POST .../review` takes `{ "recalled": true }` and returns the
same card with its advanced schedule — `interval_days`, `ease` and `due_on` are
computed server-side by the SM-2-lite step, so a client cannot schedule a card
into the far future to clear its queue.

```json
{ "id": "uuid", "block_id": "uuid", "block_no": 1, "block_title": "string",
  "front": "string", "back": "string", "topic": "string|null",
  "source_session_id": "uuid|null", "due_on": "2026-09-13",
  "interval_days": 6, "ease": 250, "is_due": true,
  "reviewed_at": "RFC3339|null", "created_at": "RFC3339" }
```

`ease` is SM-2 ease in permille (250 = 2.50), kept integral so a schedule is
exactly reproducible. `is_due` is computed against the student's local today so
the client never compares dates across zones. The card carries no
`student_id`, `student_name` or `roll_number` field at all.

### Sitting an exam

| Method | Path | Capability |
|---|---|---|
| GET | `/api/v1/student/exams` | `ViewOwnExams` — self-only |
| POST | `/api/v1/student/exams/{exam_id}/attempts` | `ViewOwnExams` — self-only |
| GET | `/api/v1/student/exams/attempts/{id}` | `ViewOwnExams` — self-only |
| PATCH | `/api/v1/student/exams/attempts/{id}/answers` | `ViewOwnExams` — self-only |
| POST | `/api/v1/student/exams/attempts/{id}/submit` | `ViewOwnExams` — self-only |

An attempt is addressed by its own id, not by `{exam_id}/attempts/{id}`: an
attempt id is unique and already carries its exam, and a second addressable
path for the same row is a second thing to keep checked.

#### The answer key never leaves the server before submission

`correct_option_index` and `explanation` are absent from the paper payload's
question type **as fields**, not set to `null` — there is no value a handler
could assign that would leak the key, because there is nowhere to put it. The
review payload is a separate type, built only from a query that refuses an
`in_progress` attempt in SQL. Grading reads the key in SQL and returns a tally;
on that path the key never transits the gateway as a value at all.

`GET /api/v1/student/exams` — published exams in the caller's enrolled courses
(`student_courses -> courses -> blocks -> exams`, `status = 'published'` only).
A draft or archived exam is not content a student may sit, and the predicate is
in the query. Paginated like every other list.

```json
[ { "id": "uuid", "block_id": "uuid", "block_no": 1, "block_title": "string",
    "course_id": "uuid", "course_code": "string", "course_name": "string",
    "semester_id": "uuid", "program_id": "uuid",
    "title": "string", "description": "string|null",
    "max_score": 10.0, "duration_minutes": 45, "time_limit_minutes": 45,
    "question_count": 10,
    "assessment_type": "assignment|mid_term_quiz|semester_exam",
    "attempts_used": 1, "best_percentage": 80.0 } ]
```

`time_limit_minutes` is the same number as `duration_minutes` under the name
the runner's countdown uses. `best_percentage` is `null` when nothing is graded
yet — a `0` would read as a measured score. `question_count` and
`assessment_type` are `null` together for an exam that is not an MCQ paper.

`POST /api/v1/student/exams/{exam_id}/attempts` — starts an attempt. The paper
is sampled from the **course's own pool** — every active question with that
`course_id`, narrowed by the exam's `assessment_type`. The course is reached from
the exam (`exams -> blocks -> course_id`); the exam still anchors to a block and
still supplies `question_count` and `duration_minutes`, but neither the exam's
block nor a question's optional unit/module narrows the draw, because most
questions have no module and a module filter would silently shrink the pool.

The student must be actively enrolled in that course. That is not a second
check: the exam is resolved through a query that already joins
`student_courses`, so an exam in a course the student is not enrolled in selects
no row and answers `404` — the same answer a missing or unpublished exam gets.

The sample is written to `exam_attempt_answers` in the same transaction as the
attempt row, which is what makes a reload show the same questions in the same
order — "no two attempts identical" is a property across attempts, not within
one.

```json
{ "attempt_id": "uuid", "exam_id": "uuid", "exam_title": "string",
  "assessment_type": "mid_term_quiz",
  "duration_minutes": 45, "time_limit_minutes": 45,
  "attempt_no": 1, "started_at": "RFC3339", "total_questions": 10,
  "questions": [ { "question_seq": 1, "question_id": "uuid",
    "topic": "string", "question_text": "string",
    "options": ["A","B","C","D"], "selected_option_index": null } ] }
```

`topic` stays on the question even though a runner need not render it: the
weak-area roll-up is by topic, and a topic label is not part of the answer.

Failure modes:

- `404 NOT_FOUND` — the exam does not exist, is not published, or the caller is
  not enrolled in its course. All three are the same answer, deliberately.
- `422 EXAM_NOT_MCQ` — the exam declares no `question_count`/`assessment_type`,
  so there is no paper to sample. Semantically invalid, not an internal fault.
- `422 INSUFFICIENT_QUESTIONS` — the **course's** pool holds fewer active
  questions of that `assessment_type` than the paper asks for. The message names
  both numbers so a console can tell the centre how far short the pool is. The
  count is re-checked after sampling as well, so a question retired between the
  two never yields a short paper scored out of the full `max_score`.
- `409 EXAM_ATTEMPT_ACTIVE` — an `in_progress` attempt already exists. The body
  is the ordinary `{ code, message }` envelope and **carries no attempt id**:
  the envelope has no details field and adding one for a single route would
  break the rule that every client parses every error the same way. The client
  finds the live attempt in `GET /api/v1/student/exam-attempts` — which returns
  `in_progress` attempts with their `id` — and resumes it at
  `GET /api/v1/student/exams/attempts/{id}`. That coupling is the resume
  mechanism; neither half may be removed without the other.

`GET /api/v1/student/exams/attempts/{id}` — resume: the same answer-free
payload with whatever has been saved so far. A submitted, graded or abandoned
attempt is `409 EXAM_ATTEMPT_NOT_ACTIVE` rather than being re-served as a
paper; its review is the submit response. Another student's attempt id is
`404`, never `403` — a `403` would confirm the id exists.

`PATCH /api/v1/student/exams/attempts/{id}/answers` — save-as-you-go, and
idempotent, so a client may retry a dropped save without reasoning about
ordering.

```json
{ "answers": [ { "question_seq": 1, "selected_option_index": 2 } ] }
```

`selected_option_index` is `0..=3`, or `null` to clear an answer — `null` is
unanswered and `0` is a real choice of A, which are different things. The whole
batch is validated before any of it is written: a duplicated `question_seq`, an
index outside A-D, an empty batch, or more than 200 answers is
`400 VALIDATION_ERROR` on `answers`. A non-`in_progress` attempt is
`409 EXAM_ATTEMPT_NOT_ACTIVE`. The response is
`{ "attempt_id": "uuid", "saved": 10 }`; a `saved` below the number submitted
means a `question_seq` was not part of this paper.

`POST /api/v1/student/exams/attempts/{id}/submit` — grades server-side in one
transaction from the stored key. Nothing the client sent participates in the
arithmetic, and an unanswered question is simply incorrect. Submitting twice is
`409 EXAM_ATTEMPT_NOT_ACTIVE`, not a re-grade. Only now is the key disclosed:

```json
{ "attempt_id": "uuid", "exam_id": "uuid", "exam_title": "string",
  "total_questions": 10, "correct_answers": 8, "score": 8.0,
  "max_score": 10.0, "score_percentage": 80.0,
  "weak_topics": ["string"], "submitted_at": "RFC3339",
  "review": [ { "question_seq": 1, "question_id": "uuid", "topic": "string",
    "question_text": "string", "options": ["A","B","C","D"],
    "selected_option_index": 2, "correct_option_index": 1,
    "is_correct": false, "explanation": "string" } ] }
```

`weak_topics` is the distinct topics of the questions answered incorrectly.
`explanation` is `"string"`, never `null`: it is NOT NULL in the schema because
it drives the feedback screen.

There is **no** `sync_targets`, no async push, and no second gradebook store.
The admin gradebook (`GET /admin/exams/{exam_id}/attempts`) reads the same
`exam_attempts` rows this route writes, so a queue or a duplicate table would
only create a way for the two to disagree. Do not add one.

#### The marked answer sheet

`GET /api/v1/student/exam-attempts/{id}/review` — capability `ViewOwnExams`,
self-only. The student's own graded paper, re-readable.

The review previously existed **only** as the body of `POST .../submit`, so a
student could see their marked answers exactly once. Losing that response — a
reload, a dropped connection, coming back next week — lost the answer sheet
permanently, though every row behind it was still in the database.

It **re-reads; it does not re-grade.** `exam_papers::grade` (which performs the
marking `UPDATE`) is not on this path: the score comes from the attempt row that
submission already wrote, so downloading an answer sheet can never alter a mark,
however many times it is fetched.

`404` for an attempt that does not exist or belongs to another student — never
`403`, which would confirm the id exists. `409 EXAM_ATTEMPT_NOT_ACTIVE` for an
attempt that was not marked: `in_progress` (still being sat) and `abandoned`
(never graded). `graded_paper`'s SQL independently refuses an `in_progress`
attempt, so the key cannot reach a live paper even if the status check were
wrong.

The payload is the submit result plus the identity and syllabus position that
make it a document rather than a screen:

```json
{ "attempt_id": "uuid", "exam_id": "uuid", "exam_title": "string",
  "course_code": "string", "course_name": "string",
  "block_no": 1, "block_title": "string",
  "student_name": "string", "roll_number": "string", "attempt_no": 2,
  "total_questions": 5, "correct_answers": 1,
  "score": 1.0, "max_score": 5.0, "score_percentage": 20.0,
  "weak_topics": ["string"],
  "started_at": "RFC3339", "submitted_at": "RFC3339",
  "review": [ { "question_seq": 1, "question_id": "uuid", "topic": "string",
    "question_text": "string", "options": ["A","B","C","D"],
    "selected_option_index": 2, "correct_option_index": 1,
    "is_correct": false, "explanation": "string" } ] }
```

`student_name` and `roll_number` appear on **no other** `/api/v1/student/*`
payload. That is deliberate and is not a widening of disclosure: the route is
self-only, so the only identity it can print is the caller's own, and a sheet
with no name on it is not a record anybody can hand over. `submitted_at` is
never `null` here, because an unsubmitted attempt is refused.

`total_questions` and `correct_answers` are counted from the marked rows rather
than read from a column, so this response and `submit`'s cannot disagree.

The client renders it at `/exams/attempts/{id}/sheet` and downloads it through
the browser's print pipeline (`window.print()` + a print stylesheet), not a PDF
library — a question, option or explanation may be in Malayalam, every library
needs the font embedded to write it, and a download that drops the script half
the syllabus is taught in is worse than none.

### The student's syllabus: course -> block -> unit

Three read-only routes, capability `ViewOwnContext`, self-only. They are the
navigation into the classroom: a student picks a course, then a block, then an
uploaded **unit**, and that unit opens the board. Before these, `/classroom`
opened straight onto `students.current_block_id` and a student could study one
block with no way to reach the rest of their own course.

| Method | Path | Capability |
|---|---|---|
| GET | `/api/v1/student/courses` | `ViewOwnContext` — self-only |
| GET | `/api/v1/student/courses/{course_id}/blocks` | `ViewOwnContext` — self-only |
| GET | `/api/v1/student/blocks/{block_id}/units` | `ViewOwnContext` — self-only |
| GET | `/api/v1/student/units/{document_id}/thumbnail` | `ViewOwnContext` — self-only |

A "unit" is a row of `documents`. `documents` is the storage-side name (it has a
`sha256`, a `storage_key`, an ingestion status); "Unit 1" is what the student was
told to read, and what the titles say. Nothing is renamed in the schema, and
neither `storage_key` nor `sha256` goes on the wire.

Each query **starts from** `student_courses` with the caller's own id bound —
not "filters by". A course, block or unit outside the student's active
enrolments selects no row, so there is no result set for a later predicate to
widen. An id belonging to someone else's programme is therefore
indistinguishable from one that does not exist and answers `404`, never `403`.
Only `active` enrolments count: a `dropped` or `completed` one is a historical
record, not a licence to keep opening the classroom.

**Not paginated.** A student has a handful of courses, a course a handful of
blocks, a block a handful of units; these are navigation screens rendered whole,
not feeds. The admin lists over the same tables stay paginated, because an admin
sees every student's worth of them.

```json
// GET /student/courses
[ { "course_id": "uuid", "code": "string", "name": "string",
    "description": "string|null", "semester_number": 1, "semester_name": "string",
    "block_count": 3, "teachable_block_count": 2 } ]

// GET /student/courses/{course_id}/blocks
[ { "block_id": "uuid", "block_no": 1, "title": "string",
    "description": "string|null", "unit_count": 1, "ready_unit_count": 1 } ]

// GET /student/blocks/{block_id}/units
[ { "document_id": "uuid", "title": "string", "page_count": 71,
    "is_ready": true, "has_thumbnail": true } ]
```

#### Unit cover images

`GET /api/v1/student/units/{document_id}/thumbnail` answers **`image/webp`
bytes**, not JSON — the only route in this namespace that does. It is the
rasterised **first page** of the unit's PDF, at 192 px wide, served
`Cache-Control: private, max-age=86400` (private because the response is
scoped to one student's enrolments, so no shared cache may hand it on).

It is scoped exactly as the three listings above are: the query starts from
`student_courses` with the caller's own id bound, so a `document_id` outside
their active enrolments selects no row and answers `404` — indistinguishable
from an id that does not exist, and never `403`. `storage_key` and the
thumbnail's own path stay off the wire as always; what is served is a small
picture of page 1, never the PDF.

`has_thumbnail` on the unit list is what a client checks before asking.
**A cover is optional and its absence is normal**, for two reasons that a
client must not render as an error:

- Rendering needs a native pdfium library the ingest worker may not have
  (`crates/rag/src/thumbnail.rs`), and a deployment without it ingests exactly
  as before, leaving `documents.thumbnail_key` NULL.
- Documents uploaded before covers existed have none, and are not backfilled.

So `has_thumbnail: false` means "draw your own placeholder", and a `404` here
means the same thing — never "this unit is broken". A cover says nothing about
whether a unit can be taught from; `is_ready` remains the only field that
answers that, and an un-ingested unit is still listed, still marked, and still
gets a cover if one was rendered.

A browser cannot load this with `<img src>`: the session is an httpOnly
cookie and the gateway is a different origin, so an image load carries no
credentials and would arrive unauthenticated. Clients fetch the bytes with
credentials and render an object URL (`apps/web/src/components/classroom/UnitCover.tsx`).

`is_ready` is `status = 'embedded'` — the only state in which the unit has
vectors to retrieve. `ready_unit_count` and `teachable_block_count` roll the same
fact up. A unit that is not ready is **listed and marked, not hidden**: opening
it would make the tutor abstain on every question (NN-4 working correctly, but
indistinguishable from a broken classroom if you are the student), and "not ready
yet" is a truthful answer where a missing unit is not.

An empty list and "not yours" are different answers and must not both render as
an empty page, so a `course_id` the student is not enrolled in, and a `block_id`
outside their courses, are resolved to `404` rather than served as `[]`.

#### `session_init` takes the chosen unit

```jsonc
{ "type": "session_init", "block_id": "uuid", "document_id": "uuid", "resume": true }
```

`document_id` is **optional and additive** — a client that omits it gets the
whole block, exactly as before. It narrows retrieval and nothing else: the block
still decides what the session *is*, and the four mandatory Qdrant filters
(`program_id AND semester_no AND course_id AND block_no`) are applied whether it
is present or not. It can only ever shrink the search, never move it, so E-25 is
intact — this is a narrowing *inside* the four, not a fifth alternative to them.

The gateway checks the document belongs to the block before honouring it. A
document that does not is **ignored with a warning, not rejected**: the four
filters would still hold so nothing could leak, but the tutor would be searching
for a unit that cannot be in this block and would find nothing at all. Teaching
the whole block is the correct fallback, and failing a lesson over a stale
bookmark would not be.

### Student reports

| Method | Path | Capability |
|---|---|---|
| GET | `/api/v1/admin/student-reports` | `ManagePrograms`; scoped for a sub-admin |
| GET | `/api/v1/admin/students/{student_id}/report` | `ManagePrograms`, scoped via `student -> program_id` |

There is **no new store behind these**. Both read the same `exam_attempts` and
`exam_attempt_answers` rows a student's own submit writes, which is exactly why
the design has no `sync_targets`, no queue and no mirrored gradebook: a second
copy would only create a way for the admin's numbers and the student's to
disagree. `GET /admin/exams/{exam_id}/attempts` is unchanged and still served —
that is the per-exam view; this is the per-student one.

`time_spent_seconds` is **derived** (`submitted_at - started_at`), not stored. A
duration column would have to be kept true by every write path touching either
timestamp, and would drift the first time one was corrected.

Scoping follows `/admin/analytics` exactly: the caller's role decides *which
query runs*, never a filter over a platform-wide result. A sub-admin sees only
its `sub_admin_scopes` programs, reached by `exam_attempts -> students ->
program_id`, and a sub-admin with no scopes gets an empty array — the correct
answer, not a reason to fall back to the unscoped query. A student outside the
scope is `404`, never `403`, because a `403` would confirm the id exists.

#### `GET /api/v1/admin/student-reports`

Every attempt, newest first, paginated like every other admin list: bare array
body, `X-Total-Count`, `limit` 1-200 (default 50) and `offset` validated rather
than clamped. Filters: `program_id`, `semester_id`, `course_id`, `student_id`,
`exam_id`, `assessment_type`, `status`, `from`/`to` (RFC3339, on `started_at`),
and `q` (student name, roll number, exam title).

Each row is denormalised enough to render without a second request.

```json
[ { "attempt_id": "uuid", "student_id": "uuid", "student_name": "string",
    "roll_number": "string", "program_id": "uuid", "program_name": "string",
    "semester_number": 1, "course_id": "uuid", "course_code": "string",
    "course_name": "string", "exam_id": "uuid", "exam_title": "string",
    "assessment_type": "assignment|mid_term_quiz|semester_exam|null",
    "attempt_no": 1, "total_questions": 10, "answered_questions": 9,
    "correct_answers": 8, "score": 8.0, "max_score": 10.0,
    "percentage": 80.0, "time_spent_seconds": 412,
    "status": "in_progress|submitted|graded|abandoned",
    "started_at": "RFC3339", "submitted_at": "RFC3339|null",
    "weak_topics": ["string"] } ]
```

- `score`, `percentage` and `time_spent_seconds` are `null` until the attempt is
  graded or submitted — never `0`, which would be a real measured claim.
- `total_questions` is counted from the attempt's own stored paper, not from
  `exams.question_count`: that is the *intended* size, and a historic attempt may
  have been served a different one.
- `weak_topics` is empty while the attempt is unsubmitted. Before grading, which
  questions are wrong is part of the answer key, and an admin list is not an
  exception to that.
- `assessment_type` is `null` for an exam that is not an MCQ paper.

#### `GET /api/v1/admin/students/{student_id}/report`

One student's record: profile, per-course rollup, and weak topics aggregated
across every graded attempt, descending by how often each was missed.

```json
{ "student_id": "uuid", "student_name": "string", "roll_number": "string",
  "program_name": "string", "semester_number": 1, "lsc_code": "string|null",
  "attempts_total": 0, "attempts_graded": 0,
  "average_percentage": null, "best_percentage": null,
  "total_time_spent_seconds": 0,
  "by_course": [ { "course_id": "uuid", "course_code": "string",
    "course_name": "string", "attempts": 0, "average_percentage": null } ],
  "weak_topics": [ { "topic": "string", "missed_count": 0 } ] }
```

Averages are `null` with nothing to average, per the existing analytics rule;
counts and summed durations are genuinely `0`. `by_course` is grouped from
attempts, so a course the student has not sat has no line. `weak_topics` is at
most 20 rows and counts only answers that were actually marked wrong — a NULL
`is_correct` belongs to an ungraded attempt and must not invent weakness from an
abandoned paper.

### List filters

- `GET /admin/students` — `q` (full name, roll number, login email), `status`
  (filters the linked `users.status`; `students` has no status column), `limit`, `offset`.
- `GET /admin/users` — `q` (email), `role`, `status`, `limit`, `offset`.
- `GET /admin/programs` — `q` (code, name), `limit`, `offset`. Scope is applied in SQL
  before pagination.
- `GET /admin/lscs` — `q` (code, name), `limit`, `offset`.

## WebSocket

One connection per session at `/ws/session?token=...`. Binary frames are audio; text frames are
JSON control messages. Every control message has `type` and, where it belongs to a turn, `seq`.

### Client to server

```jsonc
{ "type": "session_init",  "block_id": "uuid", "resume": true }
{ "type": "activity",      "speaking": true }
{ "type": "board_ack",     "seq": 42 }
{ "type": "board_error",   "seq": 42, "reason": "render_failed" }
{ "type": "skip_recap" }
{ "type": "end_session" }
```

### Server to client

```jsonc
{ "type": "session_ready", "session_id": "uuid", "is_first_login": false,
  "context": { "program": "BA Malayalam", "semester": 1, "block_no": 3 },
  "quota_remaining_ms": 1200000 }
{ "type": "board_ops",     "seq": 42, "clear_first": false, "ops": [ ... ] }
{ "type": "turn_state",    "seq": 42, "chapter": "3", "topic": "...", "page": 57 }
{ "type": "turn_complete", "seq": 42, "interrupted": false }
{ "type": "transcript",    "source": "tutor", "text": "..." }
{ "type": "quota_warning", "remaining_ms": 120000 }
{ "type": "session_end",   "reason": "quota" }
{ "type": "error",         "code": "UPSTREAM_UNAVAILABLE", "message": "..." }
```

### Turn-taking is the client's

Gemini Live's automatic turn detection is **disabled**
(`realtimeInputConfig.automaticActivityDetection.disabled = true`). The client
runs the detector and declares boundaries with `activity`, and the gateway
forwards them upstream as `activityStart` / `activityEnd`.

That is not a preference. Neither automatic setting works in a browser playing
the tutor through speakers: with the microphone gated while the tutor speaks the
model never hears an interruption, and with it open the model hears its own voice
bleeding back, takes it for the student and interrupts itself — observed as every
tutor sentence cut off mid-word and the student's transcript arriving as
fragments. Only the client can tell the two apart, because only the client knows
what it is playing and how loudly that returns to the microphone.

Consequences a client must honour:

- Audio frames are sent **only between** `activity{speaking:true}` and
  `activity{speaking:false}`. Streaming outside a declared turn puts the tutor's
  own bleed into the model's ear, which is the failure above.
- `activity{speaking:true}` during a tutor turn **is** the interruption; there is
  no separate interrupt message. The client should also flush local playback at
  that moment, so the speakers go quiet without waiting for the round trip.
- The model answers on `activity{speaking:false}`, so it must not be sent until
  the client's VAD hangover has elapsed — sending it at the first gap between
  words is what makes the tutor answer half a sentence.

### Rules

- `turn_state` carries the citation for the turn and is taken from the **retrieved chunk
  payload**, never from model output (`.claude/rules/rag-pipeline.md`). It is sent after
  `board_ops` for that turn and before its audio. A turn that abstained (NN-4) has no citation
  and sends no `turn_state` at all, rather than repeating the previous turn's.
- `turn_complete` says a tutor turn ended upstream. `interrupted` is `true` **only** when the
  turn was genuinely cut short (the student barged in, or Live reported an interruption); the
  client flushes its playback jitter buffer on `true`. A normal turn end is `false` (the field
  may be omitted and defaults to `false`) — sending `true` as a blanket "turn is over" signal
  would clip the legitimately queued tail off the end of every explanation. It is advisory:
  the client also barges in locally on its own VAD without waiting for this frame, and the
  `SyncGate` remains the sole authority on when audio is *released* (NN-1 is unaffected either
  way).
- `transcript` is a caption, not a control frame — `source` is `"tutor"` or `"student"`. It
  carries no `seq` and is not subject to NN-1 ordering: it is a running record of speech, not a
  visual the audio must wait behind. Partial and incremental per side (the service emits it as
  speech is recognised), so the client accumulates rather than replaces.
- `seq` is monotonic per session and identifies a turn across board ops, audio, and traces.
- The server never sends audio for turn `N` before it has sent `board_ops` for turn `N`.
- Unknown message types are ignored, not fatal — forward compatibility.
- Close codes: `4001` unauthenticated, `4003` quota reached, `4008` idle timeout,
  `4009` safety termination.
