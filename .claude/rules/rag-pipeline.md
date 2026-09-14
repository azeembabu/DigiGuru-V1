# RAG Pipeline Rules

## Ingestion

1. Parse with layout awareness (pdfium + boxes); OCR fallback for scanned Malayalam pages.
2. Strip page furniture — headers, footers, page numbers, watermarks — before chunking.
3. Chunk **paragraph-level**. Never split a sentence or a table. Target 250–450 tokens with a
   one-sentence overlap; paragraphs under 80 tokens merge forward.
4. Tables and verse blocks stay atomic, tagged `kind: table|verse`.
5. Every chunk is tagged with `program_id`, `semester`, `block_no`, `document_id`, `chapter`,
   `topic`, `page`, `para_index`, `lang`. A chunk missing any of these is a bug — reject it at upsert.
6. Deduplicate documents by `sha256`; re-uploading the same PDF must not double the corpus.
7. Low OCR confidence marks the document `pending_review`; it does not go live until a sub-admin approves.
8. The worker also rasterises **page 1** to a small WebP cover (`documents.thumbnail_key`), for the
   student's unit list. It is done before the pipeline runs, so a unit that stalls at
   `pending_review` still has one, and it **never fails an ingestion**: a cover is decoration and
   has no bearing on what the tutor can teach from. It contributes nothing to the corpus, is not
   chunked, embedded or upserted, and costs no tokens.

   Rendering needs pdfium, a native library this build does not vendor — the same reason
   `parse.rs` has no OCR. It is therefore behind the `pdfium` cargo feature
   (`cargo run -p ingest-worker --features pdfium`), and the library is found via
   `PDFIUM_LIB_PATH`, then beside the binary, then the system path. Without it the worker logs
   once at startup and leaves every `thumbnail_key` NULL, which the client renders as a
   generated placeholder tile.

## Retrieval

Fixed order, no shortcuts:

```
normalise + detect language
  -> hybrid search (dense + BM25, RRF fusion), top_k = 20
     MANDATORY filter: program_id AND semester_no AND course_id AND block_no
  -> cross-encoder rerank -> top 2..3
  -> if best score < ABSTAIN_THRESHOLD -> Abstain
  -> prompt = [cached preamble] + [2..3 chunks] + [turn state]
```

- **Never** issue an unfiltered Qdrant query. Cross-course leakage is a correctness bug, not a
  relevance issue.
- Retrieval returns the payload citation (`chapter`, `topic`, `page`) alongside the text; the tutor
  cites from that payload, never from its own memory.
- `Abstain` is a first-class return value, not an empty vector. The caller must handle it explicitly.
- `Abstain` means *the textbook does not cover this*, and that fact must reach the student in
  those words. Since 2026-09-14 it no longer means the turn produces nothing: the tutor may go
  on to help from general academic knowledge, but only on request and only as declared
  supplementary content (NN-4). The retrieval contract is unchanged — abstention is still
  decided by the similarity floor, never by the model.

## Cost discipline

- The static preamble (tutor instructions + block outline) is prompt-cached per block; the handle
  lives in `pcache:{block_id}`.
- Never send more than 3 chunks to the model.
- Polite phrases and recap-skip intents are handled by the local intent matcher — no retrieval,
  no LLM call.
- Log tokens per turn. A change that raises median tokens per turn needs a justification in the PR.

## Evaluation

Any change to the chunker, embedder, filter, reranker, threshold, or system prompt requires a
RAGAs run against the golden sets. Report context precision, context recall, faithfulness, and
answer relevancy before and after.
