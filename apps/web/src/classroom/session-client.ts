/**
 * The classroom session client: one WebSocket, its lifecycle, and the NN-1 ACK.
 *
 * Ordering is the whole job. For every `board_ops` frame:
 *
 *   1. render the ops and wait for the paint (`BoardRenderer.apply`),
 *   2. only then send `board_ack`.
 *
 * The gateway is holding the turn's audio until that ACK arrives or its 400 ms
 * ceiling expires, so this ordering is what makes "whiteboard before voice" true
 * rather than merely intended. ACKing on receipt instead — the obvious
 * simplification — would leave `acked_ms` looking excellent while the student
 * heard narration before the board appeared. That is the bug this file exists to
 * prevent, and it is why the ACK is awaited on the render promise and nothing
 * else.
 *
 * If rendering throws, we send `board_error` instead. The gateway then releases
 * the audio and switches to a text fallback: per `whiteboard-sync.md`, "a canvas
 * exception must never kill the voice stream".
 *
 * Reconnect follows `.claude/rules/realtime-audio.md`: exponential backoff with
 * jitter, 250 ms -> 8 s, at most 6 attempts, and `resume: true` on re-init so the
 * session continues rather than restarting. Close codes the server chose
 * deliberately (quota, safety, idle, auth) are never retried.
 */

import { BoardRenderer } from "./board-renderer";
import {
  CLOSE_NORMAL,
  clientMessage,
  closeReason,
  parseServerMessage,
  shouldReconnect,
} from "./protocol";
import type { BoardOpsMessage, SessionReady, Transcript, TurnComplete, TurnState } from "./protocol";

export const HEARTBEAT_MS = 15_000;
export const MAX_RECONNECT_ATTEMPTS = 6;
const BACKOFF_MIN_MS = 250;
const BACKOFF_MAX_MS = 8_000;

export type ConnectionState =
  | "idle"
  | "connecting"
  | "ready"
  | "reconnecting"
  | "closed"
  | "failed";

export interface SessionClientHandlers {
  onState: (state: ConnectionState, detail?: string) => void;
  onSessionReady: (msg: SessionReady) => void;
  onTurnState?: (msg: TurnState) => void;
  onTranscript?: (msg: Transcript) => void;
  /**
   * A tutor turn ended upstream. `msg.interrupted` distinguishes a turn that was
   * cut short from one that finished: only the former may flush queued playback.
   */
  onTurnComplete?: (msg: TurnComplete) => void;
  /**
   * The tutor's remaining audio must be dropped right now.
   *
   * Raised for an interrupted turn, for a server-sent `session_end`, and for a
   * terminal close — every case where further playback would be the student
   * hearing a turn that is over. The caller wires this to
   * `AudioPlayback.stop()`; this module deliberately does not own the audio
   * clock (`code-style.md`: the classroom logic stays framework- and
   * device-agnostic where it can).
   */
  onFlushAudio?: () => void;
  /**
   * The board ops for a turn, raised as they arrive and before they are
   * rendered.
   *
   * Purely observational — it exists so the classroom can keep the op-log for
   * note export (`pedagogy.md`: a note is a client-side render of the op-log,
   * not a server artifact). It must stay synchronous and cheap: it runs inside
   * the NN-1 hold window, and anything slow here delays the ACK.
   */
  onBoardOps?: (msg: BoardOpsMessage) => void;
  onQuotaWarning?: (remainingMs: number) => void;
  onSessionEnd?: (reason: string) => void;
  onError?: (code: string, message: string) => void;
  /** Inbound tutor audio, already PCM16 bytes off the wire. */
  onAudio?: (frame: ArrayBuffer) => void;
  /** Measured locally: paint time for a turn, so the UI can show ACK latency. */
  onAckLatency?: (seq: number, ms: number) => void;
}

export interface SessionClientOptions {
  /** Base WS URL, e.g. `ws://localhost:8080/ws/session`. */
  url: string;
  blockId: string;
  /** The unit chosen within the block; narrows retrieval, nothing else. */
  documentId?: string | null;
  renderer: BoardRenderer;
  handlers: SessionClientHandlers;
  /**
   * Mints a fresh single-use ticket. Called for every connection attempt,
   * including reconnects, because a ticket is spent on redemption and expires in
   * 30 seconds — caching one would guarantee a failed reconnect.
   */
  mintTicket: () => Promise<string>;
}

export class SessionClient {
  private socket: WebSocket | null = null;
  private heartbeat: ReturnType<typeof setInterval> | null = null;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private attempts = 0;
  /** Set once a session has existed, so a reconnect asks to resume it. */
  private hasSession = false;
  /** True when the caller asked to stop; suppresses all reconnect logic. */
  private closingDeliberately = false;
  /**
   * Serialises turn rendering. Two `board_ops` frames arriving back to back must
   * not interleave their paints, or turn N+1 could be ACKed before turn N is on
   * screen — which would release N's audio against an incomplete board.
   */
  private renderChain: Promise<void> = Promise.resolve();

  constructor(private readonly options: SessionClientOptions) {}

  get state(): ConnectionState {
    if (!this.socket) return this.closingDeliberately ? "closed" : "idle";
    switch (this.socket.readyState) {
      case WebSocket.CONNECTING:
        return "connecting";
      case WebSocket.OPEN:
        return "ready";
      default:
        return "closed";
    }
  }

  async connect(): Promise<void> {
    this.closingDeliberately = false;
    await this.openSocket();
  }

  private async openSocket(): Promise<void> {
    this.options.handlers.onState(this.attempts > 0 ? "reconnecting" : "connecting");

    let ticket: string;
    try {
      ticket = await this.options.mintTicket();
    } catch (error) {
      // No ticket, no socket. Treated as a retryable transport failure: the
      // usual cause is a transient 503 from the gateway, not a bad credential.
      this.scheduleReconnect(
        error instanceof Error ? error.message : "Could not authenticate the session.",
      );
      return;
    }

    const url = `${this.options.url}?token=${encodeURIComponent(ticket)}`;
    const socket = new WebSocket(url);
    socket.binaryType = "arraybuffer";
    this.socket = socket;

    socket.onopen = () => {
      this.attempts = 0;
      this.startHeartbeat();
      socket.send(
        clientMessage.sessionInit(
          this.options.blockId,
          this.hasSession,
          this.options.documentId ?? null,
        ),
      );
    };

    socket.onmessage = (event: MessageEvent<string | ArrayBuffer>) => {
      if (typeof event.data === "string") {
        this.handleControlFrame(event.data);
      } else {
        // Binary frames are tutor audio. They only ever arrive after the board
        // for their turn was released, because the gateway buffered them.
        this.options.handlers.onAudio?.(event.data);
      }
    };

    socket.onerror = () => {
      // `onerror` carries no useful detail in browsers; `onclose` follows with
      // the code, which is what decides whether to retry.
      this.options.handlers.onState("reconnecting", "Connection problem.");
    };

    socket.onclose = (event: CloseEvent) => {
      this.stopHeartbeat();
      this.socket = null;

      if (this.closingDeliberately) {
        this.options.handlers.onState("closed", "Session ended.");
        return;
      }
      if (!shouldReconnect(event.code)) {
        // A deliberate server decision. Surfaced as terminal, not retried.
        this.options.handlers.onState("failed", closeReason(event.code));
        this.options.handlers.onFlushAudio?.();
        this.options.handlers.onSessionEnd?.(closeReason(event.code));
        return;
      }
      this.scheduleReconnect(closeReason(event.code));
    };
  }

  private handleControlFrame(raw: string): void {
    const msg = parseServerMessage(raw);
    // Unknown or malformed frames are ignored, never fatal — the documented
    // forward-compatibility rule.
    if (!msg) return;

    switch (msg.type) {
      case "session_ready":
        this.hasSession = true;
        this.options.handlers.onState("ready");
        this.options.handlers.onSessionReady(msg);
        break;
      case "board_ops":
        this.options.handlers.onBoardOps?.(msg);
        this.renderTurn(msg);
        break;
      case "turn_state":
        this.options.handlers.onTurnState?.(msg);
        break;
      case "transcript":
        this.options.handlers.onTranscript?.(msg);
        break;
      case "turn_complete":
        this.options.handlers.onTurnComplete?.(msg);
        // NN-1 is unaffected: the gate releases audio, this only discards audio
        // already released for a turn that has since been abandoned.
        if (msg.interrupted) this.options.handlers.onFlushAudio?.();
        break;
      case "quota_warning":
        this.options.handlers.onQuotaWarning?.(msg.remaining_ms);
        break;
      case "session_end":
        // The server is ending this on purpose, so teardown must not reconnect.
        this.closingDeliberately = true;
        this.options.handlers.onFlushAudio?.();
        this.options.handlers.onSessionEnd?.(msg.reason);
        break;
      case "error":
        this.options.handlers.onError?.(msg.code, msg.message);
        break;
    }
  }

  /**
   * Renders a turn then ACKs it, with turns strictly serialised.
   *
   * The ACK is sent from inside the chain so its ordering relative to other
   * turns is guaranteed by the same mechanism that orders the paints.
   */
  private renderTurn(msg: BoardOpsMessage): void {
    this.renderChain = this.renderChain.then(async () => {
      const started = performance.now();
      try {
        await this.options.renderer.apply(msg.ops, msg.clear_first);
      } catch (error) {
        const reason = error instanceof Error ? error.message : "render_failed";
        this.send(clientMessage.boardError(msg.seq, reason.slice(0, 120)));
        return;
      }
      const elapsed = performance.now() - started;
      this.send(clientMessage.boardAck(msg.seq));
      this.options.handlers.onAckLatency?.(msg.seq, elapsed);
    });
  }

  /** Sends a student audio frame, if the socket is open. Dropped otherwise. */
  sendAudio(frame: Int16Array): void {
    if (this.socket?.readyState !== WebSocket.OPEN) return;
    // A copy of exactly this frame's bytes: the Int16Array may be a view onto a
    // larger reused buffer, and sending the whole buffer would send stale audio.
    this.socket.send(
      frame.buffer.slice(frame.byteOffset, frame.byteOffset + frame.byteLength) as ArrayBuffer,
    );
  }

  /**
   * Called by the capture path when the student starts speaking.
   *
   * Barge-in is decided locally because it has to be: waiting for the upstream
   * interruption signal to round-trip means the tutor keeps talking over the
   * student for the whole of that round trip. It only discards *playback* —
   * capture and the socket are untouched, so the gateway remains the authority
   * on turn boundaries.
   */
  interrupt(): void {
    this.options.handlers.onFlushAudio?.();
  }

  skipRecap(): void {
    this.send(clientMessage.skipRecap());
  }

  private send(text: string): void {
    if (this.socket?.readyState === WebSocket.OPEN) this.socket.send(text);
  }

  private startHeartbeat(): void {
    this.stopHeartbeat();
    // The browser WebSocket API cannot send a protocol-level ping, so liveness
    // relies on the server's pings and on this cheap keepalive frame. The server
    // ignores unknown control frames by contract, so `skip_recap` is not abused
    // here — an empty-but-valid frame would be, which is why this sends nothing
    // and instead only checks the socket is still open.
    this.heartbeat = setInterval(() => {
      if (this.socket?.readyState !== WebSocket.OPEN) return;
      // Reading `bufferedAmount` detects a stalled send queue: if it keeps
      // growing the connection is wedged even though readyState says OPEN.
      if (this.socket.bufferedAmount > 1_000_000) {
        this.options.handlers.onState("reconnecting", "The connection stalled.");
        this.socket.close();
      }
    }, HEARTBEAT_MS);
  }

  private stopHeartbeat(): void {
    if (this.heartbeat) {
      clearInterval(this.heartbeat);
      this.heartbeat = null;
    }
  }

  private scheduleReconnect(detail: string): void {
    if (this.closingDeliberately) return;

    if (this.attempts >= MAX_RECONNECT_ATTEMPTS) {
      this.options.handlers.onState(
        "failed",
        `${detail} Reconnection gave up after ${MAX_RECONNECT_ATTEMPTS} attempts.`,
      );
      return;
    }

    this.attempts += 1;
    // Exponential with full jitter: without jitter, 200 sockets dropped by one
    // gateway restart all retry on the same millisecond and knock it over again.
    const ceiling = Math.min(BACKOFF_MAX_MS, BACKOFF_MIN_MS * 2 ** (this.attempts - 1));
    const delay = BACKOFF_MIN_MS + Math.random() * (ceiling - BACKOFF_MIN_MS);

    this.options.handlers.onState("reconnecting", detail);
    this.reconnectTimer = setTimeout(() => {
      void this.openSocket();
    }, delay);
  }

  /**
   * Explicit teardown. Required on navigation, tab close and unmount —
   * `realtime-audio.md`: "a leaked socket burns quota and API budget".
   */
  close(): void {
    this.closingDeliberately = true;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.stopHeartbeat();
    if (this.socket?.readyState === WebSocket.OPEN) {
      this.send(clientMessage.endSession());
      this.socket.close(CLOSE_NORMAL, "client teardown");
    } else {
      this.socket?.close();
    }
    this.socket = null;
    this.options.handlers.onState("closed", "Session ended.");
  }
}
