//! The Gemini Live `BidiGenerateContent` wire protocol: the JSON this crate
//! sends upstream, the JSON it parses coming back, and the `board_ops`
//! function-tool declaration that makes NN-1 possible at all.
//!
//! Everything in here is pure — no sockets, no tokio. That is deliberate:
//! every frame shape the real client has to cope with is testable from a
//! string fixture, with no network and no `GEMINI_API_KEY`
//! (`.claude/rules/testing.md`: "No test may require the network or the key").
//!
//! ## Sample rates — they differ, and mixing them up is the classic bug
//!
//! * **Upstream** (student mic): PCM16 mono **16 000 Hz**, mime
//!   `audio/pcm;rate=16000`.
//! * **Downstream** (tutor speech): PCM16 mono **24 000 Hz**.
//!
//! Both rates are verified against the live service, not assumed. This crate
//! does *not* resample in either direction; it only labels correctly. A client
//! that plays the 24 kHz tutor audio through a 16 kHz context renders the tutor
//! slow and deep.

use crate::board::{BoardOp, BoardOpsMessage};
use serde_json::{json, Value};

/// Model id, as established from the product owner's working script.
pub const DEFAULT_MODEL: &str = "models/gemini-3.1-flash-live-preview";

/// Prebuilt voice used for the tutor.
pub const DEFAULT_VOICE: &str = "Zephyr";

/// Upstream (student capture) sample rate, Hz. Mirrors the client's
/// `AudioWorklet` capture rate in `.claude/rules/realtime-audio.md`.
pub const INPUT_SAMPLE_RATE_HZ: u32 = 16_000;

/// Downstream (tutor speech) sample rate, Hz. **Not** the same as
/// [`INPUT_SAMPLE_RATE_HZ`] — see the module docs. Verified against the live
/// service, which labels its audio parts `mimeType=audio/pcm;rate=24000`.
pub const OUTPUT_SAMPLE_RATE_HZ: u32 = 24_000;

/// Context-window compression trigger, in tokens.
pub const COMPRESSION_TRIGGER_TOKENS: u64 = 104_857;

/// Sliding-window target after compression, in tokens.
pub const COMPRESSION_TARGET_TOKENS: u64 = 52_428;

/// The tool name the model must call to put anything on the whiteboard.
pub const BOARD_OPS_TOOL: &str = "board_ops";

/// WebSocket endpoint. The key is appended as a query parameter by
/// [`endpoint_url`]; it is never stored in source.
const ENDPOINT_BASE: &str = "wss://generativelanguage.googleapis.com/ws/\
     google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent";

/// Full connect URL for `api_key`.
///
/// The Live API takes the credential as a query parameter — there is no
/// header form — so this string is secret. It is built at connect time and
/// never logged (`.claude/rules/security.md`).
pub fn endpoint_url(api_key: &str) -> String {
    format!("{ENDPOINT_BASE}?key={api_key}")
}

/// The `board_ops` **function declaration**.
///
/// This is the whole reason NN-1 can be enforced: the gateway's `SyncGate`
/// only holds a turn's audio once it has seen that turn's board ops, so the
/// board must arrive as a structured tool call the gateway can observe —
/// never as prose the model narrates. The parameter schema mirrors
/// [`crate::board::BoardOp`] (`.claude/rules/whiteboard-sync.md` "Op
/// schema") exactly, because the args are deserialised straight into it.
///
/// `seq` is deliberately **not** a parameter: turn sequencing is the
/// server's, assigned monotonically by the client (see
/// [`crate::gemini_client::GeminiLiveSessionClient`]). A model-supplied
/// sequence number could repeat or go backwards, and NN-1 depends on `seq`
/// being monotonic per session.
pub fn board_ops_declaration() -> Value {
    json!({
        "name": BOARD_OPS_TOOL,
        "description": "Draw the visual for the turn you are about to speak on the \
    student's whiteboard. You MUST call this before explaining anything: the student \
    hears nothing until the board for the turn has been drawn. Use only content from \
    the retrieved curriculum context in the system instruction.",
        "parameters": {
            "type": "object",
            "properties": {
                "clear_first": {
                    "type": "boolean",
                    "description": "Wipe the board before drawing. True on a block, \
    chapter or topic boundary, so stale content never carries across topics."
                },
                "supplementary": {
                    "type": "boolean",
                    "description": "Set TRUE when what you are about to write is NOT \
    in the student's textbook — an example, formula, chart or explanation you are giving \
    from general academic knowledge because the curriculum material does not cover what \
    was asked. The board then shows a standing notice telling the student this content \
    is not from their textbook. Never leave it false for such content, and never pass \
    off outside material as the textbook's."
                },
                "ops": {
                    "type": "array",
                    "description": "Ordered whiteboard operations for this turn.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "op": {
                                "type": "string",
                                "enum": ["heading", "bullets", "math", "draw", "image",
                                         "highlight", "bar_chart", "pie_chart", "flow"],
                                "description": "Which kind of operation this is, and it \
    decides which other fields you MUST also set: heading needs text and page; bullets needs \
    items (a non-empty array of strings); math needs latex; draw needs shape, from and to; \
    image needs ref; highlight needs target; bar_chart and pie_chart need title and series; \
    flow needs title and steps. An op missing its required field is discarded."
                            },
                            "text": {
                                "type": "string",
                                "description": "heading: the heading text."
                            },
                            "page": {
                                "type": "integer",
                                "description": "heading: the textbook page this heading \
    is cited from. Must be 1 or greater and must come from the retrieved chunk payload."
                            },
                            "items": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "bullets: one string per bullet, non-empty."
                            },
                            "latex": {
                                "type": "string",
                                "description": "math: a LaTeX expression."
                            },
                            "shape": {
                                "type": "string",
                                "description": "draw: shape primitive, e.g. arrow, line, box."
                            },
                            "from": {
                                "type": "array",
                                "items": { "type": "number" },
                                "description": "draw: [x, y] start point."
                            },
                            "to": {
                                "type": "array",
                                "items": { "type": "number" },
                                "description": "draw: [x, y] end point."
                            },
                            "ref": {
                                "type": "string",
                                "description": "image: a figure reference in the exact \
    form doc:<uuid>#p<page>-fig<n>, taken from the retrieved context. Never invent one."
                            },
                            "target": {
                                "type": "string",
                                "description": "highlight: identifier of an element \
    emitted earlier in this session."
                            },
                            "title": {
                                "type": "string",
                                "description": "bar_chart, pie_chart, flow: a short \
    caption saying what the picture shows, e.g. \"Forest cover by state (%)\"."
                            },
                            "series": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "label": { "type": "string" },
                                        "value": { "type": "number" }
                                    },
                                    "required": ["label", "value"]
                                },
                                "description": "bar_chart, pie_chart: 2 to 8 labelled \
    quantities. Use the real figures from the retrieved context — never invent or round a \
    number to make a picture look tidier. Values must be zero or more; give the raw amounts \
    and the board works out the proportions itself."
                            },
                            "steps": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "flow: 2 to 6 short stages of a process, \
    in order, each a few words. The board draws them as boxes joined by arrows."
                            }
                        },
                        "required": ["op"]
                    }
                }
            },
            "required": ["ops"]
        }
    })
}

/// Everything the caller has to decide before a Live session can open.
///
/// **The caller owns grounding.** `system_instruction` is passed in
/// wholesale and this crate neither inspects nor augments it. NN-4 (RAG-only
/// teaching: retrieved chunks, citation of `chapter`/`topic`/`page`, and
/// explicit abstention below the similarity floor) is built by the gateway,
/// which is the layer with `crates/rag` and the student's academic context.
/// A `GeminiLiveSessionClient` handed an ungrounded instruction will happily
/// open a session — there is nothing here that can detect it, which is why
/// it is the gateway's responsibility.
#[derive(Debug, Clone)]
pub struct GeminiLiveConfig {
    /// `GEMINI_API_KEY`. Never logged, never serialised into anything but
    /// the connect URL.
    pub api_key: String,
    /// The full system instruction, already grounded by the caller.
    pub system_instruction: String,
    /// Override the model id; `None` uses [`DEFAULT_MODEL`].
    pub model: Option<String>,
    /// Override the prebuilt voice; `None` uses [`DEFAULT_VOICE`].
    pub voice: Option<String>,
    /// BCP-47 language for synthesis, e.g. `ml-IN`.
    ///
    /// Set from the student's recorded locale. This is what actually makes the
    /// tutor *speak* Malayalam — asking for it in the system instruction alone
    /// leaves the synthesiser guessing from the text, which is how English and
    /// Malayalam ended up mixed in one sentence. Verified accepted by the live
    /// service inside `speechConfig`.
    pub language_code: Option<String>,
}

impl GeminiLiveConfig {
    /// The common case: a key plus a grounded system instruction.
    pub fn new(api_key: impl Into<String>, system_instruction: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            system_instruction: system_instruction.into(),
            model: None,
            voice: None,
            language_code: None,
        }
    }

    /// Sets the synthesis language from a student locale such as `ml-IN`.
    pub fn with_language(mut self, locale: impl Into<String>) -> Self {
        self.language_code = Some(locale.into());
        self
    }

    pub fn model_id(&self) -> &str {
        self.model.as_deref().unwrap_or(DEFAULT_MODEL)
    }

    pub fn voice_name(&self) -> &str {
        self.voice.as_deref().unwrap_or(DEFAULT_VOICE)
    }
}

/// The first message on the socket: `{"setup": {...}}`.
///
/// Carries the model, audio-only response modality, the voice, the thinking
/// level, context-window compression, the caller's system instruction, and
/// the `board_ops` tool declaration.
/// `speechConfig`, carrying `languageCode` only when the caller knows one —
/// sending a null/empty code is worse than omitting the field.
fn speech_config(config: &GeminiLiveConfig) -> Value {
    let mut cfg = json!({
        "voiceConfig": { "prebuiltVoiceConfig": { "voiceName": config.voice_name() } }
    });
    if let (Some(code), Some(obj)) = (config.language_code.as_deref(), cfg.as_object_mut()) {
        if !code.trim().is_empty() {
            obj.insert("languageCode".to_string(), json!(code));
        }
    }
    cfg
}

/// How long the student may pause mid-sentence before Live decides the
/// utterance is finished and answers.
///
/// The default is short enough that "What is the..." followed by a breath is
/// treated as a complete question, and the tutor answers a fragment it had to
/// guess at. 1200 ms is a natural thinking pause in speech and well past the
/// gaps inside one sentence, so the student gets to finish.
///
/// It is the *only* mechanical lever on "wait for the complete utterance": the
/// decision is made inside Live, before anything reaches this process, so no
/// amount of prompting can move it.
const SILENCE_BEFORE_REPLY_MS: i64 = 1200;

/// Audio kept from just before speech was detected, so a turn does not begin
/// clipped.
const PREFIX_PADDING_MS: i64 = 300;

/// Turn detection, tuned to answer questions rather than noises.
///
/// `START_SENSITIVITY_LOW` demands stronger evidence before treating sound as
/// the start of speech — a cough, a keyboard, a chair, someone talking in the
/// next room. `END_SENSITIVITY_LOW` makes it slower to declare the student
/// finished, which together with `silenceDurationMs` is what stops the tutor
/// interrupting a sentence that was still being formed.
///
/// `activityHandling` is deliberately left at its default (interruption
/// enabled). Barge-in is how a student stops a tutor that is talking too long,
/// and the classroom depends on it — see the client's own VAD barge-in.
fn realtime_input_config() -> Value {
    json!({
        "automaticActivityDetection": {
            "startOfSpeechSensitivity": "START_SENSITIVITY_LOW",
            "endOfSpeechSensitivity": "END_SENSITIVITY_LOW",
            "prefixPaddingMs": PREFIX_PADDING_MS,
            "silenceDurationMs": SILENCE_BEFORE_REPLY_MS
        }
    })
}

pub fn setup_message(config: &GeminiLiveConfig) -> Value {
    json!({
        "setup": {
            "model": config.model_id(),
            "generationConfig": {
                "responseModalities": ["AUDIO"],
                "speechConfig": speech_config(config),
                "thinkingConfig": { "thinkingLevel": "MINIMAL" }
            },
            "systemInstruction": {
                "parts": [ { "text": config.system_instruction } ]
            },
            "tools": [ { "functionDeclarations": [ board_ops_declaration() ] } ],
            // Turn detection. Verified accepted against the live endpoint
            // before being relied on — a setup the service rejects closes the
            // socket with 1007 and the classroom falls back to defaults with
            // no visible error.
            "realtimeInputConfig": realtime_input_config(),
            // Both directions, both required by a non-negotiable and neither
            // available any other way. The student's words are what NN-4
            // per-turn RAG grounding retrieves against, and the Tier-1
            // guardrail in `.claude/rules/security.md` is defined as exactly
            // "Live input-transcription events" -- without this the gateway
            // cannot see what the student said at all. The output side lets
            // Tier-2 screen what the tutor is about to say.
            "inputAudioTranscription": {},
            "outputAudioTranscription": {},
            "contextWindowCompression": {
                "triggerTokens": COMPRESSION_TRIGGER_TOKENS.to_string(),
                "slidingWindow": { "targetTokens": COMPRESSION_TARGET_TOKENS.to_string() }
            }
        }
    })
}

/// One 20 ms frame of student mic audio, base64-encoded, labelled at the
/// **input** rate (16 kHz). The bytes are PCM16 mono little-endian exactly as
/// captured; no resampling happens in this crate.
///
/// The payload is `realtimeInput.audio`, a **single object** — not
/// `realtimeInput.mediaChunks`, and not an array. Verified against the live
/// service: `media_chunks` is deprecated and the server closes the socket with
/// code `1007` and `"realtime_input.media_chunks is deprecated. Use audio,
/// video, or text instead."`. Getting this wrong is invisible at connect and
/// kills the session the instant the student first speaks, so the shape is
/// pinned by a test below.
pub fn realtime_audio_message(frame: &[u8]) -> Value {
    use base64::Engine as _;
    let data = base64::engine::general_purpose::STANDARD.encode(frame);
    json!({
        "realtimeInput": {
            "audio": {
                "mimeType": format!("audio/pcm;rate={INPUT_SAMPLE_RATE_HZ}"),
                "data": data
            }
        }
    })
}

/// Acknowledge a `board_ops` tool call so the model goes on to narrate the
/// turn instead of waiting on its function result.
///
/// `accepted` is false when the op failed [`crate::board::validate`]; the
/// model is told so it can re-emit a well-formed board rather than silently
/// teaching with no visual.
/// A text turn from "the user" side of the conversation.
///
/// Two uses, both gateway-driven, neither of which the student types:
///
/// * the **kickoff** after `session_ready` — the greeting on a first login
///   (NN-2), or a "continue from chapter/page" prompt on a resume. Without it the
///   tutor sits silent until the student speaks first, which a first-time
///   student has no way of knowing to do;
/// * **per-turn curriculum context** (NN-4). `systemInstruction` cannot be
///   changed once a Live session is set up, so freshly retrieved chunks for the
///   student's latest question are injected as a turn instead. Sent with
///   `turn_complete: false` so the model waits for the student's real utterance
///   rather than answering the context itself.
///
/// Shape verified against the live service: `clientContent.turns[].parts[].text`
/// with `turnComplete`.
pub fn client_text_turn_message(text: &str, turn_complete: bool) -> Value {
    json!({
        "clientContent": {
            "turns": [{ "role": "user", "parts": [{ "text": text }] }],
            "turnComplete": turn_complete
        }
    })
}

pub fn tool_response_message(call_id: Option<&str>, accepted: bool) -> Value {
    let mut response = json!({
        "name": BOARD_OPS_TOOL,
        "response": {
            "result": if accepted { "rendered" } else { "rejected" },
            "detail": if accepted {
                "The board for this turn is on screen. Narrate it now."
            } else {
                "The board ops failed schema validation and were dropped. Call \
    board_ops again with a valid payload before continuing."
            }
        }
    });
    if let (Some(id), Some(obj)) = (call_id, response.as_object_mut()) {
        obj.insert("id".to_string(), json!(id));
    }
    json!({ "toolResponse": { "functionResponses": [ response ] } })
}

/// A single function call the model made.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    /// Upstream call id, echoed back in the tool response when present.
    pub id: Option<String>,
    pub name: String,
    /// Raw argument object.
    pub args: Value,
}

/// Which side of the conversation a transcript belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptSource {
    /// The student's speech, as transcribed by the Live service. This is the
    /// Tier-1 guardrail's input (`.claude/rules/security.md`) and what NN-4
    /// per-turn retrieval is run against.
    Input,
    /// The tutor's own speech. Tier-2 output screening reads this.
    Output,
}

/// A parsed upstream frame, before it is turned into a
/// [`crate::gemini_client::LiveModelEvent`].
///
/// Unknown frames become [`ServerFrame::Ignored`] rather than an error —
/// the Live API adds message kinds (`sessionResumptionUpdate`,
/// `usageMetadata`, …) and an unrecognised one must not end a student's
/// lesson.
#[derive(Debug, Clone, PartialEq)]
pub enum ServerFrame {
    /// `{"setupComplete": {}}` — the session is ready for audio.
    SetupComplete,
    /// `{"toolCall": {"functionCalls": [...]}}`
    ToolCalls(Vec<ToolCall>),
    /// Inline PCM16 mono **24 kHz** audio from `serverContent.modelTurn`.
    Audio(Vec<u8>),
    /// `serverContent.inputTranscription` / `.outputTranscription` text.
    Transcript {
        source: TranscriptSource,
        text: String,
    },
    /// `serverContent.turnComplete`
    TurnComplete,
    /// `serverContent.interrupted` — the student spoke over the tutor. The
    /// downstream audio queue must be flushed or the old turn keeps playing.
    Interrupted,
    /// `goAway` — the upstream is about to close this connection.
    GoAway,
    /// A recognised-but-uninteresting or unknown frame.
    Ignored,
}

/// Parse one text frame from the Live API.
///
/// Returns a list because a single `serverContent` frame can carry several
/// audio parts, and because `interrupted` and `turnComplete` can arrive in
/// the same frame. Order within the returned list is the order the upstream
/// put them in.
///
/// A frame that is not JSON at all is a [`crate::error::LiveError::Malformed`];
/// a frame that is JSON but unrecognised yields
/// `[ServerFrame::Ignored]`. The distinction matters: the first is a real
/// protocol fault worth surfacing, the second is forward compatibility.
pub fn parse_server_frame(text: &str) -> crate::error::Result<Vec<ServerFrame>> {
    let value: Value = serde_json::from_str(text)
        .map_err(|e| crate::error::LiveError::Malformed(format!("live frame is not JSON: {e}")))?;

    let mut frames = Vec::new();

    if value.get("setupComplete").is_some() || value.get("setup_complete").is_some() {
        frames.push(ServerFrame::SetupComplete);
    }

    if let Some(tool_call) = value.get("toolCall").or_else(|| value.get("tool_call")) {
        let calls = tool_call
            .get("functionCalls")
            .or_else(|| tool_call.get("function_calls"))
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .map(|c| ToolCall {
                        id: c.get("id").and_then(Value::as_str).map(str::to_string),
                        name: c
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        args: c.get("args").cloned().unwrap_or(Value::Null),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        frames.push(ServerFrame::ToolCalls(calls));
    }

    if let Some(server_content) = value
        .get("serverContent")
        .or_else(|| value.get("server_content"))
    {
        // Transcripts before audio: the student's transcribed words are the
        // Tier-1 guardrail's input, so the gateway must get the chance to mute
        // or tear down the turn before any of the tutor's audio for it is
        // released (NN-5).
        for (key, snake, source) in [
            (
                "inputTranscription",
                "input_transcription",
                TranscriptSource::Input,
            ),
            (
                "outputTranscription",
                "output_transcription",
                TranscriptSource::Output,
            ),
        ] {
            if let Some(text) = server_content
                .get(key)
                .or_else(|| server_content.get(snake))
                .and_then(|t| t.get("text"))
                .and_then(Value::as_str)
            {
                if !text.is_empty() {
                    frames.push(ServerFrame::Transcript {
                        source,
                        text: text.to_string(),
                    });
                }
            }
        }

        // Audio next, then the turn-boundary flags: a frame carrying both a
        // final audio part and `turnComplete` must deliver the audio before
        // the gateway is told the turn is over.
        if let Some(parts) = server_content
            .get("modelTurn")
            .or_else(|| server_content.get("model_turn"))
            .and_then(|t| t.get("parts"))
            .and_then(Value::as_array)
        {
            for part in parts {
                if let Some(inline) = part
                    .get("inlineData")
                    .or_else(|| part.get("inline_data"))
                    .or_else(|| part.get("inlineBlob"))
                {
                    if let Some(data) = inline.get("data").and_then(Value::as_str) {
                        use base64::Engine as _;
                        match base64::engine::general_purpose::STANDARD.decode(data) {
                            Ok(bytes) if !bytes.is_empty() => {
                                frames.push(ServerFrame::Audio(bytes))
                            }
                            Ok(_) => {}
                            Err(e) => {
                                // Not fatal: one corrupt part must not end the
                                // lesson. Drop it and keep the stream alive.
                                tracing::warn!(error = %e, "undecodable inline audio part dropped");
                            }
                        }
                    }
                }
            }
        }

        if server_content
            .get("interrupted")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            frames.push(ServerFrame::Interrupted);
        }

        if server_content
            .get("turnComplete")
            .or_else(|| server_content.get("turn_complete"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            frames.push(ServerFrame::TurnComplete);
        }

        if frames.is_empty() {
            frames.push(ServerFrame::Ignored);
        }
    }

    if value.get("goAway").is_some() || value.get("go_away").is_some() {
        frames.push(ServerFrame::GoAway);
    }

    if frames.is_empty() {
        frames.push(ServerFrame::Ignored);
    }

    Ok(frames)
}

/// Turn `board_ops` tool-call arguments into a validated
/// [`BoardOpsMessage`] stamped with `seq`.
///
/// `seq` comes from the caller's monotonic turn counter, not from the model
/// (see [`board_ops_declaration`]). The message is run through
/// [`crate::board::validate`], so an invalid op is an `Err` the caller drops
/// and logs — it never reaches the client
/// (`.claude/rules/whiteboard-sync.md`).
pub fn board_ops_from_args(args: &Value, seq: u32) -> crate::error::Result<BoardOpsMessage> {
    use crate::error::LiveError;

    let ops_value = args
        .get("ops")
        .ok_or_else(|| LiveError::InvalidBoardOp("board_ops call has no \"ops\"".to_string()))?;

    // Parse ops INDIVIDUALLY and skip the ones that do not fit the schema.
    //
    // `whiteboard-sync.md` is explicit that "an invalid op is dropped and
    // logged; it never reaches the client" — dropped, not fatal. Deserialising
    // the whole array in one call made it fatal: a single malformed op took the
    // entire board down with it. Observed live, the model emitted a `bullets`
    // op with no `items` (the tool schema only marks `op` itself as required,
    // so nothing forces the per-kind fields), and the result was a turn with NO
    // board at all followed by "tutor audio arrived with no preceding
    // board_ops" — the student heard an explanation of a blank whiteboard.
    //
    // One good op is worth more than none, so the good ones are kept.
    let raw_ops = ops_value.as_array().ok_or_else(|| {
        LiveError::InvalidBoardOp("board_ops \"ops\" is not an array".to_string())
    })?;

    let mut ops: Vec<BoardOp> = Vec::with_capacity(raw_ops.len());
    for (index, raw) in raw_ops.iter().enumerate() {
        match serde_json::from_value::<BoardOp>(raw.clone()) {
            Ok(op) => ops.push(op),
            Err(e) => {
                tracing::warn!(
                    index,
                    error = %e,
                    "board op did not match the schema; dropped, rest of the turn kept"
                );
            }
        }
    }

    if ops.is_empty() {
        return Err(LiveError::InvalidBoardOp(
            "board_ops had no op matching the schema".to_string(),
        ));
    }

    let msg = BoardOpsMessage {
        msg_type: "board_ops".to_string(),
        seq,
        clear_first: args
            .get("clear_first")
            .or_else(|| args.get("clearFirst"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        supplementary: args
            .get("supplementary")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        ops,
    };

    crate::board::validate(&msg)?;
    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One malformed op must not destroy the whole board.
    ///
    /// Observed live: the model emitted a `bullets` op with no `items`, the
    /// whole-array deserialise failed, and the turn rendered NO board at all —
    /// the student then heard an explanation of a blank whiteboard.
    /// `whiteboard-sync.md`: "an invalid op is dropped and logged".
    #[test]
    fn a_malformed_op_is_dropped_and_the_rest_of_the_turn_survives() {
        let args = serde_json::json!({
            "clear_first": true,
            "ops": [
                { "op": "heading", "text": "Ecosystems", "page": 3 },
                { "op": "bullets" },
                { "op": "bullets", "items": ["Producers", "Consumers"] }
            ]
        });

        let msg = super::board_ops_from_args(&args, 7).expect("the good ops survive");
        assert_eq!(msg.seq, 7);
        assert!(msg.clear_first);
        assert_eq!(msg.ops.len(), 2, "the item-less bullets op should be gone");
    }

    /// If nothing at all parses there is genuinely no board to show, and the
    /// caller must hear about it rather than forward an empty turn.
    #[test]
    fn a_turn_whose_every_op_is_malformed_is_still_an_error() {
        let args = serde_json::json!({ "ops": [ { "op": "bullets" }, { "op": "math" } ] });
        assert!(super::board_ops_from_args(&args, 1).is_err());
    }

    /// The tool schema must tell the model which fields each kind needs —
    /// a flat union can only mark `op` itself as required.
    #[test]
    fn the_tool_declaration_states_the_per_kind_required_fields() {
        let decl = super::board_ops_declaration().to_string();
        assert!(decl.contains("bullets needs items"), "got: {decl}");
        assert!(decl.contains("heading needs text and page"), "got: {decl}");
    }

    #[test]
    fn a_text_turn_is_client_content_with_the_turn_complete_flag_honoured() {
        let v = super::client_text_turn_message("Greet the student.", true);
        assert_eq!(v["clientContent"]["turns"][0]["role"], "user");
        assert_eq!(v["clientContent"]["turns"][0]["parts"][0]["text"], "Greet the student.");
        assert_eq!(v["clientContent"]["turnComplete"], true);
        // Context injection must NOT close the turn, or the model answers the
        // context instead of the student.
        let ctx = super::client_text_turn_message("CURRICULUM CONTEXT ...", false);
        assert_eq!(ctx["clientContent"]["turnComplete"], false);
    }

    /// Turn detection is configured, not left at the default.
    ///
    /// The defaults answer on any detected speech and decide an utterance has
    /// ended after a short pause — which is why the tutor replied to coughs and
    /// to half-finished sentences. This is the only place that behaviour can be
    /// changed: the decision happens inside Live, before any audio reaches this
    /// process, so it cannot be prompted away.
    #[test]
    fn setup_waits_for_a_finished_utterance_before_replying() {
        let setup = setup_message(&GeminiLiveConfig::new("k", "grounded instruction"));
        let vad = &setup["setup"]["realtimeInputConfig"]["automaticActivityDetection"];

        assert_eq!(vad["startOfSpeechSensitivity"], "START_SENSITIVITY_LOW");
        assert_eq!(vad["endOfSpeechSensitivity"], "END_SENSITIVITY_LOW");
        assert_eq!(vad["silenceDurationMs"], 1200);
        assert_eq!(vad["prefixPaddingMs"], 300);
    }

    /// Interruption stays enabled. A student must be able to talk over a tutor
    /// that is going on too long — the classroom's own VAD barge-in depends on
    /// it, and `NO_INTERRUPTION` would take that away.
    #[test]
    fn setup_leaves_barge_in_enabled() {
        let setup = setup_message(&GeminiLiveConfig::new("k", "grounded instruction"));
        assert!(
            setup["setup"]["realtimeInputConfig"]["activityHandling"].is_null(),
            "activityHandling must stay at its default; got: {}",
            setup["setup"]["realtimeInputConfig"]
        );
    }

    #[test]
    fn setup_declares_board_ops_as_a_function_tool() {
        let setup = setup_message(&GeminiLiveConfig::new("k", "grounded instruction"));
        let decls = &setup["setup"]["tools"][0]["functionDeclarations"];
        assert_eq!(decls[0]["name"], BOARD_OPS_TOOL);
        // NN-1: the board must be a tool the model calls, not prose.
        assert_eq!(decls[0]["parameters"]["required"][0], "ops");
        assert_eq!(
            setup["setup"]["generationConfig"]["responseModalities"][0],
            "AUDIO"
        );
        assert_eq!(
            setup["setup"]["generationConfig"]["speechConfig"]["voiceConfig"]
                ["prebuiltVoiceConfig"]["voiceName"],
            DEFAULT_VOICE
        );
        assert_eq!(setup["setup"]["model"], DEFAULT_MODEL);
        assert_eq!(
            setup["setup"]["systemInstruction"]["parts"][0]["text"],
            "grounded instruction"
        );
    }

    #[test]
    fn setup_does_not_contain_the_api_key() {
        let setup = setup_message(&GeminiLiveConfig::new("super-secret", "instruction"));
        assert!(!setup.to_string().contains("super-secret"));
    }

    #[test]
    fn the_two_sample_rates_are_distinct_and_upstream_is_labelled_16k() {
        assert_ne!(INPUT_SAMPLE_RATE_HZ, OUTPUT_SAMPLE_RATE_HZ);
        let msg = realtime_audio_message(&[0x01, 0x02, 0x03, 0x04]);
        assert_eq!(
            msg["realtimeInput"]["audio"]["mimeType"],
            "audio/pcm;rate=16000"
        );
        assert_eq!(msg["realtimeInput"]["audio"]["data"], "AQIDBA==");
    }

    #[test]
    fn mic_audio_never_uses_the_deprecated_media_chunks_shape() {
        // Regression guard: `realtime_input.media_chunks` is deprecated and the
        // live service closes the socket with 1007 on the first mic frame —
        // a failure that looks like a successful connect followed by a drop as
        // soon as the student speaks.
        let msg = realtime_audio_message(&[0x00]);
        assert!(msg["realtimeInput"].get("mediaChunks").is_none());
        assert!(msg["realtimeInput"]["audio"].is_object());
    }

    #[test]
    fn parses_a_tool_call_frame() {
        let text = r#"{"toolCall":{"functionCalls":[
            {"id":"fc-1","name":"board_ops","args":{"clear_first":true,
             "ops":[{"op":"heading","text":"Sandhi","page":57},
                    {"op":"bullets","items":["one","two"]}]}}]}}"#;
        let frames = parse_server_frame(text).expect("parse");
        let ServerFrame::ToolCalls(calls) = &frames[0] else {
            panic!("expected a tool call, got {frames:?}");
        };
        assert_eq!(calls[0].name, BOARD_OPS_TOOL);
        assert_eq!(calls[0].id.as_deref(), Some("fc-1"));

        let msg = board_ops_from_args(&calls[0].args, 7).expect("valid board ops");
        assert_eq!(msg.seq, 7);
        assert!(msg.clear_first);
        assert_eq!(msg.ops.len(), 2);
    }

    #[test]
    fn parses_an_audio_frame_as_raw_pcm_bytes() {
        let text = r#"{"serverContent":{"modelTurn":{"parts":[
            {"inlineData":{"mimeType":"audio/pcm;rate=24000","data":"AQIDBA=="}}]}}}"#;
        let frames = parse_server_frame(text).expect("parse");
        assert_eq!(frames, vec![ServerFrame::Audio(vec![1, 2, 3, 4])]);
    }

    #[test]
    fn audio_in_the_same_frame_as_turn_complete_comes_first() {
        let text = r#"{"serverContent":{"modelTurn":{"parts":[
            {"inlineData":{"data":"AQI="}}]},"turnComplete":true}}"#;
        let frames = parse_server_frame(text).expect("parse");
        assert_eq!(
            frames,
            vec![ServerFrame::Audio(vec![1, 2]), ServerFrame::TurnComplete]
        );
    }

    #[test]
    fn parses_an_interrupted_frame_before_turn_complete() {
        let text = r#"{"serverContent":{"interrupted":true,"turnComplete":true}}"#;
        let frames = parse_server_frame(text).expect("parse");
        assert_eq!(
            frames,
            vec![ServerFrame::Interrupted, ServerFrame::TurnComplete]
        );
    }

    #[test]
    fn parses_setup_complete_and_go_away() {
        assert_eq!(
            parse_server_frame(r#"{"setupComplete":{}}"#).expect("parse"),
            vec![ServerFrame::SetupComplete]
        );
        assert_eq!(
            parse_server_frame(r#"{"goAway":{"timeLeft":"5s"}}"#).expect("parse"),
            vec![ServerFrame::GoAway]
        );
    }

    #[test]
    fn setup_requests_both_transcription_directions() {
        let setup = setup_message(&GeminiLiveConfig::new("k", "instruction"));
        // Without these the gateway cannot see the student's words at all,
        // which blocks NN-4 per-turn grounding and the Tier-1 guardrail.
        assert!(setup["setup"]["inputAudioTranscription"].is_object());
        assert!(setup["setup"]["outputAudioTranscription"].is_object());
    }

    #[test]
    fn parses_transcription_frames_in_both_directions() {
        let frames = parse_server_frame(
            r#"{"serverContent":{"inputTranscription":{"text":"hello"},
                "outputTranscription":{"text":"Ready."}}}"#,
        )
        .expect("parse");
        assert_eq!(
            frames,
            vec![
                ServerFrame::Transcript {
                    source: TranscriptSource::Input,
                    text: "hello".to_string()
                },
                ServerFrame::Transcript {
                    source: TranscriptSource::Output,
                    text: "Ready.".to_string()
                },
            ]
        );
    }

    #[test]
    fn an_empty_transcript_produces_no_frame() {
        let frames = parse_server_frame(r#"{"serverContent":{"inputTranscription":{"text":""}}}"#)
            .expect("parse");
        assert_eq!(frames, vec![ServerFrame::Ignored]);
    }

    #[test]
    fn unknown_frame_is_ignored_not_fatal() {
        let frames = parse_server_frame(r#"{"usageMetadata":{"totalTokenCount":12}}"#)
            .expect("unknown frames must not error");
        assert_eq!(frames, vec![ServerFrame::Ignored]);
    }

    #[test]
    fn malformed_frame_is_a_typed_error_not_a_panic() {
        let err = parse_server_frame("this is not json}{").expect_err("must be an error");
        assert!(matches!(err, crate::error::LiveError::Malformed(_)));
    }

    #[test]
    fn invalid_board_ops_args_are_rejected_rather_than_forwarded() {
        // Empty `ops` — `board::validate` rejects it.
        let err = board_ops_from_args(&serde_json::json!({ "ops": [] }), 1)
            .expect_err("empty ops must be rejected");
        assert!(matches!(err, crate::error::LiveError::InvalidBoardOp(_)));

        // Missing `ops` entirely.
        let err = board_ops_from_args(&serde_json::json!({ "clear_first": true }), 1)
            .expect_err("missing ops must be rejected");
        assert!(matches!(err, crate::error::LiveError::InvalidBoardOp(_)));

        // An op whose kind is not in the schema.
        let err = board_ops_from_args(
            &serde_json::json!({ "ops": [ { "op": "teleport", "text": "x" } ] }),
            1,
        )
        .expect_err("unknown op kind must be rejected");
        assert!(matches!(err, crate::error::LiveError::InvalidBoardOp(_)));

        // A structurally-fine image op with a bogus ref.
        let err = board_ops_from_args(
            &serde_json::json!({ "ops": [ { "op": "image", "ref": "nope" } ] }),
            1,
        )
        .expect_err("bad image ref must be rejected");
        assert!(matches!(err, crate::error::LiveError::InvalidBoardOp(_)));
    }

    #[test]
    fn tool_response_echoes_the_call_id_and_reports_rejection() {
        let ok = tool_response_message(Some("fc-1"), true);
        let fr = &ok["toolResponse"]["functionResponses"][0];
        assert_eq!(fr["id"], "fc-1");
        assert_eq!(fr["name"], BOARD_OPS_TOOL);
        assert_eq!(fr["response"]["result"], "rendered");

        let bad = tool_response_message(None, false);
        let fr = &bad["toolResponse"]["functionResponses"][0];
        assert!(fr.get("id").is_none());
        assert_eq!(fr["response"]["result"], "rejected");
    }

    #[test]
    fn endpoint_is_the_v1beta_bidi_generate_content_socket() {
        let url = endpoint_url("K");
        assert!(url.starts_with("wss://generativelanguage.googleapis.com/ws/"));
        assert!(url.contains("v1beta.GenerativeService.BidiGenerateContent"));
        assert!(url.ends_with("?key=K"));
    }
}
