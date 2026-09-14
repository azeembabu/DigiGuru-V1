//! Dense embedding (`IMPLEMENTATION_PLAN.md` §5.1 "EMBED": dense via
//! `gemini-embedding-001`, sparse via Qdrant-native BM25 — sparse is
//! Qdrant's own concern at upsert time, not this module's).
//!
//! `gemini-embedding-001` produces 3072-dim vectors (§3.2), so every
//! `Embedder` implementation here returns exactly that width.

use crate::error::Result;

/// Dense-vector width for `gemini-embedding-001` (`IMPLEMENTATION_PLAN.md` §3.2).
pub const EMBEDDING_DIM: usize = 3072;

#[async_trait::async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// Embed many texts, in order.
    ///
    /// The default implementation is the obvious loop, so every existing
    /// implementor keeps working unchanged. [`GeminiEmbedder`] overrides it
    /// with the real batch endpoint, and the difference is not a micro
    /// optimisation: the free tier caps `embedContent` at **1000 requests per
    /// day per project**, and a one-request-per-chunk ingest spends that on a
    /// few hundred chunks. Rebuilding this project's own corpus exhausted the
    /// day's quota partway through and left retrieval with nothing. Batching
    /// turns the same corpus into a handful of requests.
    ///
    /// Returns one vector per input, in the same order. An implementation that
    /// cannot honour that must fail rather than return a short vector: the
    /// caller zips these against the chunks positionally, so a missing entry
    /// would silently attach the wrong text to the wrong embedding.
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut out = Vec::with_capacity(texts.len());
        for text in texts {
            out.push(self.embed(text).await?);
        }
        Ok(out)
    }
}

/// Deterministic pseudo-embedding for pipeline plumbing/tests. Hashes the
/// input text into a seeded PRNG and fills a 3072-dim vector — not a real
/// embedding, but stable across calls for the same text, which is enough to
/// exercise chunk -> embed -> Qdrant upsert end-to-end without a live
/// Gemini API key.
pub struct StubEmbedder;

#[async_trait::async_trait]
impl Embedder for StubEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        use rand::{Rng, SeedableRng};

        let seed = seed_from_text(text);
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let vector: Vec<f32> = (0..EMBEDDING_DIM)
            .map(|_| rng.gen_range(-1.0f32..1.0f32))
            .collect();
        Ok(vector)
    }
}

/// A simple, dependency-free string hash (FNV-1a) used only to seed the
/// stub's PRNG — not a cryptographic hash, and not meant to be one.
fn seed_from_text(text: &str) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Which side of the asymmetry a vector is for.
///
/// `gemini-embedding-001` is an **asymmetric** model: a passage and a question
/// about that passage are embedded into deliberately different regions, and the
/// task type is how you say which you are producing. Using the same task type
/// for both is the single easiest way to quietly halve retrieval quality — the
/// vectors still have the right width, the search still returns results, and
/// nothing anywhere reports an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbedTask {
    /// Ingestion: a curriculum chunk being stored.
    Document,
    /// Retrieval: a student's question being matched against stored chunks.
    Query,
}

impl EmbedTask {
    fn as_api_value(self) -> &'static str {
        match self {
            EmbedTask::Document => "RETRIEVAL_DOCUMENT",
            EmbedTask::Query => "RETRIEVAL_QUERY",
        }
    }
}

/// Real `gemini-embedding-001` client.
///
/// Verified against the live API: a 3072-dimension vector comes back for a
/// single `content.parts[].text`, which is exactly [`EMBEDDING_DIM`] and the
/// width `IMPLEMENTATION_PLAN.md` §3.2 gives the Qdrant collection. The width is
/// asserted on every response rather than trusted, because a silently narrower
/// vector would be rejected by Qdrant at upsert time with an error pointing at
/// the collection rather than at this call.
pub struct GeminiEmbedder {
    pub api_key: String,
    task: EmbedTask,
    http: reqwest::Client,
}

impl GeminiEmbedder {
    /// An embedder for the ingestion side (stored curriculum chunks).
    pub fn for_documents(api_key: impl Into<String>) -> Result<Self> {
        Self::new(api_key, EmbedTask::Document)
    }

    /// An embedder for the retrieval side (a student's question).
    pub fn for_queries(api_key: impl Into<String>) -> Result<Self> {
        Self::new(api_key, EmbedTask::Query)
    }

    fn new(api_key: impl Into<String>, task: EmbedTask) -> Result<Self> {
        // A bounded timeout because this call sits inside the per-turn retrieval
        // budget (`.claude/rules/realtime-audio.md` allows 300 ms for retrieval +
        // rerank). It cannot be allowed to hang a student's turn indefinitely; a
        // timeout surfaces as an embed error, which the caller turns into an
        // abstention rather than a stalled lesson.
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|err| crate::error::RagError::Embed(err.to_string()))?;

        Ok(Self {
            api_key: api_key.into(),
            task,
            http,
        })
    }
}

/// Max texts per `batchEmbedContents` call. The API accepts up to 100.
const BATCH_MAX: usize = 100;

const BATCH_EMBED_ENDPOINT: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-embedding-001:batchEmbedContents";

const EMBED_ENDPOINT: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-embedding-001:embedContent";

#[async_trait::async_trait]
impl Embedder for GeminiEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        use crate::error::RagError;

        // An empty input is a caller bug, and the API would reject it anyway.
        // Catching it here keeps the error local instead of arriving as an
        // opaque 400 from a remote service.
        if text.trim().is_empty() {
            return Err(RagError::Embed("refusing to embed empty text".to_string()));
        }

        let body = serde_json::json!({
            "model": "models/gemini-embedding-001",
            "content": { "parts": [{ "text": text }] },
            "taskType": self.task.as_api_value(),
            "outputDimensionality": EMBEDDING_DIM,
        });

        // Retried here, per chunk, rather than left to the ingest worker.
        //
        // The worker's unit of retry is the whole DOCUMENT: one failed chunk
        // discards every embedding already paid for and computed for that
        // document, and burns one of its three attempts. The endpoint returns a
        // transient 503 often enough that a long document would then never
        // finish — observed rebuilding this corpus, where the first document
        // failed on a single 503 after embedding most of its chunks. A few
        // seconds of backoff here is far cheaper than re-embedding a document,
        // and it is the difference between a rebuild that converges and one
        // that exhausts its attempts.
        //
        // Only transient statuses are retried. A 400 (bad request) or 403 (bad
        // key) will fail identically forever, and retrying it wastes the
        // attempt budget that a genuinely transient failure needs.
        const MAX_TRIES: u32 = 5;
        let mut last: String = String::new();

        for attempt in 1..=MAX_TRIES {
            if attempt > 1 {
                // 0.5s, 1s, 2s, 4s. Bounded and short: this sits inside an
                // ingest job, not a student's turn.
                let backoff = std::time::Duration::from_millis(250 * (1 << attempt.min(5)));
                tokio::time::sleep(backoff).await;
            }

            let response = match self
                .http
                .post(EMBED_ENDPOINT)
                .query(&[("key", self.api_key.as_str())])
                .json(&body)
                .send()
                .await
            {
                Ok(response) => response,
                Err(err) => {
                    // A transport error (timeout, reset connection) is exactly
                    // the kind of thing another try fixes.
                    last = format!("embedding request failed: {err}");
                    tracing::warn!(attempt, error = %last, "embedding request failed; retrying");
                    continue;
                }
            };

            let status = response.status();
            if status.is_success() {
                let parsed: EmbedResponse = response.json().await.map_err(|err| {
                    RagError::Embed(format!("embedding response was not JSON: {err}"))
                })?;

                let vector = parsed.embedding.values;
                if vector.len() != EMBEDDING_DIM {
                    return Err(RagError::Embed(format!(
                        "embedding width {} does not match the collection's {EMBEDDING_DIM}",
                        vector.len()
                    )));
                }
                return Ok(vector);
            }

            let detail = response.text().await.unwrap_or_default();
            // Truncated: an upstream error body can be long, and this string ends
            // up in a log line, not in front of a student.
            let detail: String = detail.chars().take(300).collect();
            last = format!("embedding endpoint returned {status}: {detail}");

            // 429 and 5xx are the transient ones. Everything else is a
            // permanent answer and is returned immediately.
            let transient = status.as_u16() == 429 || status.is_server_error();
            if !transient {
                return Err(RagError::Embed(last));
            }
            tracing::warn!(attempt, %status, "embedding endpoint is unavailable; retrying");
        }

        Err(RagError::Embed(format!(
            "embedding failed after {MAX_TRIES} attempts: {last}"
        )))
    }

    /// Batched embedding via `batchEmbedContents`.
    ///
    /// One HTTP request carries up to [`BATCH_MAX`] texts, which is what keeps
    /// a corpus rebuild inside the free tier's 1000-requests-per-day cap: the
    /// per-chunk path spends one request per chunk and runs out partway
    /// through a textbook.
    ///
    /// The response's `embeddings` array is positional — entry `i` is the
    /// vector for request `i` — and the whole pipeline relies on that, because
    /// the caller zips these against the chunks by index. A response of the
    /// wrong length is therefore a hard error, never a short result: silently
    /// returning fewer vectors would pair each chunk with a later chunk's
    /// embedding and poison retrieval in a way nothing downstream could detect.
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        use crate::error::RagError;

        if texts.is_empty() {
            return Ok(Vec::new());
        }
        if texts.iter().any(|t| t.trim().is_empty()) {
            return Err(RagError::Embed("refusing to embed empty text".to_string()));
        }

        let mut out: Vec<Vec<f32>> = Vec::with_capacity(texts.len());

        for group in texts.chunks(BATCH_MAX) {
            let requests: Vec<serde_json::Value> = group
                .iter()
                .map(|text| {
                    serde_json::json!({
                        "model": "models/gemini-embedding-001",
                        "content": { "parts": [{ "text": text }] },
                        "taskType": self.task.as_api_value(),
                        "outputDimensionality": EMBEDDING_DIM,
                    })
                })
                .collect();
            let body = serde_json::json!({ "requests": requests });

            // Same transient-only retry policy as the single path; see `embed`.
            const MAX_TRIES: u32 = 5;
            let mut last = String::new();
            let mut vectors: Option<Vec<Vec<f32>>> = None;

            for attempt in 1..=MAX_TRIES {
                if attempt > 1 {
                    let backoff = std::time::Duration::from_millis(250 * (1 << attempt.min(5)));
                    tokio::time::sleep(backoff).await;
                }

                let response = match self
                    .http
                    .post(BATCH_EMBED_ENDPOINT)
                    .query(&[("key", self.api_key.as_str())])
                    .json(&body)
                    .send()
                    .await
                {
                    Ok(response) => response,
                    Err(err) => {
                        last = format!("batch embedding request failed: {err}");
                        tracing::warn!(attempt, error = %last, "batch embedding failed; retrying");
                        continue;
                    }
                };

                let status = response.status();
                if status.is_success() {
                    let parsed: BatchEmbedResponse = response.json().await.map_err(|err| {
                        RagError::Embed(format!("batch embedding response was not JSON: {err}"))
                    })?;
                    if parsed.embeddings.len() != group.len() {
                        return Err(RagError::Embed(format!(
                            "batch embedding returned {} vectors for {} inputs",
                            parsed.embeddings.len(),
                            group.len()
                        )));
                    }
                    let mut got = Vec::with_capacity(group.len());
                    for embedding in parsed.embeddings {
                        if embedding.values.len() != EMBEDDING_DIM {
                            return Err(RagError::Embed(format!(
                                "embedding width {} does not match the collection's {EMBEDDING_DIM}",
                                embedding.values.len()
                            )));
                        }
                        got.push(embedding.values);
                    }
                    vectors = Some(got);
                    break;
                }

                let detail = response.text().await.unwrap_or_default();
                let detail: String = detail.chars().take(300).collect();
                last = format!("batch embedding endpoint returned {status}: {detail}");
                let transient = status.as_u16() == 429 || status.is_server_error();
                if !transient {
                    return Err(RagError::Embed(last));
                }
                tracing::warn!(attempt, %status, "batch embedding endpoint unavailable; retrying");
            }

            match vectors {
                Some(v) => out.extend(v),
                None => {
                    return Err(RagError::Embed(format!(
                        "batch embedding failed after {MAX_TRIES} attempts: {last}"
                    )))
                }
            }
        }

        Ok(out)
    }
}

#[derive(serde::Deserialize)]
struct BatchEmbedResponse {
    embeddings: Vec<EmbeddingValues>,
}

#[derive(serde::Deserialize)]
struct EmbedResponse {
    embedding: EmbeddingValues,
}

#[derive(serde::Deserialize)]
struct EmbeddingValues {
    values: Vec<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stub_embedder_produces_correct_dimension() {
        let embedder = StubEmbedder;
        let vector = embedder.embed("some chunk text").await.unwrap();
        assert_eq!(vector.len(), EMBEDDING_DIM);
    }

    #[test]
    fn query_and_document_use_different_task_types() {
        // The asymmetry is the whole point: if these ever collapse to one value,
        // retrieval quality degrades with no error anywhere.
        assert_eq!(EmbedTask::Document.as_api_value(), "RETRIEVAL_DOCUMENT");
        assert_eq!(EmbedTask::Query.as_api_value(), "RETRIEVAL_QUERY");
        assert_ne!(
            EmbedTask::Document.as_api_value(),
            EmbedTask::Query.as_api_value()
        );
    }

    #[tokio::test]
    async fn empty_text_is_refused_locally_rather_than_sent_upstream() {
        let embedder = GeminiEmbedder::for_queries("unused-in-this-test")
            .expect("client builds");
        let err = embedder.embed("   ").await.expect_err("empty text is refused");
        assert!(err.to_string().contains("empty"), "got {err}");
    }

    #[tokio::test]
    async fn stub_embedder_is_deterministic() {
        let embedder = StubEmbedder;
        let a = embedder.embed("repeatable text").await.unwrap();
        let b = embedder.embed("repeatable text").await.unwrap();
        assert_eq!(a, b);
    }

    #[tokio::test]
    async fn stub_embedder_differs_for_different_text() {
        let embedder = StubEmbedder;
        let a = embedder.embed("text one").await.unwrap();
        let b = embedder.embed("text two").await.unwrap();
        assert_ne!(a, b);
    }
}
