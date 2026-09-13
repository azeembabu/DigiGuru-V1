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
{ "type": "quota_warning", "remaining_ms": 120000 }
{ "type": "session_end",   "reason": "quota" }
{ "type": "error",         "code": "UPSTREAM_UNAVAILABLE", "message": "..." }
```

### Rules

- `seq` is monotonic per session and identifies a turn across board ops, audio, and traces.
- The server never sends audio for turn `N` before it has sent `board_ops` for turn `N`.
- Unknown message types are ignored, not fatal — forward compatibility.
- Close codes: `4001` unauthenticated, `4003` quota reached, `4008` idle timeout,
  `4009` safety termination.
