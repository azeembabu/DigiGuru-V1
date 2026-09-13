/**
 * Tutor audio playback with a small jitter buffer.
 *
 * Frames arrive from the gateway in bursts — especially right after a turn's
 * audio is released from the NN-1 hold, which by design delivers everything that
 * was buffered at once. Playing each frame the moment it lands would click and
 * gap. So frames are scheduled on the `AudioContext` clock, each one starting
 * exactly where the previous finished.
 *
 * `.claude/rules/realtime-audio.md` budgets 70 ms for "downstream + jitter
 * buffer", which is the target here: enough lead to absorb network wobble,
 * little enough that the tutor does not feel laggy.
 *
 * Back-pressure rule from the same file, applied to the *playback* side: if the
 * queue runs far ahead of real time the student has fallen behind the lesson, so
 * the oldest scheduled audio is dropped rather than letting the backlog grow
 * unbounded.
 */

import { pcm16ToFloat } from "./vad";

/**
 * Tutor playback rate, in Hz. **Deliberately different from the 16 kHz capture
 * rate in `vad.ts`, and deliberately not imported from it.**
 *
 * Gemini Live is asymmetric: it wants student microphone audio as PCM16 mono at
 * 16 000 Hz (which is also what the gateway expects upstream, and what the exact
 * 20 ms / 320-sample frame maths in `vad.ts` depends on), but it returns the
 * tutor's voice as PCM16 mono at 24 000 Hz. So this one codebase legitimately
 * has two sample rates, one per direction.
 *
 * Do not "tidy" these into a single shared constant. Playing 24 kHz samples
 * through a 16 kHz `AudioContext` stretches them by 1.5x: the tutor comes out
 * slow and pitched down, with no error anywhere to explain it. That is the exact
 * bug this separation exists to prevent.
 */
export const PLAYBACK_SAMPLE_RATE = 24_000;

/** Target lead time before the first frame plays. */
const JITTER_LEAD_S = 0.07;
/**
 * Safety valve only — NOT a back-pressure limit.
 *
 * This used to be 1.5 s, on the reasoning that audio queued further ahead than
 * that means the listener has fallen behind. That reasoning is wrong for this
 * protocol, and it was silently eating most of every explanation.
 *
 * Gemini Live does not stream a turn in real time: it generates the whole turn
 * and pushes it as fast as the socket allows. Measured against the live
 * service, one greeting delivered **24.3 s of speech in 6.7 s of wall time** —
 * 3.6x faster than realtime. A 1.5 s cap therefore dropped ~22.8 s of that one
 * turn, which is what "it skips words and never finishes a sentence" actually
 * was.
 *
 * A turn's audio is a bounded, finite payload that the student is entitled to
 * hear in full, and the NN-1 gate upstream already decides *when* it may be
 * released. The correct way to stop unwanted audio is `stop()` on an
 * interruption, not dropping frames mid-sentence. This value now exists only
 * so a pathological stream cannot grow memory without bound; tripping it is a
 * bug worth seeing, so it logs.
 */
const MAX_QUEUE_S = 300;

export class AudioPlayback {
  private context: AudioContext | null = null;
  /** Absolute context time at which the next frame should start. */
  private nextStartTime = 0;
  private active = new Set<AudioBufferSourceNode>();
  private droppedFrames = 0;

  /** Frames discarded because the student was too far behind. Surfaced for logging. */
  get dropped(): number {
    return this.droppedFrames;
  }

  /**
   * Must be called from a user gesture.
   *
   * Browsers start an `AudioContext` suspended until a real interaction, so
   * playback is wired to the same click that starts the session rather than to
   * page load — otherwise the first turn is silent with no visible cause.
   */
  async resume(): Promise<void> {
    if (!this.context) {
      this.context = new AudioContext({ sampleRate: PLAYBACK_SAMPLE_RATE });
    }
    if (this.context.state === "suspended") {
      await this.context.resume();
    }
  }

  /**
   * Queues one PCM16 frame.
   *
   * Silently ignored before `resume()`: dropping audio is the right failure when
   * the context is not running, since the alternative is buffering an unbounded
   * backlog that would then play all at once.
   */
  enqueue(bytes: ArrayBuffer): void {
    const context = this.context;
    if (!context) return;
    if (bytes.byteLength < 2) return;

    // A suspended context silently swallows everything scheduled on it, and
    // browsers suspend one for reasons outside this app's control — the tab
    // going to the background is the common one, and Chrome will also suspend
    // an idle context on its own. Previously this method simply returned when
    // the context was not running, so from the first suspension onward the
    // tutor was mute forever with nothing logged: the frames kept arriving and
    // kept being thrown away.
    //
    // `resume()` is async, so this frame is still lost, but the NEXT one will
    // play. Deliberately fire-and-forget: awaiting here would reorder frames
    // against later `enqueue` calls, which is a worse failure than losing one
    // 100 ms chunk at the moment a tab is brought back to the foreground.
    if (context.state !== "running") {
      void context.resume().catch(() => {
        // Nothing useful to do — a context that will not resume needs a user
        // gesture, and the session UI is what can ask for one.
      });
      return;
    }

    // A PCM16 frame is an even number of bytes. An odd length means a truncated
    // frame, and decoding it would shift every subsequent sample by one byte,
    // turning speech into noise.
    const usableBytes = bytes.byteLength - (bytes.byteLength % 2);
    const pcm = new Int16Array(bytes, 0, usableBytes / 2);
    const samples = pcm16ToFloat(pcm);

    const now = context.currentTime;
    if (this.nextStartTime < now) {
      // Queue ran dry (or this is the first frame): restart the schedule with a
      // small lead so the next few frames have room to arrive.
      this.nextStartTime = now + JITTER_LEAD_S;
    } else if (this.nextStartTime - now > MAX_QUEUE_S) {
      // Should not happen in normal operation — see `MAX_QUEUE_S`. Loud,
      // because dropping tutor audio means the student loses teaching content.
      this.droppedFrames += 1;
      console.warn(
        `AudioPlayback: dropping a frame, ${this.queuedSeconds.toFixed(1)}s already queued ` +
          `(total dropped: ${this.droppedFrames}). This loses tutor speech.`,
      );
      return;
    }

    const buffer = context.createBuffer(1, samples.length, PLAYBACK_SAMPLE_RATE);
    // `copyToChannel` wants a Float32Array over a plain ArrayBuffer; the view
    // returned by the converter is typed over `ArrayBufferLike`, so it is
    // written through the channel's own buffer instead.
    buffer.getChannelData(0).set(samples);

    const source = context.createBufferSource();
    source.buffer = buffer;
    source.connect(context.destination);
    source.start(this.nextStartTime);
    this.nextStartTime += buffer.duration;

    // Tracked so `stop()` can cut playback immediately; a finished node removes
    // itself so the set cannot grow for the length of the session.
    this.active.add(source);
    source.onended = () => {
      this.active.delete(source);
    };
  }

  /** Seconds of audio currently scheduled but not yet played. */
  get queuedSeconds(): number {
    if (!this.context) return 0;
    return Math.max(0, this.nextStartTime - this.context.currentTime);
  }

  /**
   * Drops every scheduled frame immediately.
   *
   * This is the interruption flush: when the tutor's turn is cut short — the
   * student barges in, or the gateway reports the turn ended early — whatever is
   * still queued here belongs to a turn that is no longer happening. Leaving it
   * to play means the tutor talks over the student for up to `MAX_QUEUE_S`.
   *
   * Resetting `nextStartTime` to 0 is what makes the next frame re-seed the
   * schedule from `currentTime` instead of appending to the abandoned timeline.
   */
  stop(): void {
    for (const source of this.active) {
      try {
        source.stop();
      } catch {
        // Already finished; nothing to stop.
      }
    }
    this.active.clear();
    this.nextStartTime = 0;
  }

  async close(): Promise<void> {
    this.stop();
    if (this.context) {
      try {
        await this.context.close();
      } catch {
        // Already closed.
      }
      this.context = null;
    }
  }
}
