---
name: whiteboard-sync
description: Use when working on the Gemini Live audio stream, the SyncGate, board ops, canvas rendering, or any bug where audio and whiteboard visuals are out of order or out of sync.
---

# Whiteboard-First Sync

The product rule: **the whiteboard visual must render before or simultaneously with the audio that
explains it.** This is NN-1 and it is enforced mechanically in the gateway.

## How enforcement works

The model has a `board_ops` tool and is instructed to call it before narrating. The gateway does not
trust that — it buffers:

```
board_ops(seq=N) from model
  -> forward control frame to client, start HOLD_MAX (400 ms), turn N = PENDING_ACK
  -> audio frames for turn N are BUFFERED

board_ack(seq=N) from client  -> release audio, turn N = OPEN
HOLD_MAX expiry               -> release audio, wb_violation += 1, log turn N
board_error(seq=N)            -> release audio, text-fallback board, lesson continues
```

The buffer is the guarantee. Never "fix" a sync bug by relaxing the buffer.

## Invariants to preserve

- `wb_violation` is 0 in CI e2e. Non-zero blocks release.
- A canvas exception must not kill the voice stream — error boundary reports `board_error`.
- Every op and ACK latency is persisted to `board_events`.
- `clear_first: true` on block, chapter, and session boundaries.

## Debugging order

1. Read the trace for the failing `seq` — the spans are
   `ws.turn → guardrail → rag.retrieve → rag.rerank → live.request → board.emit → board.ack → audio.release`.
2. Check `board_events.acked_ms` for that turn. NULL means the client never ACKed — a client bug.
3. High ACK latency means canvas work on the main thread — batch into one `requestAnimationFrame`.
4. If the model narrated without calling `board_ops` at all, that is a prompt problem; the gate
   still held the audio, so there is no NN-1 violation — but the lesson had no visual, which is its
   own defect worth fixing in the system instruction.

## Canvas

Fabric.js over an offscreen buffer, batched rAF commits, 60 FPS target on low-spec mobile.
Malayalam via subset Noto Sans Malayalam — verify conjunct shaping after any font change.
Board state archives as a JSON op-log, never an image.
