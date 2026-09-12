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
