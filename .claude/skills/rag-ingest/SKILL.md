---
name: rag-ingest
description: Use when working on PDF ingestion, chunking, embedding, metadata tagging, or Qdrant upserts for Digi Guru curriculum content — including debugging bad chunks, missing citations, or cross-course retrieval leakage.
---

# RAG Ingestion

Governs everything between a sub-admin uploading a PDF and a chunk being retrievable.

## Pipeline (fixed order)

```
upload -> virus scan + sha256 dedupe -> object storage
  -> job enqueued (Postgres outbox -> workers/ingest)
  -> layout parse (pdfium; OCR fallback for scanned Malayalam)
  -> clean (strip headers, footers, page numbers, watermarks)
  -> paragraph chunk (never split a sentence or table)
  -> enrich (chapter, topic, page, para_index, lang)
  -> embed (dense gemini-embedding-001 + sparse BM25)
  -> upsert to Qdrant with the full metadata payload
  -> documents.status = 'embedded' + ingest report
```

## Hard requirements

- Every chunk carries `program_id`, `semester`, `block_no`, `document_id`, `chapter`, `topic`,
  `page`, `para_index`, `lang`. Reject the upsert if any is missing.
- Chunks are 250–450 tokens with one sentence of overlap; paragraphs under 80 tokens merge forward.
- Tables and verse blocks stay atomic, tagged `kind`.
- `sha256` dedupe — the same PDF must never double the corpus.
- Low OCR confidence sets `pending_review`; the document does not go live until approved.

## When debugging

| Symptom | Look at |
|---------|---------|
| Answer cites the wrong page | `page` enrichment; check the layout parser offset on the first page |
| Retrieval returns another semester | Missing Qdrant payload filter — check the retriever, not the index |
| Mid-sentence chunk | Chunker boundary logic; verify the sentence splitter handles Malayalam danda and abbreviations |
| Noisy context, high tokens | Header/footer stripping; print the cleaned text before chunking |
| Tutor abstains on in-syllabus questions | `ABSTAIN_THRESHOLD` too high, or the reranker starving good chunks |

## Always

After any change here, run `/project:eval` and report the before/after metrics. Ingestion changes
are the easiest way to silently degrade teaching quality.
