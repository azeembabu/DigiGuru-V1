/**
 * Microphone capture: `getUserMedia` -> `AudioWorklet` -> 20 ms PCM16 frames.
 *
 * Responsibilities kept deliberately narrow: open the device, frame the samples,
 * run the VAD, and hand finished frames to a callback. It does not know what a
 * WebSocket is — `session-client.ts` decides whether a frame is sent.
 *
 * Two things `.claude/rules/realtime-audio.md` demands that are easy to get
 * wrong and are therefore explicit here:
 *
 * * the `AudioContext` is created at 16 kHz, so no resampling is needed and the
 *   frame maths stays exact,
 * * teardown is explicit and complete — "a leaked socket burns quota and API
 *   budget", and a leaked mic track also leaves the browser's recording
 *   indicator on, which users reasonably read as spying.
 */

import { FRAME_SAMPLES, FrameAccumulator, SAMPLE_RATE, Vad, floatToPcm16 } from "./vad";
import type { VadEvent } from "./vad";

export interface AudioCaptureHandlers {
  /** One complete 20 ms PCM16 frame, little-endian, ready for the socket. */
  /**
   * One 20 ms frame, with its **unclipped** RMS.
   *
   * The level is passed alongside rather than being re-derived by the caller
   * because it has already been computed here, and because `onLevel` is scaled
   * and clamped for a meter — which throws away exactly the headroom that
   * distinguishes a person talking from speaker bleed.
   */
  onFrame: (frame: Int16Array, level: number) => void;
  onVad?: (event: VadEvent) => void;
  /** Input level 0..1, for a meter. Called once per frame; cheap to ignore. */
  onLevel?: (level: number) => void;
  onError?: (error: Error) => void;
}

/** Where the worklet module is served from. See `public/worklets/pcm-capture.js`. */
const WORKLET_URL = "/worklets/pcm-capture.js";

export class AudioCapture {
  private context: AudioContext | null = null;
  private stream: MediaStream | null = null;
  private source: MediaStreamAudioSourceNode | null = null;
  private worklet: AudioWorkletNode | null = null;

  private readonly accumulator = new FrameAccumulator(FRAME_SAMPLES);
  private readonly vad = new Vad();
  private running = false;

  constructor(private readonly handlers: AudioCaptureHandlers) {}

  get isRunning(): boolean {
    return this.running;
  }

  /** True while the student is mid-utterance, per the VAD's hangover. */
  get isSpeaking(): boolean {
    return this.vad.isSpeaking;
  }

  /**
   * Opens the mic and starts framing.
   *
   * Throws on permission denial or an unsupported browser rather than failing
   * silently — the classroom has to tell the student why it cannot hear them.
   */
  async start(): Promise<void> {
    if (this.running) return;

    if (typeof navigator === "undefined" || !navigator.mediaDevices?.getUserMedia) {
      throw new Error("This browser cannot capture audio.");
    }

    // Browser-side cleanup is enabled because the tutor's own voice coming back
    // through the student's speakers would otherwise keep the VAD permanently
    // open and make turn-taking impossible.
    this.stream = await navigator.mediaDevices.getUserMedia({
      audio: {
        channelCount: 1,
        echoCancellation: true,
        noiseSuppression: true,
        autoGainControl: true,
      },
      video: false,
    });

    // Asking for 16 kHz directly avoids a resample step. A browser may still
    // hand back its own rate, which is why `actualSampleRate` is surfaced below
    // rather than assumed.
    this.context = new AudioContext({ sampleRate: SAMPLE_RATE });
    await this.context.audioWorklet.addModule(WORKLET_URL);

    this.source = this.context.createMediaStreamSource(this.stream);
    this.worklet = new AudioWorkletNode(this.context, "pcm-capture");

    this.worklet.port.onmessage = (event: MessageEvent<Float32Array>) => {
      this.handleBlock(event.data);
    };

    // Connected to the worklet only — NOT to `context.destination`. Routing the
    // mic to the speakers would feed the student's own voice back at them.
    this.source.connect(this.worklet);

    this.running = true;
  }

  private handleBlock(block: Float32Array): void {
    try {
      for (const frame of this.accumulator.push(block)) {
        const event = this.vad.push(frame);
        if (event && this.handlers.onVad) this.handlers.onVad(event);
        const level = rmsOf(frame);
        if (this.handlers.onLevel) {
          // Reuse the VAD's own measure so the meter and the gate agree.
          this.handlers.onLevel(Math.min(1, level * 8));
        }
        this.handlers.onFrame(floatToPcm16(frame), level);
      }
    } catch (error) {
      this.handlers.onError?.(error instanceof Error ? error : new Error(String(error)));
    }
  }

  /** The rate the browser actually gave us, which may not be what we asked for. */
  get actualSampleRate(): number | null {
    return this.context?.sampleRate ?? null;
  }

  /**
   * Releases the device and the audio graph.
   *
   * Ordered so the track stops before the context closes: closing first leaves
   * the track live for a moment, which is long enough for the recording
   * indicator to linger and look like a leak.
   */
  async stop(): Promise<void> {
    this.running = false;

    if (this.worklet) {
      this.worklet.port.onmessage = null;
      this.worklet.disconnect();
      this.worklet = null;
    }
    if (this.source) {
      this.source.disconnect();
      this.source = null;
    }
    if (this.stream) {
      for (const track of this.stream.getTracks()) track.stop();
      this.stream = null;
    }
    if (this.context) {
      try {
        await this.context.close();
      } catch {
        // Already closed — not worth surfacing during teardown.
      }
      this.context = null;
    }
    this.accumulator.reset();
    this.vad.reset();
  }
}

function rmsOf(samples: Float32Array): number {
  let sum = 0;
  for (let i = 0; i < samples.length; i += 1) sum += samples[i] * samples[i];
  return samples.length === 0 ? 0 : Math.sqrt(sum / samples.length);
}
