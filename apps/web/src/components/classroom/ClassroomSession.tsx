"use client";

/**
 * The live classroom surface.
 *
 * `"use client"` is unavoidable here: the canvas, the mic, the audio clock and
 * the WebSocket all need the browser. The logic they wrap does not, which is why
 * it lives in `src/classroom/` and this file is only wiring
 * (`.claude/rules/code-style.md`).
 *
 * The NN-1 ordering is not implemented here — `SessionClient` owns it. This
 * component's only contribution to it is mounting the canvas before connecting,
 * so the first turn's ops have something to paint onto.
 */

import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import Link from "next/link";

import { API_BASE_URL, apiFetch } from "@/lib/api";
import { AudioCapture } from "@/classroom/audio-capture";
import { AudioPlayback } from "@/classroom/audio-playback";
import { BoardRenderer, HOLD_MAX_MS } from "@/classroom/board-renderer";
import { SessionClient } from "@/classroom/session-client";
import { FRAME_SAMPLES } from "@/classroom/vad";
import type { ConnectionState } from "@/classroom/session-client";
import type {
  BoardOp,
  BoardOpsMessage,
  SessionReady,
  Transcript,
  TurnState,
} from "@/classroom/protocol";
import { buttonClass } from "@/components/ui/Button";
import { Logo } from "@/components/ui/Logo";

/**
 * Turn-taking is decided **here**, not by the service.
 *
 * Gemini's automatic detection cannot work in a browser playing the tutor
 * through speakers, and both settings of it failed in practice: with the
 * microphone gated the model could never hear an interruption, and with it open
 * the model heard its own voice bleeding back, took it for the student, and
 * interrupted itself — every tutor sentence cut off mid-word, the student's
 * transcript arriving as disconnected fragments.
 *
 * Only this side has what is needed to tell the two apart: it knows exactly what
 * it is playing and how loudly that comes back. So the client runs the detector
 * and declares turns with `activity` frames, and audio is sent only inside a
 * declared turn — so bleed never reaches the model at all, and a real
 * interruption is an explicit signal rather than something inferred from a
 * waveform.
 */

/**
 * Mic level must clear this multiple of the measured bleed to count as the
 * student.
 *
 * Deliberately close to 1. The first version demanded 3x, which in practice
 * meant the student had to raise their voice to interrupt at all — reported as
 * "I need to be loud to ask anything". Speaker bleed reaching a laptop
 * microphone is already well below a person speaking normally at it, so a
 * modest margin is enough to separate them; the work of rejecting noise is done
 * by `SPEECH_OPEN_FRAMES` below, which costs nothing in volume.
 */
const SPEECH_OVER_BLEED = 1.8;

/** Absolute floor for that test, so near-silence cannot qualify as speech. */
const SPEECH_MIN_LEVEL = 0.012;

/**
 * Consecutive qualifying frames before a turn is declared.
 *
 * Four frames is 80 ms — long enough to reject a single-frame spike from a key
 * press or a click, short enough to feel immediate. This is the right axis to
 * be strict on: being *consistent* for 80 ms is something ordinary speech does
 * and a transient does not, whereas being *loud* is something only a raised
 * voice does.
 */
const SPEECH_OPEN_FRAMES = 4;

/**
 * Frames of bleed measured before the detector will open a turn.
 *
 * The estimate starts at zero, so without this the tutor's first loud moment
 * clears the threshold and opens a turn on the tutor's own voice. Shortened to
 * 200 ms so the opening of a tutor sentence is interruptible too.
 */
const BLEED_WARMUP_FRAMES = 10;

/**
 * Silence, in frames, before a declared turn is closed.
 *
 * The turn used to close on the VAD's own `speech_end`, which never arrived
 * while the tutor's bleed was keeping the VAD open — so `activityEnd` was never
 * sent and the model, which answers on that signal, never answered at all.
 * Reported as "it is listening every time, no response from Gemini".
 *
 * So the close is driven by the same qualification test as the open: frames
 * that are not the student. 700 ms is comfortably longer than the gaps inside a
 * sentence and short enough not to feel like a wait.
 */
const SPEECH_CLOSE_FRAMES = 35;

/**
 * Queued tutor audio above which the header shows "tutor speaking".
 *
 * A display threshold only — it no longer gates anything. `queuedSeconds`
 * reads a tiny positive number for a moment as a turn ends, and treating that
 * as speech briefly is harmless where flapping on `> 0` would not be.
 */
const TUTOR_SPEAKING_GATE_S = 0.05;

/**
 * How long the bleed guard stays armed after the tutor's audio queue drains.
 *
 * `queuedSeconds` reaching zero means the last frame has been *handed to the
 * output*, not that the room has gone quiet: the speaker is still emitting that
 * tail and a live room still rings for a moment after it. Dropping the guard on
 * the queue alone left exactly that window unprotected — the tail came back
 * through the microphone with nothing left to compare it against, opened a
 * turn, and barged in on a tutor that had only just stopped talking.
 */
const BLEED_TAIL_MS = 400;

/**
 * How long "hearing you" stays lit after the qualified turn closes. The
 * indicator is an instrument, not a live meter: holding it across the gaps
 * inside a sentence is the honest reading of "yes, I can hear you".
 */
const HEARING_HOLD_MS = 400;

/** `subscribeNever` has no store to subscribe to; `document.body` never changes. */
function subscribeNever(): () => void {
  return () => {};
}

/** `http(s)://host/api/v1` -> `ws(s)://host/ws/session`. */
function webSocketUrl(): string {
  const base = new URL(API_BASE_URL);
  base.protocol = base.protocol === "https:" ? "wss:" : "ws:";
  base.pathname = "/ws/session";
  base.search = "";
  return base.toString();
}

async function mintTicket(): Promise<string> {
  // Through `apiFetch`, not a bare `fetch`: the access JWT lives 15 minutes,
  // and a bare fetch gets a flat 401 once it lapses. The socket client treats
  // a mint failure as a retryable transport fault, so that 401 used to leave
  // the classroom stuck on "Reconnecting" forever with no path back — the
  // session was recoverable the whole time. `apiFetch` refreshes and retries,
  // and sends the student to sign in when the session is genuinely over.
  const body: unknown = await apiFetch<unknown>("/ws/ticket", { method: "POST" });
  if (
    typeof body !== "object" ||
    body === null ||
    !("ticket" in body) ||
    typeof (body as { ticket: unknown }).ticket !== "string"
  ) {
    throw new Error("The session ticket was malformed.");
  }
  return (body as { ticket: string }).ticket;
}

interface TurnLogEntry {
  seq: number;
  ackMs: number;
  violation: boolean;
}

/** One `board_ops` frame plus the citation that arrived with its turn. */
interface BoardLogEntry {
  /**
   * Unique per entry. `seq` identifies the *turn*, and a turn may emit more
   * than one `board_ops` frame, so `seq` is not a usable React key — two
   * frames from one turn collide and React drops or duplicates a note.
   */
  id: number;
  seq: number;
  ops: BoardOp[];
  turn: TurnState | null;
}

export function ClassroomSession({
  blockId,
  documentId = null,
  backHref = "/classroom",
}: {
  blockId: string;
  /** The unit chosen in `/classroom`; narrows retrieval within the block. */
  documentId?: string | null;
  /** Where the back arrow returns to — the unit list this session came from. */
  backHref?: string;
}) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const shellRef = useRef<HTMLDivElement | null>(null);
  const rendererRef = useRef<BoardRenderer | null>(null);
  const clientRef = useRef<SessionClient | null>(null);
  const captureRef = useRef<AudioCapture | null>(null);
  const playbackRef = useRef<AudioPlayback | null>(null);
  /** True between `activity(true)` and `activity(false)` — a declared turn. */
  const turnOpenRef = useRef(false);
  /** Consecutive qualifying frames — see `SPEECH_OPEN_FRAMES`. */
  const speechRunRef = useRef(0);
  /** Consecutive non-qualifying frames — see `SPEECH_CLOSE_FRAMES`. */
  const silenceRunRef = useRef(0);
  /** Running estimate of the tutor's bleed level, learned while it plays. */
  const bleedRef = useRef(0);
  /** Frames of bleed seen this tutor turn; the detector is deaf until warmed up. */
  const bleedFramesRef = useRef(0);

  /**
   * Live mirror of the VAD's speech state.
   *
   * Only the held `hearing` label reads it now — the barge-in that used to
   * depend on it is gone with the gate, because Gemini's own turn detection
   * does that job once the audio actually reaches it.
   */
  const speakingRef = useRef(false);
  /**
   * Re-entrancy latch for `start()`, set SYNCHRONOUSLY before its first
   * `await`.
   *
   * `clientRef.current` cannot do this job: it is only assigned after
   * `await playback.resume()`, so two calls arriving during that window both
   * saw `null`, both passed the guard, and both went on to open a socket.
   * That produced two live Gemini sessions on one page, each with its own
   * audio stream — heard as two tutors talking over each other. A ref
   * assigned before any suspension point closes the window; React state
   * cannot, because a `setState` is not visible to a second call in the same
   * tick.
   */
  const startingRef = useRef(false);

  const [state, setState] = useState<ConnectionState>("idle");
  const [detail, setDetail] = useState<string | null>(null);
  const [session, setSession] = useState<SessionReady | null>(null);
  const [turn, setTurn] = useState<TurnState | null>(null);
  const [turns, setTurns] = useState<TurnLogEntry[]>([]);
  const [micOn, setMicOn] = useState(false);
  const [micError, setMicError] = useState<string | null>(null);
  const [quotaMs, setQuotaMs] = useState<number | null>(null);
  const [ended, setEnded] = useState<string | null>(null);
  // Whether `start()` has run. Tracked in state rather than read off
  // `clientRef` during render: a ref is not a render input, so reading it here
  // would not re-render when it changed (and the lint rule rightly forbids it).
  const [started, setStarted] = useState(false);
  // True once the first turn has actually landed on the board. Distinct from
  // `started`: the socket can be open and the tutor still composing its
  // opening turn, and "the board is blank because nothing has been taught
  // yet" reads very differently from "the classroom is broken".
  const [boardReady, setBoardReady] = useState(false);
  /**
   * Mirrors `document.fullscreenElement` rather than tracking our own intent.
   *
   * The browser exits fullscreen on its own — Escape, a tab switch, a window
   * manager — and none of those routes call our handler. A flag we set
   * ourselves would then say "fullscreen" over a windowed page, and the button
   * would offer to expand something already collapsed.
   */
  const [fullscreen, setFullscreen] = useState(false);
  const [notesOpen, setNotesOpen] = useState(true);
  /**
   * Which board page is on screen, and how many there are.
   *
   * Mirrored from the renderer rather than counted here: the renderer decides
   * when a page is full, and a second opinion in React would drift from it.
   */
  const [board, setBoard] = useState<{ page: number; total: number }>({ page: 0, total: 1 });
  /**
   * The board op-log, kept for note export.
   *
   * `pedagogy.md`: an exported note is a client-side render of the ops, not a
   * server artifact — so there is nothing to fetch and nothing to keep in sync,
   * but it does mean the ops have to be retained here as they go past. Each
   * entry carries the citation that was current when it was written, because
   * that is what makes the export a study note rather than a wall of text.
   */
  const [boardLog, setBoardLog] = useState<BoardLogEntry[]>([]);
  const boardLogIdRef = useRef(0);
  // Captions. The Live service can repeat the last partial output chunk when
  // a turn ends (observed live), so consecutive identical entries from the
  // same speaker are collapsed rather than shown twice.
  const [captions, setCaptions] = useState<Transcript[]>([]);
  const notesScrollRef = useRef<HTMLDivElement | null>(null);
  // Whether the tutor currently has audio queued — drives both the "hearing
  // you" vs "tutor speaking" indicator and whether the manual Interrupt
  // button is shown. `AudioPlayback.queuedSeconds` is imperative, so this is
  // a light poll rather than an event; cheap, and simpler than plumbing a
  // callback through for a UI-only signal.
  const [tutorSpeaking, setTutorSpeaking] = useState(false);
  /** Drives the button's disabled state; `startingRef` is what actually guards. */
  const [starting, setStarting] = useState(false);
  /**
   * `speaking`, held on briefly after it drops.
   *
   * The VAD's hysteresis stops it flapping on a steady noise floor, but a real
   * sentence still has gaps between words shorter than the hangover, and the
   * raw signal switching off and on across them made the indicator blink while
   * the student was simply talking. The label is a reassurance, not an
   * instrument; holding it for a moment is the honest reading of "yes, I can
   * hear you".
   */
  /** A frame of digital silence, the size the capture emits; sent in place of
   *  echo so the upstream stream never breaks. Allocated once. */
  const silentFrameRef = useRef<Int16Array>(new Int16Array(FRAME_SAMPLES));
  /** Monotonic ms at which the tutor's audio queue last held anything. */
  const lastTutorAudioMsRef = useRef(0);
  const [hearing, setHearing] = useState(false);
  const hearingTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  /** Drive the indicator from the QUALIFIED turn (see `onFrame`), never from
   *  the raw VAD event. On is immediate; off is held so it does not blink. */
  const showHearing = useCallback((on: boolean) => {
    if (hearingTimer.current !== null) {
      clearTimeout(hearingTimer.current);
      hearingTimer.current = null;
    }
    if (on) {
      setHearing(true);
      return;
    }
    hearingTimer.current = setTimeout(() => {
      hearingTimer.current = null;
      setHearing(false);
    }, HEARING_HOLD_MS);
  }, []);

  useEffect(() => {
    // Reset happens in `leave()` (an event handler, not here) — an effect
    // body must only synchronise with the external system, never call
    // `setState` directly on entry.
    if (!started) return;
    const id = setInterval(() => {
      setTutorSpeaking((playbackRef.current?.queuedSeconds ?? 0) > TUTOR_SPEAKING_GATE_S);
    }, 150);
    return () => clearInterval(id);
  }, [started]);

  // Create the renderer once the canvas element exists, and keep it sized to its
  // container. A board sized once at mount is wrong the moment the window moves.
  useEffect(() => {
    const element = canvasRef.current;
    const shell = shellRef.current;
    if (!element || !shell) return;

    const renderer = new BoardRenderer(element, {
      onPageChange: (page: number, total: number) => setBoard({ page, total }),
      onOpError: (op: BoardOp, error: Error) => {
        // Logged, not shown: one unrenderable op is not worth interrupting a
        // lesson for, and the gateway already hears about it via board_error
        // when the whole turn fails.
        console.warn(`board op ${op.kind} failed to render:`, error.message);
      },
    });
    rendererRef.current = renderer;

    // Last-applied size, so an observation that reports the size we just set is
    // ignored. Without this, any rounding difference between the CSS box and the
    // pixel size Fabric writes back becomes an endless resize loop.
    let appliedWidth = 0;
    let appliedHeight = 0;
    const fit = () => {
      const rect = shell.getBoundingClientRect();
      const width = Math.max(320, Math.round(rect.width));
      const height = Math.max(240, Math.round(rect.height));
      if (width === appliedWidth && height === appliedHeight) return;
      appliedWidth = width;
      appliedHeight = height;
      renderer.resize(width, height);
    };
    fit();

    const observer = new ResizeObserver(fit);
    observer.observe(shell);

    return () => {
      observer.disconnect();
      renderer.dispose();
      rendererRef.current = null;
    };
  }, []);

  const stopMic = useCallback(async () => {
    await captureRef.current?.stop();
    captureRef.current = null;
    setMicOn(false);
    // The mic is off, so nothing is being heard — including the held display
    // value and any timer still waiting to clear it.
    speakingRef.current = false;
    // A turn left open would have the model waiting for an end that never
    // comes.
    if (turnOpenRef.current) {
      turnOpenRef.current = false;
      clientRef.current?.setActivity(false);
    }
    speechRunRef.current = 0;
    silenceRunRef.current = 0;
    bleedRef.current = 0;
    bleedFramesRef.current = 0;
    if (hearingTimer.current !== null) {
      clearTimeout(hearingTimer.current);
      hearingTimer.current = null;
    }
    setHearing(false);
  }, []);

  const startMic = useCallback(async () => {
    if (captureRef.current) return;
    setMicError(null);
    const capture = new AudioCapture({
      // Half-duplex on the send side: mic frames are withheld while the tutor
      // has audio queued (`AudioPlayback.queuedSeconds`), not just when it is
      // "the model's turn" — this is what fixes the reported doubled/flickering
      // voice.
      //
      // What was actually happening: mic capture ran continuously with
      // `echoCancellation: true`, but browser AEC is best-effort and commonly
      // fails to reference audio scheduled through raw Web Audio API nodes
      // (as opposed to an <audio>/<video> element) — especially over
      // speakers rather than headphones. The tutor's own voice leaked back
      // into the mic, went upstream as if the student had spoken, and Gemini
      // Live's own server-side barge-in detection interrupted itself on
      // hearing its own echo and started a new response — two overlapping
      // TTS streams from the model, heard locally as doubled, glitchy audio.
      // Withholding the send while the tutor is speaking means Gemini never
      // hears its own echo, so it never has a reason to self-interrupt.
      onFrame: (frame, level) => {
        const queued = playbackRef.current?.queuedSeconds ?? 0;
        if (queued > TUTOR_SPEAKING_GATE_S) {
          lastTutorAudioMsRef.current = performance.now();
        }
        // Sticky: the guard outlives the queue by `BLEED_TAIL_MS` so the tail
        // still coming out of the speaker is measured against the learned bleed
        // level rather than against nothing.
        const tutorAudible =
          queued > TUTOR_SPEAKING_GATE_S ||
          performance.now() - lastTutorAudioMsRef.current < BLEED_TAIL_MS;

        // While the tutor is audible, whatever the microphone hears is mostly
        // its own voice. Learn that level — but never from frames already loud
        // enough to be the student, or the student's voice teaches the detector
        // to ignore them.
        if (tutorAudible) {
          bleedFramesRef.current += 1;
          const warming = bleedFramesRef.current <= BLEED_WARMUP_FRAMES;
          const loud = level > Math.max(bleedRef.current * SPEECH_OVER_BLEED, SPEECH_MIN_LEVEL);
          if (warming) {
            // Converge fast, or the estimate is still near zero when the freeze
            // below starts — every frame then looks louder than "bleed", the
            // estimate never catches up, and the turn opens on the tutor's own
            // voice and stays open. A slow average here was exactly that bug.
            bleedRef.current = bleedRef.current * 0.7 + level * 0.3;
          } else if (!loud) {
            // Adapt slowly, and never from frames already loud enough to be the
            // student — otherwise their voice trains the detector to ignore it.
            bleedRef.current = bleedRef.current * 0.97 + level * 0.03;
          }
        } else {
          bleedFramesRef.current = 0;
          bleedRef.current = 0;
        }

        // The student is speaking if the VAD says so and — while the tutor is
        // audible — the signal is clearly louder than the bleed. When the tutor
        // is silent there is nothing to confuse it with, so the VAD alone
        // decides.
        // `SPEECH_MIN_LEVEL` is an ABSOLUTE floor and is applied in both cases.
        // It used to sit only inside the bleed branch, so whenever the tutor was
        // silent — most of a lesson — the floor was never consulted and the
        // VAD's own threshold decided alone. That threshold bottoms out at a
        // level a fan or a distant voice clears easily, which is how turns
        // opened with nobody speaking.
        const aboveFloor = level > SPEECH_MIN_LEVEL;
        const overBleed =
          aboveFloor &&
          (!tutorAudible ||
            (bleedFramesRef.current > BLEED_WARMUP_FRAMES &&
              level > Math.max(bleedRef.current * SPEECH_OVER_BLEED, SPEECH_MIN_LEVEL)));
        const qualifies = speakingRef.current && overBleed;
        speechRunRef.current = qualifies ? speechRunRef.current + 1 : 0;
        silenceRunRef.current = qualifies ? 0 : silenceRunRef.current + 1;

        // Closing is driven by the same test as opening, not by the VAD's own
        // end-of-speech — which the tutor's bleed can suppress indefinitely.
        if (turnOpenRef.current && silenceRunRef.current >= SPEECH_CLOSE_FRAMES) {
          turnOpenRef.current = false;
          clientRef.current?.setActivity(false);
          showHearing(false);
        }

        if (speechRunRef.current >= SPEECH_OPEN_FRAMES && !turnOpenRef.current) {
          turnOpenRef.current = true;
          // Advisory only now: the gateway does not forward this upstream while
          // turn detection is the service's. It still drives the indicator.
          clientRef.current?.setActivity(true);
          showHearing(true);
          // Deliberately NO local `playback.stop()` here any more. This used to
          // silence the speakers the instant the local detector opened, and
          // the local detector opens on far more than speech — twelve opens
          // against two real interruptions in one session. Each one threw away
          // the tutor's queued audio while the transcript kept streaming: the
          // student saw the tutor "type" a full sentence and heard only parts
          // of it. Playback is now flushed on one signal only — the service's
          // own `interrupted` (`onFlushAudio`), which is the authority on
          // whether the student actually spoke over the tutor.
        }

        // The microphone streams CONTINUOUSLY, as the reference Live client
        // does. Turn detection is the service's now, and it can only hear a
        // turn begin in audio it is actually being sent — gating the stream on
        // a local decision would mean the tutor never answers, and would also
        // make barge-in impossible, since an interruption is just speech the
        // server hears while it is talking.
        //
        // The consequence is honest and worth stating: the tutor's own voice
        // now reaches the model whenever it leaves the speakers. `getUserMedia`
        // is asked for `echoCancellation` and that is what must suppress it.
        // Browser AEC is best-effort and weakest over loudspeakers, so a
        // headset is the reliable configuration here.
        //
        // ECHO GATE. What the service must never hear is the tutor's own voice
        // coming back through the microphone: every time it did, the server
        // detector took it for the student, cancelled the generation, and the
        // tutor was heard breaking off mid-sentence. So while the tutor is
        // audible, a frame that is not clearly louder than the learned bleed
        // level is sent as SILENCE rather than dropped. The stream stays
        // continuous — server-side detection needs an unbroken stream and a
        // gap would read as the client going away — but the echo is blanked
        // out of it. Real barge-in survives: a student speaking over the tutor
        // is louder than the bleed and passes through `overBleed` untouched.
        // The gate uses the same measurement the "hearing you" indicator does,
        // so what the student sees and what the model hears agree.
        const passes = !tutorAudible || overBleed;
        clientRef.current?.sendAudio(passes ? frame : silentFrameRef.current);
      },
      onVad: (event) => {
        const talking = event === "speech_start";
        // Mirrored into a ref because `onFrame` runs on every 20 ms frame and
        // must read the current value, not the one captured when the capture
        // was constructed.
        // The ref is what `onFrame` reads; `hearing` below is what the UI
        // shows. There is deliberately no third `speaking` state: it rendered
        // the raw, gap-by-gap VAD signal and was the blinking.
        speakingRef.current = talking;

        // The turn is closed in `onFrame`, not here: this event depends on the
        // VAD alone, and the tutor's bleed can hold the VAD open for as long as
        // it is speaking.

        // Held display value — see `hearing`. Driven from the event rather
        // than an effect, because the hold is a property of the transition,
        // not of the rendered state.
        // The indicator is no longer driven from this raw event. `speech_start`
        // fires on every energy blip the detector sees — echo, a chair, a
        // cough — and lighting "hearing you" on each one is the flicker the
        // student reported. It now follows the QUALIFIED turn in `onFrame`,
        // which has passed the absolute floor, the bleed test and the
        // `SPEECH_OPEN_FRAMES` run: the same evidence the echo gate uses, so
        // what the student sees and what the model is sent agree.
      },
      onError: (error) => setMicError(error.message),
    });
    try {
      await capture.start();
      captureRef.current = capture;
      setMicOn(true);
    } catch (error) {
      // The common case is a denied permission prompt, which needs saying
      // plainly — a silent failure looks like the tutor ignoring the student.
      setMicError(
        error instanceof Error
          ? `Microphone unavailable: ${error.message}`
          : "Microphone unavailable.",
      );
      await capture.stop();
    }
  }, [showHearing]);

  const start = useCallback(async () => {
    const renderer = rendererRef.current;
    // Synchronous latch first — see `startingRef`. Everything below this line
    // may suspend, so no other guard here is safe against a second call.
    if (!renderer || clientRef.current || startingRef.current) return;
    startingRef.current = true;
    setStarting(true);

    // Resumed inside this click: an AudioContext created outside a user gesture
    // stays suspended and the first turn plays silently.
    const playback = new AudioPlayback();
    await playback.resume();
    playbackRef.current = playback;

    const client = new SessionClient({
      documentId,
      url: webSocketUrl(),
      blockId,
      renderer,
      mintTicket,
      handlers: {
        onState: (next, info) => {
          setState(next);
          setDetail(info ?? null);
        },
        onSessionReady: (msg) => {
          // The unit (or, failing that, the block) named along the top of the
          // slate for the whole session — see `BoardRenderer.setUnitLabel`.
          const context = msg.context ?? {};
          const unit = typeof context.unit_title === "string" ? context.unit_title : null;
          const block =
            typeof context.block_title === "string"
              ? typeof context.block_no === "number"
                ? `Block ${context.block_no} · ${context.block_title}`
                : context.block_title
              : null;
          renderer.setUnitLabel(unit ?? block);
          setSession(msg);
          setQuotaMs(msg.quota_remaining_ms);
        },
        onTurnState: (msg) => {
          setTurn(msg);
          setBoardReady(true);
          // Attaches the citation to the ops already logged for this turn: the
          // gateway sends `board_ops` first and `turn_state` after it, so the
          // page number is not known at the moment the ops are captured.
          setBoardLog((prev) =>
            prev.map((entry) =>
              entry.seq === msg.seq && entry.turn === null ? { ...entry, turn: msg } : entry,
            ),
          );
        },
        onBoardOps: (msg: BoardOpsMessage) => {
          setBoardLog((prev) => {
            const next = msg.clear_first ? [] : prev;
            if (msg.ops.length === 0) return next;
            boardLogIdRef.current += 1;
            return [...next, { id: boardLogIdRef.current, seq: msg.seq, ops: msg.ops, turn: null }];
          });
        },
        onTranscript: (msg) => {
          setCaptions((prev) => {
            const last = prev[prev.length - 1];
            if (!last || last.source !== msg.source) {
              // Bounded: a long lesson must not grow this without limit.
              return [...prev, msg].slice(-40);
            }
            if (last.text.endsWith(msg.text)) return prev; // duplicate fragment
            // Live emits transcription in fragments — often a word or two at a
            // time. Rendered one per row they came out as a column of single
            // words each with its own "TUTOR" label, which is unreadable. They
            // are the same utterance, so they are appended into one line until
            // the speaker changes.
            //
            // The join is a space unless the fragment opens with punctuation
            // that must hug the previous word, and Malayalam fragments often
            // arrive already carrying their leading space.
            const joiner =
              /^[\s,.;:!?)\]}\u0d02\u0d03]/.test(msg.text) || last.text.endsWith(" ") ? "" : " ";
            const merged = { ...last, text: `${last.text}${joiner}${msg.text}` };
            return [...prev.slice(0, -1), merged];
          });
        },
        onQuotaWarning: setQuotaMs,
        onSessionEnd: (reason) => setEnded(reason),
        onError: (code, message) => setDetail(`${code}: ${message}`),
        onAudio: (frame) => playback.enqueue(frame),
        // An interrupted turn, a server-sent session_end, or a terminal close.
        onFlushAudio: () => playback.stop(),
        onAckLatency: (seq, ms) => {
          setBoardReady(true);
          setTurns((prev) =>
            [...prev, { seq, ackMs: Math.round(ms), violation: ms > HOLD_MAX_MS }].slice(-8),
          );
        },
      },
    });
    clientRef.current = client;
    setStarted(true);
    setBoardReady(false);
    setCaptions([]);
    setBoardLog([]);
    try {
      await client.connect();
    } finally {
      // The socket is now owned by `clientRef`, which is itself a sufficient
      // guard from here on; the latch only had to cover the suspension window
      // above.
      startingRef.current = false;
      setStarting(false);
    }
    // Started in the same click as everything else above, so it shares the
    // user-gesture permission grant — a student who came here to learn should
    // not have to find and press a second button to be heard.
    void startMic();
    // `documentId` is read when the socket opens, so it belongs here with
    // `blockId` — the two together decide what this session teaches.
  }, [blockId, documentId, startMic]);

  const leave = useCallback(async () => {
    clientRef.current?.close();
    clientRef.current = null;
    startingRef.current = false;
    setStarted(false);
    setBoardReady(false);
    setTutorSpeaking(false);
    await stopMic();
    await playbackRef.current?.close();
    playbackRef.current = null;
  }, [stopMic]);

  // Teardown on unmount and on tab close. Both matter: "explicit teardown on
  // navigation, tab close, and component unmount. A leaked socket burns quota
  // and API budget."
  useEffect(() => {
    const onPageHide = () => {
      clientRef.current?.close();
      void captureRef.current?.stop();
    };
    window.addEventListener("pagehide", onPageHide);
    return () => {
      window.removeEventListener("pagehide", onPageHide);
      onPageHide();
      void playbackRef.current?.close();
    };
  }, []);

  useEffect(() => {
    // Scoped to this one element's own `scrollTop`, deliberately not
    // `scrollIntoView()`: that call walks up the ancestor chain and can
    // scroll ANY scrollable ancestor it finds along the way, including
    // `.dg-board-shell` itself — `overflow: hidden` is still a valid (if
    // scrollbar-less) scroll container. Since the canvas is positioned
    // `absolute` inside that same box, scrolling the shell would drag the
    // canvas's rendered content sideways with it. This was the leading
    // suspect for the board content observed shifted and clipped on one
    // side — `scrollTop` on the caption panel directly can only ever affect
    // that one element.
    const el = notesScrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [captions]);

  useEffect(() => {
    const sync = () => setFullscreen(document.fullscreenElement !== null);
    document.addEventListener("fullscreenchange", sync);
    sync();
    return () => document.removeEventListener("fullscreenchange", sync);
  }, []);

  /**
   * Fullscreen is requested on the whole page, not on the board element.
   * Fullscreening the canvas alone would take the transcript, the topic and
   * the microphone control off screen — the board is the biggest part of the
   * classroom, not the whole of it.
   *
   * Rejections are swallowed: a browser may refuse (an iframe without
   * `allowfullscreen`, a policy block) and a refused *cosmetic* request is not
   * something to interrupt a lesson with. `fullscreenchange` keeps the label
   * honest either way.
   */
  const toggleFullscreen = useCallback(() => {
    if (document.fullscreenElement) {
      void document.exitFullscreen().catch(() => {});
      return;
    }
    void document.documentElement.requestFullscreen().catch(() => {});
  }, []);

  /**
   * Exports the lesson as a PDF through the browser's own print pipeline.
   *
   * Deliberately not a PDF library. Every candidate needs the font embedded to
   * write Malayalam at all, and an export that silently drops the script the
   * lesson is taught in is worse than no export. The print path uses the fonts
   * already loaded on the page, so what is saved is what was on the board —
   * conjuncts and all — and "Save as PDF" is a destination in every browser's
   * print dialogue.
   *
   * The printable document is rendered into the page behind `@media print`
   * rules rather than opened in a new window, which a pop-up blocker would
   * eat.
   */
  const downloadNotes = useCallback(() => {
    window.print();
  }, []);

  /** Plain-text fallback: the same notes on the clipboard, for a chat or an email. */
  const copyNotes = useCallback(async () => {
    const text = notesAsText(boardLog, captions, turn);
    try {
      await navigator.clipboard.writeText(text);
      setDetail("Notes copied to the clipboard.");
    } catch {
      setDetail("The browser would not let the page write to the clipboard.");
    }
  }, [boardLog, captions, turn]);

  /**
   * The portal target for the printable notes, resolved after mount.
   *
   * `document.body` cannot be read during render without breaking SSR, so it
   * is reached through `useSyncExternalStore`: the server snapshot is `null`
   * and the client snapshot is the body, which is exactly the split React
   * wants and which hydrates without a mismatch warning. The store never
   * changes, hence the no-op subscribe.
   */
  const printHost = useSyncExternalStore(
    subscribeNever,
    () => document.body as HTMLElement | null,
    () => null,
  );

  const lastCaption = captions[captions.length - 1] ?? null;
  const connected = state === "ready";
  const worstAck = turns.reduce((max, entry) => Math.max(max, entry.ackMs), 0);
  const violations = turns.filter((entry) => entry.violation).length;

  return (
    /*
      The classroom fills the browser tab and nothing more: `h-dvh` plus
      `overflow-hidden` on the page, and every child sized in `min-h-0` flex
      terms. There is deliberately no Fullscreen API call — a lesson the student
      cannot escape with a glance at their tabs is a worse experience than one
      that simply uses all the room it has been given.
    */
    <main className="flex h-dvh flex-col overflow-hidden bg-[#141a17] text-lavender-50">
      <header className="flex flex-wrap items-center gap-x-4 gap-y-2 border-b border-black/40 bg-[#101512] px-4 py-2.5 sm:px-6">
        {/*
          Back to the unit list, not to the dashboard — a student who wants a
          different unit of the same block should not have to walk the whole
          course tree again. "Leave" (far right) is the way out of the module
          entirely; these are different intentions and get different controls.
        */}
        <Link
          href={backHref}
          aria-label="Back to the unit list"
          className="rounded-lg px-2 py-1 text-[13px] text-gray-400 transition hover:bg-white/5 hover:text-gray-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400"
        >
          ← Back
        </Link>
        <Logo size="sm" />
        <StatusPill state={state} sessionId={session?.session_id ?? null} />
        {turn ? (
          <span className="flex min-w-0 items-baseline gap-2 text-[13px]">
            <span className="truncate font-semibold text-white">
              {turn.topic ?? "Untitled topic"}
            </span>
            <span className="shrink-0 text-gray-500">
              ch {turn.chapter ?? "—"} · p{turn.page ?? "—"}
            </span>
          </span>
        ) : null}
        {quotaMs !== null ? (
          <span className="text-[13px] text-gray-400">
            {Math.floor(quotaMs / 60000)} min left today
          </span>
        ) : null}
        {turns.length > 0 ? (
          /*
            The one invariant a student-facing surface can usefully prove:
            every turn's board landed before its audio. A non-zero violation
            count is a release blocker (whiteboard-sync.md), so it stays
            visible — but as a chip rather than the old sidebar panel, because
            the board now takes the full width.
          */
          <span
            title={`worst paint ${worstAck} ms · ceiling ${HOLD_MAX_MS} ms`}
            className={`rounded-full px-2.5 py-1 text-[12px] font-semibold ${
              violations === 0
                ? "bg-emerald-500/10 text-emerald-300"
                : "bg-rose-500/15 text-rose-300"
            }`}
          >
            {violations === 0 ? `board in sync · ${worstAck} ms` : `${violations} over ceiling`}
          </span>
        ) : null}
        <div className="ml-auto flex items-center gap-2">
          {/*
            The microphone state moved here when the caption strip became a
            three-line glance surface. It is genuinely load-bearing: the mic is
            gated closed while the tutor speaks (half-duplex), and without this
            a student talking into a closed gate has no way to tell why nothing
            is being heard.
          */}
          {connected && micOn ? (
            <span
              className={`hidden rounded-full px-2.5 py-1 text-[12px] font-medium sm:inline ${
                tutorSpeaking
                  ? "bg-white/5 text-gray-400"
                  : hearing
                    ? "bg-emerald-500/15 text-emerald-300"
                    : "bg-white/5 text-gray-400"
              }`}
            >
              {tutorSpeaking ? "tutor speaking" : hearing ? "hearing you" : "listening"}
            </span>
          ) : null}
          {connected ? (
            <button
              type="button"
              onClick={micOn ? stopMic : startMic}
              className={buttonClass("outline", "px-3 py-1.5 text-[13px]")}
            >
              {micOn ? "Mute microphone" : "Unmute microphone"}
            </button>
          ) : null}
          {/*
            There is deliberately no Interrupt button. Barging in is what
            speaking does — the student talks and the tutor stops, the way it
            works in a room — and a button that duplicates that only adds a
            control to think about mid-lesson. `SessionClient.interrupt()` is
            still called on a real barge-in; only the manual affordance is
            gone.
          */}
          {started ? (
            <button
              type="button"
              onClick={copyNotes}
              className={buttonClass("ghost", "px-3 py-1.5 text-[13px]")}
            >
              Copy notes
            </button>
          ) : null}
          {started ? (
            <button
              type="button"
              onClick={downloadNotes}
              className={buttonClass("outline", "px-3 py-1.5 text-[13px]")}
            >
              Download PDF
            </button>
          ) : null}
          {/*
            Two buttons rather than one that changes label, so the state the
            student is in is readable from which one is live rather than from
            reading the text on a single toggle.
          */}
          <button
            type="button"
            onClick={toggleFullscreen}
            disabled={fullscreen}
            title="Fill the screen"
            className={buttonClass("outline", `px-3 py-1.5 text-[13px] ${fullscreen ? "opacity-40" : ""}`)}
          >
            Fullscreen
          </button>
          <button
            type="button"
            onClick={toggleFullscreen}
            disabled={!fullscreen}
            title="Back to the window (Esc)"
            className={buttonClass("outline", `px-3 py-1.5 text-[13px] ${!fullscreen ? "opacity-40" : ""}`)}
          >
            Minimise
          </button>
          {started ? (
            <button
              type="button"
              onClick={leave}
              className={buttonClass("ghost", "text-[13px]")}
            >
              Leave
            </button>
          ) : null}
        </div>
      </header>

      {detail ? (
        <p className="border-b border-black/40 bg-[#101512] px-4 py-2 text-[13px] text-amber-300 sm:px-6">
          {detail}
        </p>
      ) : null}
      {micError ? (
        <p className="border-b border-black/40 bg-[#101512] px-4 py-2 text-[13px] text-rose-300 sm:px-6">
          {micError}
        </p>
      ) : null}

      <div className="dg-stage flex min-h-0 flex-1 gap-3 p-3 sm:p-4">
        {/*
          The slate and its wooden surround. The frame is CSS chrome and the
          canvas is only the green field inside it — keeping the woodwork out
          of the op renderer means a board snapshot or replay is the lesson,
          not the furniture.
        */}
        <div className="dg-chalkboard relative min-h-0 flex-1">
          <div
            ref={shellRef}
            className="dg-board-shell absolute inset-0 overflow-hidden rounded-[4px]"
          >
            <canvas ref={canvasRef} />

            {!started ? (
              <div className="absolute inset-0 flex flex-col items-center justify-center gap-4 bg-black/45 p-6 text-center">
                <h1 className="max-w-md text-balance font-display text-[26px] font-bold leading-tight text-white">
                  Ready when you are
                </h1>
                <p className="max-w-md text-[15px] leading-relaxed text-gray-200">
                  The board always appears before the tutor speaks. Starting the session asks for
                  your microphone.
                </p>
                <button
                  type="button"
                  onClick={start}
                  disabled={starting}
                  className={buttonClass("primary", starting ? "opacity-60" : "")}
                >
                  {starting ? "Starting…" : "Start the session"}
                </button>
              </div>
            ) : null}

            {/*
              Distinct from the pre-start overlay above: the socket is open and
              connected, the tutor just has not produced its opening turn yet
              (typically a couple of seconds while it composes the greeting).
              Without this the board is indistinguishable from broken.
            */}
            {started && !boardReady && captions.length === 0 && !ended ? (
              <div className="absolute inset-0 flex flex-col items-center justify-center gap-3 bg-black/35 p-6 text-center">
                <div
                  aria-hidden
                  className="h-8 w-8 animate-spin rounded-full border-2 border-white/25 border-t-white/80"
                />
                <p className="text-[15px] text-gray-100">Your tutor is opening the block…</p>
              </div>
            ) : null}

            {ended ? (
              <div className="absolute inset-0 flex flex-col items-center justify-center gap-4 bg-black/70 p-6 text-center">
                <h2 className="font-display text-[22px] font-bold text-white">Session ended</h2>
                <p className="max-w-sm text-[15px] text-gray-200">{ended}</p>
                <Link
                  href="/dashboard"
                  className="rounded-lg bg-indigo-500 px-4 py-2 text-[14px] font-semibold text-white hover:bg-indigo-400"
                >
                  Back to dashboard
                </Link>
              </div>
            ) : null}

            {/*
              The transcript rides on the lower edge of the slate rather than
              taking a panel of its own below it. Two reasons it is an overlay
              and not a sibling: the board is the lesson and should get every
              pixel of the tab, and captions are read in glances — parked at the
              bottom of the thing being explained, they need no eye travel.

              It fades out upward into the board instead of sitting on a solid
              bar, and the renderer pages the board at `FLOW_BOTTOM` (0.72), so
              a line of chalk is never written behind a caption.
            */}
            {/*
              Slide controls, bottom-left of the slate.

              The board fills up and turns a page on its own; before this, the
              page that scrolled away was simply gone, so a student who looked
              down at their notebook lost the explanation behind them. Turning
              back does not pause anything — the tutor keeps writing, and the
              next op snaps the student forward to it, because NN-1 exists so
              the student is looking at the thing being explained.
            */}
            {started && board.total > 1 ? (
              <div className="absolute bottom-3 left-4 z-10 flex items-center gap-1 rounded-full border border-white/10 bg-black/55 px-1.5 py-1 backdrop-blur-sm sm:left-6">
                <button
                  type="button"
                  onClick={() => rendererRef.current?.goToPage(board.page - 1)}
                  disabled={board.page === 0}
                  aria-label="Previous slide"
                  className="rounded-full px-2 py-0.5 text-[15px] leading-none text-white/80 transition hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-30"
                >
                  ‹
                </button>
                <span className="min-w-[44px] text-center text-[12px] tabular-nums text-white/60">
                  {board.page + 1} / {board.total}
                </span>
                <button
                  type="button"
                  onClick={() => rendererRef.current?.goToPage(board.page + 1)}
                  disabled={board.page >= board.total - 1}
                  aria-label="Next slide"
                  className="rounded-full px-2 py-0.5 text-[15px] leading-none text-white/80 transition hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-30"
                >
                  ›
                </button>
              </div>
            ) : null}

            {/*
              The live caption strip: the most recent thing said, clamped to
              three lines.

              It is a glance surface, not a record — the full conversation is
              in the notebook beside the board. Scrolling it back was the wrong
              affordance on the slate: a student reading upward through history
              is a student not watching the board the audio is waiting on. The
              clamp also fixes the band's height, so the chalk above it never
              shifts as captions arrive.
            */}
            {started && lastCaption ? (
              <div
                aria-live="polite"
                className="pointer-events-none absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/75 via-black/50 to-transparent px-4 pb-3 pl-28 pt-10 sm:px-6 sm:pl-32"
              >
                <p className="dg-caption-clamp text-[15px] leading-snug text-white/95">
                  <span className="mr-1.5 align-[1px] text-[10px] font-semibold uppercase tracking-[0.12em] text-white/40">
                    {lastCaption.source === "tutor" ? "Tutor" : "You"}
                  </span>
                  {lastCaption.text}
                </p>
              </div>
            ) : null}
          </div>
          {/* The chalk ledge, purely decorative — it is what makes the frame read as a board. */}
          <div className="dg-chalk-ledge" aria-hidden>
            <i />
            <i />
            <span />
          </div>
        </div>

        {/*
          The notebook. Shown only in the windowed view: fullscreen is the
          "just the board" mode, and a panel taking a fifth of the screen there
          would defeat the reason for asking for fullscreen in the first place.
          The three-line caption strip on the slate carries the conversation
          while it is hidden.
        */}
        {started && notesOpen && !fullscreen ? (
          <aside className="dg-notes flex w-full max-w-sm shrink-0 flex-col overflow-hidden rounded-xl border border-white/10 bg-[#101512] lg:w-80">
            <div className="flex items-center gap-2 border-b border-white/10 px-3 py-2">
              <h2 className="text-[11px] font-semibold uppercase tracking-[0.14em] text-gray-500">
                Notebook
              </h2>
              <button
                type="button"
                onClick={() => setNotesOpen(false)}
                className="ml-auto text-[12px] text-gray-500 hover:text-gray-300"
              >
                Hide
              </button>
            </div>
            <div ref={notesScrollRef} className="min-h-0 flex-1 overflow-y-auto px-3 py-3">
              {captions.length === 0 ? (
                <p className="text-[13px] text-gray-500">
                  Everything said in this lesson is collected here, and the board is written
                  up underneath it. Use Download PDF to keep it.
                </p>
              ) : (
                <ul className="space-y-3">
                  {captions.map((entry, index) => (
                    <li key={index}>
                      <p
                        className={`text-[10px] font-semibold uppercase tracking-[0.12em] ${
                          entry.source === "tutor" ? "text-emerald-400/70" : "text-indigo-300/70"
                        }`}
                      >
                        {entry.source === "tutor" ? "Tutor" : "You"}
                      </p>
                      <p className="mt-0.5 text-[14px] leading-relaxed text-gray-200">
                        {entry.text}
                      </p>
                    </li>
                  ))}
                </ul>
              )}

              {boardLog.length > 0 ? (
                <section className="mt-5 border-t border-white/10 pt-4">
                  <h3 className="text-[10px] font-semibold uppercase tracking-[0.14em] text-gray-500">
                    From the board
                  </h3>
                  <div className="mt-2 space-y-3">
                    {boardLog.map((entry) => (
                      <BoardNote key={entry.id} entry={entry} />
                    ))}
                  </div>
                </section>
              ) : null}
            </div>
          </aside>
        ) : null}

        {started && !notesOpen && !fullscreen ? (
          <button
            type="button"
            onClick={() => setNotesOpen(true)}
            className="dg-notes shrink-0 self-start rounded-xl border border-white/10 bg-[#101512] px-3 py-2 text-[12px] text-gray-400 hover:text-gray-200"
          >
            Notebook
          </button>
        ) : null}
      </div>

      {/*
        The printable document. In the page rather than in a pop-up so no
        blocker can eat it, and hidden except under `@media print` — see
        `downloadNotes` for why this goes through the browser's print pipeline
        instead of a PDF library.

        Portalled to `<body>` deliberately. The print stylesheet hides
        `body > *` and re-shows this one node; while it was nested inside
        `<main>` it was hidden along with its ancestor, and the export came out
        as a blank page. A portal makes it a sibling of the classroom rather
        than a descendant, so exactly one rule governs what prints.
      */}
      {printHost
        ? createPortal(
            <article className="dg-print-notes" aria-hidden>
        <h1>{turn?.topic ?? "Digi Guru lesson"}</h1>
        <p className="dg-print-meta">{lessonMeta(session, turn)}</p>

        {boardLog.length > 0 ? (
          <>
            <h2>Notes from the board</h2>
            {boardLog.map((entry) => (
              <PrintableBoardNote key={entry.id} entry={entry} />
            ))}
          </>
        ) : null}

        {captions.length > 0 ? (
          <>
            <h2>What was said</h2>
            {captions.map((entry, index) => (
              <p key={index} className="dg-print-line">
                <b>{entry.source === "tutor" ? "Tutor" : "You"}:</b> {entry.text}
              </p>
            ))}
          </>
              ) : null}
            </article>,
            printHost,
          )
        : null}
    </main>
  );
}

/**
 * The breadcrumb line under the export's title.
 *
 * `session_ready.context` is `Record<string, unknown>` on the wire — the
 * gateway is free to add keys to it without a client change — so each field is
 * narrowed here rather than trusted. A key that is missing or the wrong type is
 * left out of the line instead of printing "undefined" onto a study note.
 */
function lessonMeta(session: SessionReady | null, turn: TurnState | null): string {
  const context = session?.context ?? {};
  const text = (key: string): string | null => {
    const value = context[key];
    return typeof value === "string" && value.trim() !== "" ? value : null;
  };
  const num = (key: string, label: string): string | null => {
    const value = context[key];
    return typeof value === "number" ? `${label} ${value}` : null;
  };
  return [
    text("program"),
    num("semester", "Semester"),
    num("block_no", "Block"),
    turn?.chapter ? `Chapter ${turn.chapter}` : null,
    turn?.page != null ? `Page ${turn.page}` : null,
  ]
    .filter(Boolean)
    .join(" · ");
}

/** One turn of the board, rendered as notebook lines. */
function BoardNote({ entry }: { entry: BoardLogEntry }) {
  return (
    <div>
      {entry.turn?.page != null ? (
        <p className="text-[11px] text-gray-500">
          {entry.turn.chapter ? `${entry.turn.chapter} · ` : ""}p{entry.turn.page}
        </p>
      ) : null}
      {entry.ops.map((op, index) => (
        <BoardOpText key={index} op={op} />
      ))}
    </div>
  );
}

/**
 * The text an op carries, or nothing.
 *
 * `draw` and `highlight` have no words of their own — a shape and an emphasis
 * on an element written earlier — so they contribute nothing to a note and are
 * skipped rather than rendered as a placeholder.
 */
function BoardOpText({ op }: { op: BoardOp }) {
  switch (op.kind) {
    case "heading":
      return <p className="text-[14px] font-semibold text-white">{op.text}</p>;
    case "bullets":
      return (
        <ul className="mt-1 space-y-0.5">
          {op.items.map((item, index) => (
            <li key={index} className="text-[13px] leading-relaxed text-gray-300">
              — {item}
            </li>
          ))}
        </ul>
      );
    case "math":
      return (
        <p className="mt-1 font-mono text-[13px] text-emerald-200/90">{op.latex}</p>
      );
    case "image":
      return <p className="mt-1 text-[12px] italic text-gray-500">figure {op.reference}</p>;
    default:
      return null;
  }
}

/** The same turn, with no Tailwind — the print stylesheet owns this typography. */
function PrintableBoardNote({ entry }: { entry: BoardLogEntry }) {
  return (
    <section className="dg-print-turn">
      {entry.ops.map((op, index) => {
        switch (op.kind) {
          case "heading":
            return <h3 key={index}>{op.text}</h3>;
          case "bullets":
            return (
              <ul key={index}>
                {op.items.map((item, i) => (
                  <li key={i}>{item}</li>
                ))}
              </ul>
            );
          case "math":
            return <pre key={index}>{op.latex}</pre>;
          case "image":
            return <p key={index}>[figure {op.reference}]</p>;
          default:
            return null;
        }
      })}
      {entry.turn?.page != null ? (
        <p className="dg-print-cite">
          {entry.turn.chapter ? `${entry.turn.chapter}, ` : ""}page {entry.turn.page}
        </p>
      ) : null}
    </section>
  );
}

/** The clipboard version of what `Download PDF` prints. */
function notesAsText(
  boardLog: BoardLogEntry[],
  captions: Transcript[],
  turn: TurnState | null,
): string {
  const lines: string[] = [];
  if (turn?.topic) lines.push(turn.topic, "");
  if (boardLog.length > 0) {
    lines.push("NOTES FROM THE BOARD", "");
    for (const entry of boardLog) {
      for (const op of entry.ops) {
        if (op.kind === "heading") lines.push(op.text);
        else if (op.kind === "bullets") lines.push(...op.items.map((item) => `  - ${item}`));
        else if (op.kind === "math") lines.push(`  ${op.latex}`);
      }
      if (entry.turn?.page != null) lines.push(`  (page ${entry.turn.page})`);
      lines.push("");
    }
  }
  if (captions.length > 0) {
    lines.push("WHAT WAS SAID", "");
    for (const entry of captions) {
      lines.push(`${entry.source === "tutor" ? "Tutor" : "You"}: ${entry.text}`);
    }
  }
  return lines.join("\n");
}

function StatusPill({
  state,
  sessionId,
}: {
  state: ConnectionState;
  sessionId: string | null;
}) {
  const tone: Record<ConnectionState, string> = {
    idle: "bg-ink-800 text-gray-300",
    connecting: "bg-indigo-500/15 text-indigo-300",
    reconnecting: "bg-amber-500/15 text-amber-300",
    ready: "bg-emerald-500/15 text-emerald-300",
    closed: "bg-ink-800 text-gray-400",
    failed: "bg-rose-500/15 text-rose-300",
  };
  return (
    <span
      // The session id is what a support request is traced by, but it is not
      // something a student reads, so it lives in the tooltip rather than
      // taking header width from the topic.
      title={sessionId ? `session ${sessionId}` : undefined}
      className={`rounded-full px-2.5 py-1 text-[12px] font-semibold ${tone[state]}`}
    >
      {state}
    </span>
  );
}
