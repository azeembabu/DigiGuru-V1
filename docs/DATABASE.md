# Setting up the database

Everything a contributor needs to reach a working local database is in this repository.
Follow this file top to bottom on a fresh clone and you end up with the same schema, the
same catalogue and a working exam engine as everybody else.

## Why there is no `.sql` dump in the repo

A `pg_dump` of a working database is the obvious thing to commit and the wrong thing to
commit, for three separate reasons:

1. **It publishes credentials and PII.** `users` holds argon2id password hashes and
   `students` holds names, roll numbers and login emails. `.claude/rules/security.md`
   forbids PII anywhere it could be exported, and a git repository is the most exportable
   place there is. A dump also cannot be un-published once pushed.
2. **It drifts from the schema.** `migrations/` is forward-only and reviewed. A dump is a
   second, unreviewed description of the same schema, and the moment the two disagree the
   dump wins on one machine and the migrations win on another — which is exactly the class
   of bug that is hardest to trace.
3. **It is not reviewable.** Nobody reads a 40 MB `INSERT` in a pull request, so a change
   to the data arrives with no review at all.

The schema is shared through `migrations/`, and the data through the two seeders below.
Both are code, both are reviewed, and both produce the same result on every machine.

## Prerequisites

Docker Desktop, the Rust toolchain, and `sqlx-cli`:

```bash
cargo install sqlx-cli --no-default-features --features rustls,postgres
```

## 1. Start the local services

```bash
docker compose -f infra/docker-compose.yml up -d
```

Postgres on `localhost:5432`, Redis on `6379`, Qdrant on `6333` (REST) and `6334` (gRPC).
`./dev.ps1` does this for you and then runs the migrations and both processes.

Set `DATABASE_URL` in your `.env` to match the compose file. `.env` is gitignored and must
stay that way — no committed secrets, ever.

## 2. Apply the migrations

```bash
sqlx migrate run
```

Forward-only, and **never edit an applied migration**. If a checksum mismatch stops the
run, the file changed after being applied somewhere: fix it by adding a new migration, not
by editing the old one.

`sqlx`'s compile-time query macros check every query against a live database, so this step
has to succeed before `cargo check` will.

## 3. Create the first admin

Nothing in `migrations/` inserts an admin. A committed credential is a published
credential, so the bootstrap is a deliberate, interactive step:

```bash
cargo run -p gateway --bin create_admin -- \
  --email you@example.com --full-name "Your Name" --role super_admin
```

It prompts for the password twice and never takes it as an argument, so it cannot land in
shell history. Run this **before** the seeder: `documents.uploaded_by`,
`question_pool.created_by` and `exams.created_by` are all foreign keys to a real user, and
the seeder skips those stages with a printed warning rather than inventing an account with
a known password.

A scoped sub-admin, so the scoped-vs-platform-wide split is exercised rather than assumed:

```bash
cargo run -p gateway --bin create_admin -- \
  --email subadmin@digiguru.local --full-name "Sub Admin" --role sub_admin --scope <PROGRAM_UUID>
```

## 4. Seed

Two seeders, doing different jobs. Most contributors want the second.

### `migrations/seed/0001_seed_dev.sql` — the fixed catalogue

One LSC, one program (BA Malayalam), six semesters, one course each, four blocks each, all
with **hard-coded UUIDs**. That is the point: tests and fixtures can refer to
`00000000-…-0002` and mean the same program on every machine.

```bash
psql "$DATABASE_URL" -f migrations/seed/0001_seed_dev.sql
```

Not idempotent — run it once against a freshly migrated database.

### `seed_dev` — the working dataset

```bash
cargo run -p gateway --bin seed_dev
```

Idempotent: every stage owns its rows and replaces them, so re-running refreshes rather
than doubles. It refuses to run against a `DATABASE_URL` that does not look local unless
you pass `--force`.

| Stage | What it creates |
|---|---|
| `catalogue` | A program, semesters, courses, blocks, LSCs, students, enrolments, and documents in every ingestion state |
| `activity` | ~30 days of `learning_sessions`, `board_events` (with a realistic NN-1 ACK-latency tail), `safety_incidents`, `note_reminders` |
| `assessment` | The MCQ `question_pool` and the published `exams` that sample it |

Options: `--students N` (default 40), `--days N` (default 30),
`--scope-admin <EMAIL>`, `--student-password <P>` (default `SeedStudent-Dev-1!`).

The `assessment` stage matters more than it sounds. An exam samples the **course's** pool
and answers `422 INSUFFICIENT_QUESTIONS` when the pool holds fewer active questions of the
paper's `assessment_type` than `question_count` asks for. Without a seeded pool the exam
engine looks broken on a fresh clone: every paper fails to start. It seeds 12 questions per
course per assessment type and one published exam of each type per course, which is
comfortably above what any seeded paper draws.

## 5. Verify

```bash
# The student can see published exams and start one.
curl -s -X POST localhost:8080/api/v1/auth/login -H 'Content-Type: application/json' \
  -d '{"email":"seed.student000@digiguru.local","password":"SeedStudent-Dev-1!"}' -i \
  | grep -i set-cookie
curl -s localhost:8080/api/v1/student/exams -H "Cookie: dg_access=..."
```

A non-empty list, and `POST /student/exams/{id}/attempts` returning a paper with
`total_questions` matching the exam's `question_count`, means the database is set up
correctly.

## Rebuilding the retrieval corpus (Qdrant)

The vector corpus is derived data: every point in the `curriculum` collection
comes from a PDF in `data/uploads/` plus the row describing it in `documents`.
Nothing is lost by rebuilding it, and it **must** be rebuilt whenever the
parser, the chunker, the embedder or its task type changes — the stored vectors
were produced by the code as it was that day, and a query embedded by newer
code is not comparable to them.

### Prerequisite: `pdftotext` must be on PATH

Extraction uses `pdftotext` (poppler-utils). `PopplerParser::available()` is
checked at worker start:

```bash
pdftotext -v          # any version banner is fine
# Debian/Ubuntu: sudo apt install poppler-utils
# macOS:         brew install poppler
# Windows:       it ships with Git for Windows in /mingw64/bin
```

Without it the worker falls back to `LopdfParser` and **says so at WARN**. That
fallback cannot decode Identity-H/CID fonts: it writes the literal string
`?Identity-H Unimplemented?` into the corpus in place of the text. Measured on
this project's own corpus before the switch, that was **55% of chunks and 32%
of every stored character**, and retrieval served those placeholders to the
tutor as CURRICULUM CONTEXT — which is why the tutor reported that topics were
not in the textbook. Never ship a corpus built by the fallback.

### Rebuild

```bash
# 1. Drop the collection. The worker recreates it with the right vector
#    dimensions and payload indexes on first upsert.
curl -X DELETE http://localhost:6333/collections/curriculum

# 2. Requeue every document. `documents.status` goes back to `pending` and a
#    fresh `ingestion_jobs` row is enqueued for each.
docker exec digiguru-postgres psql -U digiguru -d digiguru -c "
  UPDATE documents SET status='pending';
  INSERT INTO ingestion_jobs (document_id, status, attempts, last_error)
  SELECT id, 'pending', 0, NULL FROM documents;"

# 3. Drain the queue. GEMINI_API_KEY must be set, or the worker embeds with the
#    deterministic stub and the corpus is not semantically searchable.
GEMINI_API_KEY=... cargo run -p ingest-worker --bin ingest-worker
```

The worker exits when the queue is empty. Re-running it is safe: a document is
re-chunked and re-upserted by `document_id`, so a partial run is resumed rather
than duplicated.

### Verify before trusting it

```bash
# Points exist, and none of them are extraction placeholders.
curl -s http://localhost:6333/collections/curriculum \
  | python -c "import sys,json;print(json.load(sys.stdin)['result']['points_count'])"
```

Then check a real query end to end — retrieval that returns junk still returns
*something*, so a point count alone proves nothing. Ask the classroom a
question whose answer you can see in the PDF, and confirm the tutor cites the
chapter and page rather than saying the topic is not in the textbook.

### The embedding quota is the binding constraint

Embedding is batched (`batchEmbedContents`, up to 100 texts per request), so a
full rebuild of this corpus costs a handful of requests rather than one per
chunk. That matters because the free tier caps embedding at **1000 requests per
day per project** (`EmbedContentRequestsPerDayPerProjectPerModel-FreeTier`).

A per-chunk ingest spends that budget on a few hundred chunks. It is how a
rebuild here ran out of quota partway through, with the old collection already
deleted and nothing to serve — so:

**Check quota before dropping the collection.** One embedding call is enough:
a `429 RESOURCE_EXHAUSTED` naming `embed_content_free_tier_requests` means the
day is spent and no retry or backoff will help; it resets on Google's daily
boundary. Better still, ingest into a fresh collection and repoint retrieval
only once it is populated, so a failed rebuild costs nothing.
### Documents that cannot be extracted

A PDF that is a pure scan yields no text under any extractor. Those pages are
recorded with `ocr_confidence = 0.0` and the document is held at
`pending_review` rather than `embedded`, so it is never silently treated as
indexed. There is no OCR engine in this build: such a file must be re-uploaded
as a text PDF, or OCR'd first. `SELECT title FROM documents WHERE status =
'pending_review';` lists them.

## What is *not* reproducible from this repository

Be aware of these before assuming a fresh clone is identical to a working machine:

- **The RAG corpus.** Qdrant vectors come from ingesting a real PDF, and neither the PDF
  (copyright) nor the vectors (large, and derived) are in the repository. Until an admin
  uploads a document to a block and the ingest worker embeds it, the tutor abstains on
  everything — which is NN-4 behaving correctly, not a bug. Upload through
  `POST /api/v1/admin/blocks/{block_id}/documents`.
- **`GEMINI_API_KEY`.** Not in the repo and never will be. Without it the gateway logs
  `no GEMINI_API_KEY configured` and classroom sessions run a scripted stub client, which
  is useful for UI work and useless for tutoring.
- **The NN-3 quota ledger.** Redis, per student per local day. It is deliberately not
  seeded; every student starts a fresh day with the full 20 minutes.

## Changing the schema

1. `git pull origin main --rebase` first, so your migration number does not collide.
2. Add a new file `migrations/00NN_what_it_does.sql`. Never edit an applied one.
3. `sqlx migrate run`, then `cargo check --workspace` — the query macros will catch every
   query the change broke, at compile time.
4. If the change adds a table the seeder should populate, add it to the right `seed_dev`
   stage in the same commit. A schema that is shared but unseedable puts the next
   contributor back where this document started.
