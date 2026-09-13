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

import { useCallback, useEffect, useRef, useState } from "react";
import Link from "next/link";

import { API_BASE_URL } from "@/lib/api";
import { AudioCapture } from "@/classroom/audio-capture";
import { AudioPlayback } from "@/classroom/audio-playback";
import { BoardRenderer, HOLD_MAX_MS } from "@/classroom/board-renderer";
import { SessionClient } from "@/classroom/session-client";
import type { ConnectionState } from "@/classroom/session-client";
import type { BoardOp, SessionReady, Transcript, TurnState } from "@/classroom/protocol";
import { buttonClass } from "@/components/ui/Button";
import { Logo } from "@/components/ui/Logo";

/**
 * Mic frames are withheld while at least this much tutor audio is queued.
 * Small and non-zero: `queuedSeconds` briefly reads a tiny positive number
 * even right as a turn finishes, and treating that as "still speaking" for a
 * moment is harmless, whereas gating on `> 0` exactly would flap on/off with
 * every scheduling jitter.
 */
const PLAYBACK_GATE_S = 0.05;

/**
 * How much of the student's speech to hold back while the tutor is talking, so
 * that the moment the gate opens it can be sent ahead of the live frames.
 *
 * Without this, a student who starts answering before the tutor has quite
 * finished loses the front of their own sentence: the gate is closed, those
 * frames are dropped, and the model receives a fragment starting mid-word.
 * That was the "sentences are not connected" symptom. 300 ms covers a normal
 * overlap without hoarding enough audio to matter if it is discarded.
 *
 * Sending this pre-roll cannot revive the echo bug it sits next to: the echo
 * problem was Gemini interrupting its OWN in-flight stream on hearing itself.
 * By the time this is flushed the tutor's stream has already ended, so a little
 * echo tail at the head of the student's turn has nothing to interrupt.
 */
const PREROLL_MS = 300;
const PREROLL_FRAMES = Math.ceil(PREROLL_MS / 20);

/** `http(s)://host/api/v1` -> `ws(s)://host/ws/session`. */
function webSocketUrl(): string {
  const base = new URL(API_BASE_URL);
  base.protocol = base.protocol === "https:" ? "wss:" : "ws:";
  base.pathname = "/ws/session";
  base.search = "";
  return base.toString();
}

async function mintTicket(): Promise<string> {
  const response = await fetch(`${API_BASE_URL}/ws/ticket`, {
    method: "POST",
    credentials: "include",
  });
  if (!response.ok) {
    throw new Error("Could not authorise the classroom session.");
  }
  const body: unknown = await response.json();
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

export function ClassroomSession({ blockId }: { blockId: string }) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const shellRef = useRef<HTMLDivElement | null>(null);
  const rendererRef = useRef<BoardRenderer | null>(null);
  const clientRef = useRef<SessionClient | null>(null);
  const captureRef = useRef<AudioCapture | null>(null);
  const playbackRef = useRef<AudioPlayback | null>(null);
  /** Student speech captured while the mic was gated — see `PREROLL_MS`. */
  const prerollRef = useRef<Int16Array[]>([]);
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
  const [speaking, setSpeaking] = useState(false);
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
  // Captions. The Live service can repeat the last partial output chunk when
  // a turn ends (observed live), so consecutive identical entries from the
  // same speaker are collapsed rather than shown twice.
  const [captions, setCaptions] = useState<Transcript[]>([]);
  const captionsScrollRef = useRef<HTMLDivElement | null>(null);
  // Whether the tutor currently has audio queued — drives both the "hearing
  // you" vs "tutor speaking" indicator and whether the manual Interrupt
  // button is shown. `AudioPlayback.queuedSeconds` is imperative, so this is
  // a light poll rather than an event; cheap, and simpler than plumbing a
  // callback through for a UI-only signal.
  const [tutorSpeaking, setTutorSpeaking] = useState(false);
  /** Drives the button's disabled state; `startingRef` is what actually guards. */
  const [starting, setStarting] = useState(false);

  useEffect(() => {
    // Reset happens in `leave()` (an event handler, not here) — an effect
    // body must only synchronise with the external system, never call
    // `setState` directly on entry.
    if (!started) return;
    const id = setInterval(() => {
      setTutorSpeaking((playbackRef.current?.queuedSeconds ?? 0) > PLAYBACK_GATE_S);
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
    prerollRef.current = [];
    setMicOn(false);
    setSpeaking(false);
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
      onFrame: (frame) => {
        const gated = (playbackRef.current?.queuedSeconds ?? 0) > PLAYBACK_GATE_S;
        if (gated) {
          // Hold the tail end of what the student is saying rather than
          // discarding it outright.
          const roll = prerollRef.current;
          roll.push(frame);
          if (roll.length > PREROLL_FRAMES) roll.shift();
          return;
        }
        // Gate just opened: lead with whatever the student had already started
        // saying, in order, before the live frames.
        const roll = prerollRef.current;
        if (roll.length > 0) {
          prerollRef.current = [];
          for (const held of roll) clientRef.current?.sendAudio(held);
        }
        clientRef.current?.sendAudio(frame);
      },
      onVad: (event) => {
        setSpeaking(event === "speech_start");
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
  }, []);

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
          setSession(msg);
          setQuotaMs(msg.quota_remaining_ms);
        },
        onTurnState: (msg) => {
          setTurn(msg);
          setBoardReady(true);
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
  }, [blockId, startMic]);

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
    const el = captionsScrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [captions]);

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
            Barge-in is manual, not automatic-on-VAD: browser echo
            cancellation is unreliable for audio scheduled through raw Web
            Audio nodes, so the mic hears the tutor's own voice and local VAD
            cannot tell that apart from the student genuinely interrupting.
            Automatic interruption on that false signal was the doubled/
            flickering-voice bug. A button the student presses on purpose has
            no such ambiguity.
          */}
          {connected && tutorSpeaking ? (
            <button
              type="button"
              onClick={() => clientRef.current?.interrupt()}
              className={buttonClass("outline", "px-3 py-1.5 text-[13px]")}
            >
              Interrupt
            </button>
          ) : null}
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

      <div className="flex min-h-0 flex-1 flex-col p-3 sm:p-4">
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
            {started ? (
              <div
                ref={captionsScrollRef}
                aria-live="polite"
                className="pointer-events-none absolute inset-x-0 bottom-0 max-h-[32%] overflow-y-auto scroll-smooth bg-gradient-to-t from-black/70 via-black/45 to-transparent px-4 pb-3 pt-10 sm:px-6"
              >
                {captions.length === 0 ? (
                  <p className="text-[13px] text-white/45">
                    {!micOn
                      ? "Microphone off."
                      : tutorSpeaking
                        ? "Tutor speaking — your microphone is muted."
                        : speaking
                          ? "Hearing you…"
                          : "Listening. What you both say appears here."}
                  </p>
                ) : (
                  <ul className="space-y-1">
                    {captions.map((entry, index) => (
                      <li
                        key={index}
                        className={`text-[15px] leading-snug ${
                          entry.source === "tutor"
                            ? "text-white/95"
                            : "text-white/60"
                        }`}
                      >
                        <span className="mr-1.5 align-[1px] text-[10px] font-semibold uppercase tracking-[0.12em] text-white/40">
                          {entry.source === "tutor" ? "Tutor" : "You"}
                        </span>
                        {entry.text}
                      </li>
                    ))}
                  </ul>
                )}
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
      </div>
    </main>
  );
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
