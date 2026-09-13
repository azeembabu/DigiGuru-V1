/**
 * Mic capture worklet: forwards raw mono render quanta to the main thread.
 *
 * `.claude/rules/realtime-audio.md` requires `AudioWorklet`, never
 * `ScriptProcessorNode` — a ScriptProcessor runs on the main thread, so a canvas
 * paint or a React render stalls capture and clips the student's speech.
 *
 * Deliberately thin. Framing to 20 ms, PCM16 conversion and VAD all live in
 * testable modules (`src/classroom/vad.ts`); doing them here would put that
 * logic somewhere no unit test can reach, for no latency gain — a `postMessage`
 * of 128 floats every 8 ms is not the bottleneck.
 *
 * Served as a static file rather than bundled because `addModule()` takes a URL
 * that must be fetched at runtime, and a hashed bundle path is not stable enough
 * to hard-code.
 */

class PcmCaptureProcessor extends AudioWorkletProcessor {
  process(inputs) {
    const input = inputs[0];
    // No input connected yet (or the track ended): keep the node alive, since
    // returning false would permanently remove it from the graph.
    if (!input || input.length === 0) return true;

    const channel = input[0];
    if (!channel || channel.length === 0) return true;

    // Copy: the underlying buffer is reused by the audio thread on the next
    // quantum, so posting it without copying would deliver mutated samples.
    this.port.postMessage(new Float32Array(channel));
    return true;
  }
}

registerProcessor("pcm-capture", PcmCaptureProcessor);
