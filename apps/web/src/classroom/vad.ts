/**
 * Voice activity detection and PCM framing.
 *
 * Pure functions and one small state machine, no Web Audio types — so the
 * turn-taking rules in `.claude/rules/pedagogy.md` and the VAD tuning in
 * `.claude/rules/realtime-audio.md` can be tested without a browser, which is
 * the only way to test a 300 ms hangover in milliseconds rather than seconds.
 *
 * Two rules from the project docs are encoded here and must not be softened:
 *
 * * **16 kHz mono PCM16, 20 ms frames** — so a frame is exactly 320 samples.
 *   Partial frames are never emitted (`realtime-audio.md`: "Frames are aligned
 *   and buffered cleanly before upstream — no partial frames, no clipping").
 * * **300 ms hangover** — speech is not considered over until the signal has
 *   been below the floor continuously for 300 ms, "so ambient noise does not
 *   open a turn". `pedagogy.md` then forbids the model from starting before
 *   that boundary, which is why end-of-speech is a deliberate, late decision
 *   rather than an eager one.
 */

export const SAMPLE_RATE = 16_000;
export const FRAME_MS = 20;
/** 16 kHz × 20 ms = 320 samples per frame. */
export const FRAME_SAMPLES = (SAMPLE_RATE * FRAME_MS) / 1000;
export const HANGOVER_MS = 300;

/** Root-mean-square of a block of normalised (-1..1) samples. */
export function rms(samples: Float32Array): number {
  if (samples.length === 0) return 0;
  let sum = 0;
  for (let i = 0; i < samples.length; i += 1) {
    sum += samples[i] * samples[i];
  }
  return Math.sqrt(sum / samples.length);
}

/**
 * Converts normalised float samples to little-endian PCM16.
 *
 * Clamped before scaling: a sample slightly outside -1..1 (which happens with
 * gain applied upstream) would otherwise wrap to the opposite sign and arrive as
 * a loud click.
 */
export function floatToPcm16(samples: Float32Array): Int16Array {
  const out = new Int16Array(samples.length);
  for (let i = 0; i < samples.length; i += 1) {
    const clamped = Math.max(-1, Math.min(1, samples[i]));
    out[i] = Math.round(clamped < 0 ? clamped * 0x8000 : clamped * 0x7fff);
  }
  return out;
}

/** Converts little-endian PCM16 back to normalised floats, for playback. */
export function pcm16ToFloat(pcm: Int16Array): Float32Array {
  const out = new Float32Array(pcm.length);
  for (let i = 0; i < pcm.length; i += 1) {
    out[i] = pcm[i] / 0x8000;
  }
  return out;
}

/**
 * Accumulates arbitrary-length sample blocks into exact 20 ms frames.
 *
 * An `AudioWorklet` hands over 128-sample render quanta, which do not divide
 * into 320, so without this every frame would be ragged. Leftover samples are
 * carried to the next call and never dropped.
 */
export class FrameAccumulator {
  private pending: number[] = [];

  constructor(private readonly frameSamples: number = FRAME_SAMPLES) {}

  /** Pushes a block and returns however many whole frames are now complete. */
  push(block: Float32Array): Float32Array[] {
    for (let i = 0; i < block.length; i += 1) {
      this.pending.push(block[i]);
    }
    const frames: Float32Array[] = [];
    while (this.pending.length >= this.frameSamples) {
      frames.push(new Float32Array(this.pending.splice(0, this.frameSamples)));
    }
    return frames;
  }

  /** Samples held back because they do not yet make a whole frame. */
  get pendingSamples(): number {
    return this.pending.length;
  }

  reset(): void {
    this.pending = [];
  }
}

export type VadEvent = "speech_start" | "speech_end";

export interface VadOptions {
  /**
   * Energy above which a frame counts as speech. Calibrated from the room's
   * noise floor rather than fixed, because a laptop fan and a quiet room differ
   * by more than any single constant can cover.
   */
  ///
  /// Setting it explicitly disables the hysteresis and uses one fixed level
  /// for both opening and closing a turn — which is what the tests want, and
  /// almost never what a real room wants.
  threshold?: number;
  hangoverMs?: number;
  frameMs?: number;
}

/**
 * Energy-gated VAD with a trailing hangover.
 *
 * Deliberately simple and deterministic: the gateway is the authority on turn
 * boundaries, and this only has to decide when to *start* and *stop sending*.
 * A heavier model on the main thread would cost exactly the latency budget
 * `realtime-audio.md` is protecting.
 */
export class Vad {
  private speaking = false;
  private silenceMs = 0;
  private noiseFloor = 0.005;
  private calibrationFrames = 0;

  private readonly threshold: number | undefined;
  private readonly hangoverMs: number;
  private readonly frameMs: number;

  constructor(options: VadOptions = {}) {
    this.threshold = options.threshold;
    this.hangoverMs = options.hangoverMs ?? HANGOVER_MS;
    this.frameMs = options.frameMs ?? FRAME_MS;
  }

  /** True while a turn is open (speech detected and hangover not yet elapsed). */
  get isSpeaking(): boolean {
    return this.speaking;
  }

  /** The adaptive floor, exposed for a level meter and for tests. */
  get floor(): number {
    return this.noiseFloor;
  }

  /**
   * Feeds one frame and returns a transition, or `null` if nothing changed.
   *
   * The first 25 frames (half a second) only calibrate the noise floor and can
   * never open a turn: otherwise the very first breath or keystroke after the
   * mic opens starts a turn, which is the single most common VAD complaint.
   */
  push(frame: Float32Array): VadEvent | null {
    const energy = rms(frame);

    if (this.calibrationFrames < 25) {
      this.calibrationFrames += 1;
      // Running mean of the quietest observed level.
      this.noiseFloor = this.noiseFloor * 0.9 + energy * 0.1;
      return null;
    }

    // Hysteresis: it takes more energy to *open* a turn than to keep one open.
    //
    // With a single threshold, a room whose noise sits near it flips the
    // detector on and off every few frames — a fan, a distant conversation, or
    // the tutor's own voice through the speakers. On screen that was a
    // "hearing you" indicator blinking continuously while the student sat in
    // silence, and underneath it the same flapping fed the barge-in counter,
    // so room noise could cut the tutor off.
    //
    // Two thresholds fix it at the source: cross the higher one to start,
    // fall below the lower one to stop. Anything between is "carry on as you
    // were", which is what a steady noise floor now does — nothing.
    // Softened from 4.5x/2.5x: together with the bleed test in the classroom
    // that made a normal speaking voice fail to register, so the student had to
    // raise their voice to be heard at all. The hysteresis gap is what stops
    // the flapping, not the absolute height of the pair.
    const openAt = this.threshold ?? Math.max(this.noiseFloor * 3.2, 0.009);
    const closeAt = this.threshold ?? Math.max(this.noiseFloor * 2.0, 0.006);
    const isSpeech = this.speaking ? energy > closeAt : energy > openAt;

    if (isSpeech) {
      this.silenceMs = 0;
      if (!this.speaking) {
        this.speaking = true;
        return "speech_start";
      }
      return null;
    }

    // Not speech. The floor tracks the room, asymmetrically: it falls quickly
    // when the room goes quiet and rises very slowly when it does not.
    //
    // It used to only ever fall (`Math.min` of the blend), which looks
    // conservative and is the opposite. Any transient dip — a pause between
    // sentences, a moment with the fan off — permanently lowered the bar, and
    // nothing could raise it again, so across a twenty-minute lesson the floor
    // converged on the quietest frame ever observed and `openAt` pinned itself
    // to the absolute minimum below. The detector therefore grew steadily more
    // trigger-happy the longer a student sat still, which is exactly backwards
    // and shows up as "hearing you" with nobody speaking.
    //
    // Rising is deliberately ~50x slower than falling, and is capped at the
    // open threshold: a noisy room may raise the bar towards the level it
    // would take to open a turn, but never past it, so the floor can never
    // climb into speech territory and deafen the detector to the student.
    const settling = this.noiseFloor * 0.95 + energy * 0.05;
    this.noiseFloor =
      settling < this.noiseFloor
        ? settling
        : Math.min(this.noiseFloor * 0.999 + energy * 0.001, openAt);

    if (this.speaking) {
      this.silenceMs += this.frameMs;
      if (this.silenceMs >= this.hangoverMs) {
        this.speaking = false;
        this.silenceMs = 0;
        return "speech_end";
      }
    }
    return null;
  }

  reset(): void {
    this.speaking = false;
    this.silenceMs = 0;
    this.calibrationFrames = 0;
    this.noiseFloor = 0.005;
  }
}
