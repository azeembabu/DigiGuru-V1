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
`409` state conflict (e.g. `SESSION_ACTIVE`) · `422` semantically invalid · `429` rate limited ·
`503` upstream (Gemini/Qdrant) unavailable.

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
