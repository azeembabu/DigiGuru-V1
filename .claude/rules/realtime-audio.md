# Real-Time Audio Rules

## Client capture

- `AudioWorklet`, 16 kHz mono PCM16, 20 ms frames. Never `ScriptProcessorNode`.
- Frames are aligned and buffered cleanly before upstream — no partial frames, no clipping.
- VAD with a tuned noise floor and a 300 ms hangover so ambient noise does not open a turn.
- Codec: Opus preferred, raw PCM16 fallback when Opus encoding is unavailable on the device.

## Connection

- Auto-reconnect with exponential backoff + jitter: 250 ms → 8 s, max 6 attempts.
- Session state lives in `sess:{session_id}` in Redis, so a reconnect **resumes** the session and
  does not start a new one. The gateway is stateless with respect to any single socket.
- Explicit teardown on navigation, tab close, and component unmount. A leaked socket burns quota
  and API budget.
- Heartbeat ping every 15 s; a missed pong closes and triggers reconnect.

## Latency budget — p95 speech-end to audio-start < 1.5 s

| Segment | Budget |
|---------|--------|
| VAD end-of-speech | 300 ms |
| Upstream network | 60 ms |
| Guardrail tier 0 | 20 ms |
| Retrieval + rerank | 300 ms |
| Live first audio token | 500 ms |
| SyncGate board hold | ≤ 250 ms typical |
| Downstream + jitter buffer | 70 ms |

Each segment is its own histogram in the trace. A regression on any segment fails CI.

## Gateway

- One tokio task per socket; CPU-bound work off the audio path via `spawn_blocking`.
- No allocation in the per-frame hot loop where it can be avoided — reuse buffers.
- Backpressure: if the downstream client cannot keep up, drop the oldest buffered audio rather than
  growing unbounded, and log it.
- Load target: 200 concurrent sockets per node with no task starvation.
