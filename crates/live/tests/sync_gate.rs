//! NN-1 conformance tests for the `SyncGate`.
//!
//! `testing.md` names this suite: "`SyncGate` unit tests: audio never released
//! before ACK or hold expiry". Every timing test drives a `TestClock` by hand —
//! there is not one sleep in this file.

use live::{
    AckOutcome, AudioDisposition, BoardOp, DrawShape, OpError, Point, ReleaseReason, SyncGate,
    SyncGateConfig, SyncGateError, TestClock, TurnSeq, HOLD_MAX_MS,
};

/// A 20 ms frame of 16 kHz mono PCM16 — 320 samples, 640 bytes.
const FRAME: [u8; 640] = [0u8; 640];

fn heading(id: &str) -> BoardOp {
    BoardOp::Heading {
        id: id.to_owned(),
        text: "സന്ധി — Sandhi".to_owned(),
    }
}

fn gate() -> (TestClock, SyncGate<TestClock>) {
    let clock = TestClock::new();
    (clock.clone(), SyncGate::new(clock))
}

fn released(outcome: AckOutcome) -> live::ReleasedAudio {
    match outcome {
        AckOutcome::Released(audio) => *audio,
        other => panic!("expected a release, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// The core NN-1 property.
// ---------------------------------------------------------------------------

/// Proves the non-negotiable itself: while turn N is PENDING_ACK every frame is
/// held, and the release happens on the ack — not before it.
#[test]
fn releases_audio_only_after_board_ack() {
    let (_clock, mut gate) = gate();
    let accepted = gate
        .on_board_ops(TurnSeq(1), true, vec![heading("h1")])
        .expect("valid ops open turn 1");
    assert_eq!(accepted.ops.len(), 1);
    assert!(gate.is_holding(TurnSeq(1)));

    for _ in 0..10 {
        match gate.push_audio(TurnSeq(1), &FRAME) {
            AudioDisposition::Buffered { .. } => {}
            other => panic!("audio escaped before the ack: {other:?}"),
        }
    }

    let audio = released(gate.on_board_ack(TurnSeq(1)));
    assert_eq!(audio.reason, ReleaseReason::BoardAck);
    assert_eq!(audio.frames.len(), 10, "every held frame is returned, in order");
    assert_eq!(gate.wb_violation(), 0, "an acked turn is never a violation");
}

/// After the ack the turn is OPEN, so the hot path forwards without copying.
#[test]
fn forwards_audio_without_buffering_once_turn_is_open() {
    let (_clock, mut gate) = gate();
    gate.on_board_ops(TurnSeq(1), true, vec![heading("h1")])
        .expect("valid ops");
    let _ = gate.on_board_ack(TurnSeq(1));

    assert_eq!(
        gate.push_audio(TurnSeq(1), &FRAME),
        AudioDisposition::Forward
    );
    assert!(!gate.is_holding(TurnSeq(1)));
}

/// Audio must never be released by the mere passage of time short of the
/// ceiling: at 399 ms the gate is still holding.
#[test]
fn holds_audio_until_the_very_end_of_the_hold_ceiling() {
    let (clock, mut gate) = gate();
    gate.on_board_ops(TurnSeq(1), true, vec![heading("h1")])
        .expect("valid ops");
    let _ = gate.push_audio(TurnSeq(1), &FRAME);

    clock.advance_ms(HOLD_MAX_MS - 1);
    assert!(
        gate.poll_timeouts().is_empty(),
        "released one millisecond early"
    );
    assert!(gate.is_holding(TurnSeq(1)));
    assert_eq!(gate.wb_violation(), 0);
}

/// There is no simultaneous-release mode: a turn that never opened board ops
/// has no way to get audio through the gate at all.
#[test]
fn audio_for_a_turn_with_no_board_ops_is_never_forwarded() {
    let (_clock, mut gate) = gate();
    assert_eq!(
        gate.push_audio(TurnSeq(7), &FRAME),
        AudioDisposition::UnknownTurn
    );
    assert_eq!(gate.metrics().frames_unknown_turn, 1);
}

// ---------------------------------------------------------------------------
// HOLD_MAX expiry.
// ---------------------------------------------------------------------------

/// The ceiling releases the audio *and* banks a `wb_violation` — a stalled
/// canvas must not silence the tutor, but it must be visible in CI.
#[test]
fn hold_expiry_releases_audio_and_increments_wb_violation() {
    let (clock, mut gate) = gate();
    gate.on_board_ops(TurnSeq(1), true, vec![heading("h1")])
        .expect("valid ops");
    for _ in 0..3 {
        let _ = gate.push_audio(TurnSeq(1), &FRAME);
    }

    clock.advance_ms(HOLD_MAX_MS);
    let releases = gate.poll_timeouts();
    assert_eq!(releases.len(), 1);
    assert_eq!(releases[0].reason, ReleaseReason::HoldExpired);
    assert_eq!(releases[0].frames.len(), 3);
    assert_eq!(releases[0].held_ms, HOLD_MAX_MS);
    assert_eq!(gate.wb_violation(), 1);

    // And the turn is open afterwards, so teaching continues.
    assert_eq!(
        gate.push_audio(TurnSeq(1), &FRAME),
        AudioDisposition::Forward
    );
}

/// An ack that arrives after the ceiling already fired is a no-op, not a second
/// release and not a panic.
#[test]
fn ack_arriving_after_hold_expiry_is_ignored() {
    let (clock, mut gate) = gate();
    gate.on_board_ops(TurnSeq(1), true, vec![heading("h1")])
        .expect("valid ops");
    clock.advance_ms(HOLD_MAX_MS);
    assert_eq!(gate.poll_timeouts().len(), 1);

    assert_eq!(
        gate.on_board_ack(TurnSeq(1)),
        AckOutcome::AlreadyReleased(TurnSeq(1))
    );
    assert_eq!(gate.wb_violation(), 1, "the late ack does not double-count");
}

/// The socket task schedules its timer from this; a holding gate must always
/// name a deadline, and an idle one must name none.
#[test]
fn reports_the_next_hold_deadline_for_the_caller_to_sleep_on() {
    let (clock, mut gate) = gate();
    assert_eq!(gate.next_hold_deadline_ms(), None);

    gate.on_board_ops(TurnSeq(1), true, vec![heading("h1")])
        .expect("valid ops");
    assert_eq!(gate.next_hold_deadline_ms(), Some(HOLD_MAX_MS));

    clock.advance_ms(10);
    gate.on_board_ops(TurnSeq(2), false, vec![heading("h2")])
        .expect("valid ops");
    assert_eq!(
        gate.next_hold_deadline_ms(),
        Some(HOLD_MAX_MS),
        "the earliest outstanding deadline wins"
    );

    let _ = gate.on_board_ack(TurnSeq(1));
    assert_eq!(gate.next_hold_deadline_ms(), Some(HOLD_MAX_MS + 10));
    let _ = gate.on_board_ack(TurnSeq(2));
    assert_eq!(gate.next_hold_deadline_ms(), None);
}

// ---------------------------------------------------------------------------
// board_error -> text fallback.
// ---------------------------------------------------------------------------

/// A canvas exception releases the audio and degrades the board; it does not
/// stop the voice stream and it is not an NN-1 violation.
#[test]
fn board_error_releases_audio_and_degrades_to_text_fallback() {
    let (_clock, mut gate) = gate();
    gate.on_board_ops(TurnSeq(1), true, vec![heading("h1")])
        .expect("valid ops");
    let _ = gate.push_audio(TurnSeq(1), &FRAME);

    let audio = released(gate.on_board_error(TurnSeq(1), "render_failed"));
    assert_eq!(audio.reason, ReleaseReason::BoardError);
    assert!(audio.is_text_fallback());
    assert_eq!(audio.frames.len(), 1);
    assert!(gate.is_text_fallback(TurnSeq(1)));
    assert_eq!(gate.metrics().board_errors, 1);
    assert_eq!(
        gate.wb_violation(),
        0,
        "a client render failure is not a gateway violation"
    );

    // Teaching keeps going: the next turn still works normally.
    gate.on_board_ops(TurnSeq(2), false, vec![heading("h2")])
        .expect("valid ops");
    let _ = gate.push_audio(TurnSeq(2), &FRAME);
    assert_eq!(released(gate.on_board_ack(TurnSeq(2))).frames.len(), 1);
}

// ---------------------------------------------------------------------------
// Op schema validation.
// ---------------------------------------------------------------------------

/// An op the schema refuses is dropped and logged, and never appears in the
/// vector the caller forwards.
#[test]
fn invalid_op_is_dropped_and_never_forwarded() {
    let (_clock, mut gate) = gate();
    let accepted = gate
        .on_board_ops(
            TurnSeq(1),
            true,
            vec![
                heading("h1"),
                BoardOp::Bullets {
                    id: "b1".into(),
                    items: vec![],
                },
                BoardOp::Image {
                    id: "i1".into(),
                    reference: "https://evil.example/x.png".into(),
                },
                BoardOp::Math {
                    id: "m1".into(),
                    latex: "a^2 + b^2 = c^2".into(),
                },
            ],
        )
        .expect("two ops survive");

    let kinds: Vec<&str> = accepted.ops.iter().map(BoardOp::kind).collect();
    assert_eq!(kinds, vec!["heading", "math"]);
    assert_eq!(accepted.dropped.len(), 2);
    assert_eq!(accepted.dropped[0].error, OpError::EmptyField { field: "items" });
    assert!(matches!(
        accepted.dropped[1].error,
        OpError::BadImageReference(_)
    ));
    assert_eq!(gate.metrics().ops_dropped, 2);
}

/// A `board_ops` frame whose ops are all invalid opens no hold: nothing reached
/// the client, so nothing can ack, and stranding the audio for 400 ms would
/// bank a violation for a purely upstream fault.
#[test]
fn board_ops_with_no_valid_ops_opens_no_hold() {
    let (_clock, mut gate) = gate();
    let err = gate
        .on_board_ops(
            TurnSeq(1),
            true,
            vec![BoardOp::Highlight {
                target: "never-drawn".into(),
            }],
        )
        .expect_err("no valid ops");
    assert_eq!(err, SyncGateError::NoValidOps { seq: TurnSeq(1) });
    assert!(!gate.is_holding(TurnSeq(1)));
    assert_eq!(gate.next_hold_deadline_ms(), None);
}

/// `highlight` may only target an element the board has actually drawn, and a
/// `clear_first` wipes those targets.
#[test]
fn highlight_target_must_exist_on_the_current_board() {
    let (_clock, mut gate) = gate();
    gate.on_board_ops(TurnSeq(1), true, vec![heading("h1")])
        .expect("valid ops");
    let accepted = gate
        .on_board_ops(
            TurnSeq(2),
            false,
            vec![BoardOp::Highlight {
                target: "h1".into(),
            }],
        )
        .expect("h1 is on the board");
    assert_eq!(accepted.ops.len(), 1);

    // clear_first wipes the board, so h1 is no longer a legal target.
    let err = gate
        .on_board_ops(
            TurnSeq(3),
            true,
            vec![BoardOp::Highlight {
                target: "h1".into(),
            }],
        )
        .expect_err("board was cleared");
    assert_eq!(err, SyncGateError::NoValidOps { seq: TurnSeq(3) });
}

/// Every kind listed in `whiteboard-sync.md` round-trips through the validator,
/// and an out-of-board-space `draw` does not.
#[test]
fn accepts_every_documented_op_kind_and_rejects_out_of_range_geometry() {
    let (_clock, mut gate) = gate();
    let good = vec![
        heading("h1"),
        BoardOp::Bullets {
            id: "b1".into(),
            items: vec!["one".into(), "two".into()],
        },
        BoardOp::Math {
            id: "m1".into(),
            latex: "x = 1".into(),
        },
        BoardOp::Draw {
            id: "d1".into(),
            shape: DrawShape::Arrow,
            points: vec![Point { x: 0.1, y: 0.1 }, Point { x: 0.9, y: 0.4 }],
        },
        BoardOp::Image {
            id: "i1".into(),
            reference: "doc:0f8fad5b-d9cb-469f-a165-70867728950e#p57-fig2".into(),
        },
        BoardOp::Highlight {
            target: "h1".into(),
        },
    ];
    let accepted = gate.on_board_ops(TurnSeq(1), true, good).expect("all valid");
    assert_eq!(accepted.ops.len(), 6);
    assert!(accepted.dropped.is_empty());

    let accepted = gate
        .on_board_ops(
            TurnSeq(2),
            false,
            vec![
                heading("h2"),
                BoardOp::Draw {
                    id: "d2".into(),
                    shape: DrawShape::Line,
                    points: vec![Point { x: -3.0, y: 0.5 }, Point { x: 0.5, y: 0.5 }],
                },
            ],
        )
        .expect("the heading survives");
    assert_eq!(accepted.ops.len(), 1);
    assert_eq!(
        accepted.dropped[0].error,
        OpError::PointOutOfRange { index: 0 }
    );
}

/// Ops arrive from the model as untyped JSON; an unknown kind is a parse error,
/// never a panic.
#[test]
fn unknown_op_kind_from_json_is_a_parse_error() {
    let value = serde_json::json!({ "kind": "iframe", "src": "https://evil.example" });
    assert!(matches!(
        live::parse_op(&value),
        Err(OpError::Malformed(_))
    ));

    let value = serde_json::json!({ "kind": "heading", "id": "h1", "text": "സന്ധി — Sandhi" });
    assert_eq!(live::parse_op(&value), Ok(heading("h1")));

    // A known kind with a missing required field is also Malformed, not a panic.
    let value = serde_json::json!({ "kind": "bullets", "id": "b1" });
    assert!(matches!(live::parse_op(&value), Err(OpError::Malformed(_))));
}

// ---------------------------------------------------------------------------
// seq robustness.
// ---------------------------------------------------------------------------

/// `seq` is monotonic per session. A repeat or a step backwards is refused with
/// a typed error, and — critically — does not disturb the turn already holding.
#[test]
fn duplicate_and_out_of_order_seq_are_refused_without_panicking() {
    let (_clock, mut gate) = gate();
    gate.on_board_ops(TurnSeq(5), true, vec![heading("h5")])
        .expect("valid ops");
    let _ = gate.push_audio(TurnSeq(5), &FRAME);

    assert_eq!(
        gate.on_board_ops(TurnSeq(5), false, vec![heading("dup")]),
        Err(SyncGateError::NonMonotonicSeq {
            got: TurnSeq(5),
            last: TurnSeq(5)
        })
    );
    assert_eq!(
        gate.on_board_ops(TurnSeq(3), false, vec![heading("old")]),
        Err(SyncGateError::NonMonotonicSeq {
            got: TurnSeq(3),
            last: TurnSeq(5)
        })
    );

    // Turn 5 is untouched and still holding its frame.
    assert!(gate.is_holding(TurnSeq(5)));
    assert_eq!(released(gate.on_board_ack(TurnSeq(5))).frames.len(), 1);
}

/// Control messages naming a turn the gate has never seen are ignored, per the
/// forward-compatibility rule in `api-conventions.md`.
#[test]
fn control_messages_for_unknown_seq_are_ignored_and_counted() {
    let (_clock, mut gate) = gate();
    assert_eq!(
        gate.on_board_ack(TurnSeq(99)),
        AckOutcome::UnknownTurn(TurnSeq(99))
    );
    assert_eq!(
        gate.on_board_error(TurnSeq(99), "render_failed"),
        AckOutcome::UnknownTurn(TurnSeq(99))
    );
    assert_eq!(gate.metrics().unknown_seq_messages, 2);
    assert_eq!(gate.wb_violation(), 0);
}

/// A duplicate ack for a turn that is already open is ignored.
#[test]
fn duplicate_board_ack_is_ignored() {
    let (_clock, mut gate) = gate();
    gate.on_board_ops(TurnSeq(1), true, vec![heading("h1")])
        .expect("valid ops");
    assert!(matches!(
        gate.on_board_ack(TurnSeq(1)),
        AckOutcome::Released(_)
    ));
    assert_eq!(
        gate.on_board_ack(TurnSeq(1)),
        AckOutcome::AlreadyReleased(TurnSeq(1))
    );
}

/// Two turns can hold at once (the model may emit board ops for N+1 before the
/// client acks N); each is released independently and in its own right.
#[test]
fn concurrent_holds_are_released_independently() {
    let (clock, mut gate) = gate();
    gate.on_board_ops(TurnSeq(1), true, vec![heading("h1")])
        .expect("valid ops");
    let _ = gate.push_audio(TurnSeq(1), &FRAME);
    clock.advance_ms(100);
    gate.on_board_ops(TurnSeq(2), false, vec![heading("h2")])
        .expect("valid ops");
    let _ = gate.push_audio(TurnSeq(2), &FRAME);

    let audio = released(gate.on_board_ack(TurnSeq(2)));
    assert_eq!(audio.seq, TurnSeq(2));
    assert!(gate.is_holding(TurnSeq(1)), "turn 1 keeps holding");

    clock.advance_ms(HOLD_MAX_MS);
    let expired = gate.poll_timeouts();
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0].seq, TurnSeq(1));
    assert_eq!(gate.wb_violation(), 1);
}

// ---------------------------------------------------------------------------
// Backpressure.
// ---------------------------------------------------------------------------

/// `realtime-audio.md`: drop the OLDEST buffered audio rather than growing
/// unbounded. The buffer stays at its ceiling and the newest frames survive.
#[test]
fn full_hold_buffer_drops_the_oldest_frame_not_the_newest() {
    let clock = TestClock::new();
    let mut gate = SyncGate::with_config(
        clock,
        SyncGateConfig {
            max_buffered_frames: 3,
            ..SyncGateConfig::default()
        },
    );
    gate.on_board_ops(TurnSeq(1), true, vec![heading("h1")])
        .expect("valid ops");

    for marker in 1u8..=5 {
        let frame = [marker; 4];
        let _ = gate.push_audio(TurnSeq(1), &frame);
    }
    assert_eq!(gate.metrics().frames_evicted, 2);

    let audio = released(gate.on_board_ack(TurnSeq(1)));
    assert_eq!(audio.frames.len(), 3);
    let markers: Vec<u8> = audio.frames.iter().filter_map(|f| f.first().copied()).collect();
    assert_eq!(markers, vec![3, 4, 5], "the two oldest frames were dropped");
}

/// Recycled buffers are reused, so a warm session does not allocate per held
/// frame. Observable only through behaviour: the released content is still
/// correct after a recycle round-trip.
#[test]
fn recycled_frame_buffers_are_reused_without_corrupting_later_audio() {
    let (_clock, mut gate) = gate();
    gate.on_board_ops(TurnSeq(1), true, vec![heading("h1")])
        .expect("valid ops");
    let _ = gate.push_audio(TurnSeq(1), &[9u8; 640]);
    let audio = released(gate.on_board_ack(TurnSeq(1)));
    gate.recycle(audio.frames);

    gate.on_board_ops(TurnSeq(2), false, vec![heading("h2")])
        .expect("valid ops");
    let _ = gate.push_audio(TurnSeq(2), &[1u8; 16]);
    let audio = released(gate.on_board_ack(TurnSeq(2)));
    assert_eq!(audio.frames, vec![vec![1u8; 16]], "no stale bytes survive");
}

/// A turn still holding audio is never retired to keep the map small — that
/// would lose the audio rather than release it.
#[test]
fn a_holding_turn_is_never_retired_by_map_pruning() {
    let clock = TestClock::new();
    let mut gate = SyncGate::with_config(
        clock,
        SyncGateConfig {
            retained_turns: 1,
            ..SyncGateConfig::default()
        },
    );
    gate.on_board_ops(TurnSeq(1), true, vec![heading("h1")])
        .expect("valid ops");
    let _ = gate.push_audio(TurnSeq(1), &FRAME);

    for seq in 2..=20u32 {
        gate.on_board_ops(TurnSeq(seq), false, vec![heading(&format!("h{seq}"))])
            .expect("valid ops");
        let _ = gate.on_board_ack(TurnSeq(seq));
    }

    assert!(gate.is_holding(TurnSeq(1)));
    assert_eq!(released(gate.on_board_ack(TurnSeq(1))).frames.len(), 1);
    assert_eq!(gate.wb_violation(), 0);
}
