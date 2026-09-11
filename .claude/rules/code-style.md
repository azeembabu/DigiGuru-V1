# Code Style

## Rust

- Edition 2021, `rustfmt` defaults, `clippy -D warnings` is a merge gate.
- Errors: `thiserror` for library crates, `anyhow` only in binaries. Every crate exposes its own
  `Error` enum; the gateway maps them to a single `PublicError` before serialisation.
- **No `unwrap()` / `expect()` / `panic!` in the request or audio path.** Allowed in tests, build
  scripts, and in `main` during startup where a failure should abort the process.
- Async: `tokio` multi-threaded runtime. Never block a worker — CPU-bound work (embedding,
  reranking, ASR) goes through `spawn_blocking` or a dedicated rayon pool.
- Database: `sqlx` macros (`query_as!`) so queries are checked at compile time. No string-built SQL.
- Newtypes over primitives for ids: `StudentId(Uuid)`, `BlockId(Uuid)`, `TurnSeq(u32)`.
- Public functions in `crates/` carry doc comments; private helpers do not need them.

## TypeScript / Next.js

- `strict: true`. No `any` — use `unknown` plus a narrowing guard.
- Server Components by default; `"use client"` only where hooks, audio, or canvas require it.
- Data fetching in Server Components or route handlers; never call the gateway directly from a
  component that could render on the server without auth context.
- Zod schemas for every API boundary, shared between client and route handlers.
- Tailwind for styling; component-local CSS only for canvas overlays.
- Audio and canvas code lives in `apps/web/src/classroom/` and stays framework-agnostic where it can
  be — it should be unit-testable without React.

## General

- Match the surrounding file: naming, comment density, and idiom.
- Comment *why*, not *what*. A comment explaining a non-obvious protocol rule is valuable; a comment
  restating the line above is not.
- File length: prefer splitting over 500-line modules.
- No commented-out code in a merged PR.
