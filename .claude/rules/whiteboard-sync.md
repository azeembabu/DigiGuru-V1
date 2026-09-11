# Whiteboard Sync Rules (NN-1)

**The rule:** audio for a turn must never reach the student before the whiteboard visual for that
turn. This is enforced in the gateway, not requested in the prompt — the model cannot outrun the
board because its audio is physically buffered.

## SyncGate state machine

```
board_ops(seq=N) received from model
  -> forward control frame to client
  -> mark turn N PENDING_ACK, start HOLD_MAX timer (400 ms)
  -> audio frames for turn N are BUFFERED, not forwarded

client board_ack(seq=N)   -> release buffered audio, turn N OPEN
HOLD_MAX expires          -> release audio, wb_violation += 1, log turn N
client board_error(seq=N) -> release audio, switch to text-fallback board, keep teaching
```

## Invariants

- `wb_violation` must be `0` in CI end-to-end runs. A non-zero count is a release blocker.
- A canvas exception must never kill the voice stream. The renderer is wrapped in an error boundary
  that reports `board_error` and degrades to text-only.
- Every op and its ACK latency is written to `board_events` for audit and replay.
- `clear_first: true` on a block, chapter, or session boundary — the board never carries stale
  content between topics.

## Canvas

- Fabric.js over an offscreen buffer; batch commits in a single `requestAnimationFrame`, so a burst
  of ops is one paint. Target 60 FPS on low-spec mobile.
- Malayalam rendered with a subset Noto Sans Malayalam webfont; verify glyph shaping for conjuncts.
- Board state is archived as a JSON op-log, never as an image — replay is reconstruction.

## Op schema

`heading` · `bullets` · `math` (LaTeX) · `draw` (shape primitives) · `image` (`doc:UUID#pNN-figM`)
· `highlight` (targets a previously emitted element).

Ops are validated against the schema at the gateway before forwarding. An invalid op is dropped and
logged; it never reaches the client.
