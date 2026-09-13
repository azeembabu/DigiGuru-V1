/**
 * The classroom WebSocket contract, as Zod schemas.
 *
 * Mirrors `.claude/rules/api-conventions.md` "WebSocket" and the gateway's
 * `apps/gateway/src/ws/mod.rs`. Parsing every inbound control frame rather than
 * casting it is the point: a frame that does not match is *ignored*, which is
 * the documented forward-compatibility rule ("unknown message types are
 * ignored, not fatal"). A cast would instead let a renamed field arrive as
 * `undefined` and surface three layers later as a blank board.
 *
 * This module is framework-agnostic on purpose (`.claude/rules/code-style.md`:
 * audio and canvas code "stays framework-agnostic where it can be — it should
 * be unit-testable without React"). Nothing here imports React or touches the
 * DOM.
 */

import { z } from "zod";

/** Board coordinates are normalised 0..1 so the board renders identically at any canvas size. */
export const pointSchema = z.object({ x: z.number(), y: z.number() });

/**
 * One validated board op.
 *
 * Tagged `kind`, matching `live::BoardOp` in `crates/live/src/ops.rs` — NOT the
 * model-facing `live::board::BoardOp`, which is tagged `op`. The gateway sends
 * the former. Getting this wrong renders an empty board with no error, so the
 * discriminator is asserted by a test.
 */
/** One labelled quantity in a chart. */
export const dataPointSchema = z.object({ label: z.string(), value: z.number() });

export const boardOpSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("heading"), id: z.string(), text: z.string() }),
  z.object({ kind: z.literal("bullets"), id: z.string(), items: z.array(z.string()) }),
  z.object({ kind: z.literal("math"), id: z.string(), latex: z.string() }),
  z.object({
    kind: z.literal("draw"),
    id: z.string(),
    shape: z.string(),
    points: z.array(pointSchema),
  }),
  z.object({ kind: z.literal("image"), id: z.string(), reference: z.string() }),
  // `highlight` targets the `id` of an element emitted earlier and carries no
  // `id` of its own — it is a reference, not a new element.
  z.object({ kind: z.literal("highlight"), target: z.string() }),
  // Infographics. The gateway has already rejected empty, negative,
  // non-finite and all-zero series, so the renderer can trust the numbers and
  // concentrate on drawing them.
  z.object({
    kind: z.literal("bar_chart"),
    id: z.string(),
    title: z.string(),
    series: z.array(dataPointSchema),
  }),
  z.object({
    kind: z.literal("pie_chart"),
    id: z.string(),
    title: z.string(),
    series: z.array(dataPointSchema),
  }),
  z.object({
    kind: z.literal("flow"),
    id: z.string(),
    title: z.string(),
    steps: z.array(z.string()),
  }),
]);

export type BoardOp = z.infer<typeof boardOpSchema>;

export const sessionReadySchema = z.object({
  type: z.literal("session_ready"),
  session_id: z.string(),
  is_first_login: z.boolean(),
  context: z.record(z.string(), z.unknown()),
  quota_remaining_ms: z.number(),
});

export const boardOpsSchema = z.object({
  type: z.literal("board_ops"),
  seq: z.number(),
  clear_first: z.boolean(),
  ops: z.array(boardOpSchema),
});

export const turnStateSchema = z.object({
  type: z.literal("turn_state"),
  seq: z.number(),
  chapter: z.string().nullish(),
  topic: z.string().nullish(),
  page: z.number().nullish(),
});

/**
 * The tutor's turn finished upstream.
 *
 * `interrupted: true` means the turn was cut short — the student barged in, or
 * Gemini Live itself reported an interruption — and anything still sitting in the
 * playback jitter buffer is from a turn that is no longer happening. A normal
 * completion (`false`, the default when the field is absent) must NOT flush:
 * the tail of the turn is legitimately still queued and dropping it would clip
 * the last word of every explanation.
 */
export const turnCompleteSchema = z.object({
  type: z.literal("turn_complete"),
  seq: z.number().nullish(),
  interrupted: z.boolean().default(false),
});

/**
 * A caption — not a control frame. Carries no `seq` and is not subject to NN-1
 * ordering (`api-conventions.md`): it is a running record of speech, not a
 * visual the audio must wait behind. Partial and incremental per side, so the
 * UI accumulates rather than replaces.
 */
export const transcriptSchema = z.object({
  type: z.literal("transcript"),
  source: z.enum(["tutor", "student"]),
  text: z.string(),
});

export const quotaWarningSchema = z.object({
  type: z.literal("quota_warning"),
  remaining_ms: z.number(),
});

export const sessionEndSchema = z.object({
  type: z.literal("session_end"),
  reason: z.string(),
});

export const errorSchema = z.object({
  type: z.literal("error"),
  code: z.string(),
  message: z.string(),
});

export const serverMessageSchema = z.discriminatedUnion("type", [
  sessionReadySchema,
  boardOpsSchema,
  turnStateSchema,
  turnCompleteSchema,
  transcriptSchema,
  quotaWarningSchema,
  sessionEndSchema,
  errorSchema,
]);

export type ServerMessage = z.infer<typeof serverMessageSchema>;
export type SessionReady = z.infer<typeof sessionReadySchema>;
export type BoardOpsMessage = z.infer<typeof boardOpsSchema>;
export type TurnState = z.infer<typeof turnStateSchema>;
export type Transcript = z.infer<typeof transcriptSchema>;
export type TurnComplete = z.infer<typeof turnCompleteSchema>;

/**
 * Parses an inbound text frame.
 *
 * Returns `null` for anything unrecognised instead of throwing: per the
 * forward-compatibility rule an unknown frame is ignored, and a throw here would
 * tear down a working lesson because the server added a field.
 */
export function parseServerMessage(raw: string): ServerMessage | null {
  let json: unknown;
  try {
    json = JSON.parse(raw);
  } catch {
    return null;
  }
  const result = serverMessageSchema.safeParse(json);
  return result.success ? result.data : null;
}

/** Client -> server frames. Built here so no call site hand-writes the wire shape. */
export const clientMessage = {
  // `document_id` is the unit the student chose inside the block. Omitted when
  // there is none, so the frame stays exactly what shipped for a student who
  // opened a block rather than a unit.
  sessionInit: (blockId: string, resume: boolean, documentId?: string | null) =>
    JSON.stringify({
      type: "session_init",
      block_id: blockId,
      resume,
      ...(documentId ? { document_id: documentId } : {}),
    }),
  boardAck: (seq: number) => JSON.stringify({ type: "board_ack", seq }),
  boardError: (seq: number, reason: string) =>
    JSON.stringify({ type: "board_error", seq, reason }),
  skipRecap: () => JSON.stringify({ type: "skip_recap" }),
  endSession: () => JSON.stringify({ type: "end_session" }),
} as const;

/**
 * Documented close codes (`api-conventions.md` "Close codes").
 *
 * These decide whether a reconnect is even appropriate: reconnecting after a
 * quota or safety termination would be the client trying to get around a
 * server-side decision, so `shouldReconnect` says no.
 */
export const CLOSE_UNAUTHENTICATED = 4001;
export const CLOSE_QUOTA_REACHED = 4003;
export const CLOSE_IDLE_TIMEOUT = 4008;
export const CLOSE_SAFETY_TERMINATION = 4009;

/** Normal client-initiated teardown. */
export const CLOSE_NORMAL = 1000;

export function shouldReconnect(code: number): boolean {
  switch (code) {
    // Every one of these is a decision the server made deliberately. Retrying
    // is either futile (the credential is bad) or an attempt to overturn it.
    case CLOSE_UNAUTHENTICATED:
    case CLOSE_QUOTA_REACHED:
    case CLOSE_IDLE_TIMEOUT:
    case CLOSE_SAFETY_TERMINATION:
    case CLOSE_NORMAL:
      return false;
    default:
      return true;
  }
}

/** Human-readable reason for a terminal close, for the UI to show. */
export function closeReason(code: number): string {
  switch (code) {
    case CLOSE_UNAUTHENTICATED:
      return "Your session could not be authenticated. Please sign in again.";
    case CLOSE_QUOTA_REACHED:
      return "You have used today's 20 minutes of voice tutoring. It resets at midnight your time.";
    case CLOSE_IDLE_TIMEOUT:
      return "The session ended because it was idle.";
    case CLOSE_SAFETY_TERMINATION:
      return "The session was ended for safety reasons.";
    case CLOSE_NORMAL:
      return "Session ended.";
    default:
      return "The connection dropped.";
  }
}
