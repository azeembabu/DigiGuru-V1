//! Persistence of board ops and their ACK latency to `board_events`.
//!
//! `.claude/rules/whiteboard-sync.md` makes this an invariant, not an option:
//! *"Every op and its ACK latency is written to `board_events` for audit and
//! replay."* Two things depend on it that were, until this module existed,
//! reading a table nothing ever wrote:
//!
//! * `GET /admin/board-events` and the analytics `ack_latency_buckets` /
//!   `whiteboard.violation_rate` figures — an always-empty table reports a
//!   `violation_rate` of `null` no matter how badly a session behaved,
//! * replay, which `whiteboard-sync.md` defines as reconstruction from the JSON
//!   op-log, so an unwritten op is an unreplayable turn.
//!
//! ## Why this is one task with a channel, not two `tokio::spawn`s
//!
//! The obvious implementation — spawn the INSERT when ops are emitted, spawn the
//! UPDATE when the ACK arrives — is wrong, and wrong in a way that looks fine.
//! A fast client ACKs in about a millisecond, so the UPDATE routinely runs
//! *before* the INSERT has committed, matches zero rows, and leaves `acked_ms`
//! NULL. `acked_ms IS NULL` is exactly how the analytics endpoint defines a
//! whiteboard-first violation, so the healthiest possible client would be
//! recorded as violating NN-1 on every single turn. That was observed happening
//! before this was rewritten — the first version of this file had precisely that
//! bug.
//!
//! So all writes for one session go through one channel to one task, which
//! applies them **in submission order**. Ordering is the only property that has
//! to be guaranteed; latency does not, which is why the socket never awaits a
//! write.
//!
//! ## Why the audio path never waits
//!
//! `try_send` is used, never `send().await`. The NN-1 budget in
//! `.claude/rules/realtime-audio.md` gives the board hold ≤250 ms typical, and an
//! audit insert may not spend any of it: a slow or wedged Postgres must degrade
//! the audit trail, never the lesson. A full queue drops the row and logs it, so
//! a degraded trail is visible rather than silent.

use sqlx::PgPool;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Bounded, so a wedged database cannot grow this for the life of a session.
/// Sized for a burst of turns, not for a backlog: if it fills, the right answer
/// is to drop audit rows and say so.
const QUEUE_CAPACITY: usize = 256;

/// One ordered write.
enum AuditEvent {
    OpsEmitted {
        turn_seq: i32,
        payloads: Vec<serde_json::Value>,
    },
    AckLatency {
        turn_seq: i32,
        acked_ms: i32,
    },
}

/// Handle held by the socket task.
///
/// `None` inside means auditing is disabled for this connection — the account has
/// no `students` row, so there is no `learning_sessions` row for
/// `board_events.session_id` to reference. Every method then does nothing, which
/// is deliberate: an admin opening the classroom still gets a working lesson, and
/// the alternative is a guaranteed foreign-key failure on every turn.
#[derive(Clone)]
pub struct BoardAudit {
    tx: Option<mpsc::Sender<AuditEvent>>,
}

impl BoardAudit {
    /// Auditing turned off, for a socket with no durable session row.
    pub fn disabled() -> Self {
        Self { tx: None }
    }

    /// Starts the writer for one session.
    ///
    /// Takes `session_id` once, at construction, so no call site can pass the
    /// wrong one on a later turn.
    pub fn spawn(pool: PgPool, session_id: Uuid) -> Self {
        let (tx, mut rx) = mpsc::channel::<AuditEvent>(QUEUE_CAPACITY);

        tokio::spawn(async move {
            // Sequential by construction: one receiver, one event awaited at a
            // time. This is what makes an ACK update land after its own insert.
            while let Some(event) = rx.recv().await {
                match event {
                    AuditEvent::OpsEmitted { turn_seq, payloads } => {
                        // One statement per turn: `unnest` keeps it a single round
                        // trip however many ops the turn emitted.
                        let result = sqlx::query!(
                            r#"
                            INSERT INTO board_events (session_id, turn_seq, op)
                            SELECT $1, $2, op
                            FROM unnest($3::jsonb[]) AS t(op)
                            "#,
                            session_id,
                            turn_seq,
                            &payloads,
                        )
                        .execute(&pool)
                        .await;

                        if let Err(err) = result {
                            tracing::error!(%err, %session_id, turn_seq, "failed to persist board_events");
                        }
                    }
                    AuditEvent::AckLatency { turn_seq, acked_ms } => {
                        // Fills a NULL only: a turn releases once, and a duplicate
                        // `board_ack` for an already-open turn must not overwrite
                        // the first (true) latency with a later, larger one.
                        let result = sqlx::query!(
                            r#"
                            UPDATE board_events
                            SET acked_ms = $3
                            WHERE session_id = $1 AND turn_seq = $2 AND acked_ms IS NULL
                            "#,
                            session_id,
                            turn_seq,
                            acked_ms,
                        )
                        .execute(&pool)
                        .await;

                        match result {
                            Ok(done) if done.rows_affected() == 0 => {
                                // No row to update means this turn's insert never
                                // landed. Warned about explicitly: it is the
                                // signature of the ordering bug this design
                                // exists to prevent, so a regression is loud.
                                tracing::warn!(
                                    %session_id,
                                    turn_seq,
                                    "ack latency had no board_events row to update"
                                );
                            }
                            Ok(_) => {}
                            Err(err) => {
                                tracing::error!(%err, %session_id, turn_seq, "failed to persist ack latency");
                            }
                        }
                    }
                }
            }
        });

        Self { tx: Some(tx) }
    }

    /// Records the ops emitted for one turn, one row per op.
    ///
    /// Stores the validated JSON the client actually received — never raw model
    /// output, which the gate may have rejected ops from.
    pub fn ops_emitted(&self, turn_seq: u32, ops: &[live::BoardOp]) {
        if self.tx.is_none() {
            return;
        }

        let payloads: Vec<serde_json::Value> = ops
            .iter()
            .filter_map(|op| match serde_json::to_value(op) {
                Ok(value) => Some(value),
                Err(err) => {
                    // A validated op that will not serialise is a bug in the op
                    // schema, not a client problem. Drop the audit row, not the
                    // turn.
                    tracing::error!(%err, turn_seq, "board op failed to serialise for audit");
                    None
                }
            })
            .collect();

        if payloads.is_empty() {
            return;
        }

        self.send(AuditEvent::OpsEmitted {
            turn_seq: clamp_seq(turn_seq),
            payloads,
        });
    }

    /// Fills in `acked_ms` once the turn's audio is released.
    ///
    /// Called on every release edge, including the hold-ceiling expiry: the
    /// `>400ms` analytics bucket exists precisely to surface the ceiling case, so
    /// recording only the happy path would hide the violations it is there to
    /// count.
    pub fn ack_latency(&self, turn_seq: u32, acked_ms: u64) {
        self.send(AuditEvent::AckLatency {
            turn_seq: clamp_seq(turn_seq),
            acked_ms: i32::try_from(acked_ms).unwrap_or(i32::MAX),
        });
    }

    fn send(&self, event: AuditEvent) {
        let Some(tx) = self.tx.as_ref() else {
            return;
        };
        // `try_send`, never `send().await`: this is called from the socket loop,
        // and awaiting would put database backpressure straight into the audio
        // path.
        if let Err(err) = tx.try_send(event) {
            match err {
                mpsc::error::TrySendError::Full(_) => {
                    tracing::warn!("board audit queue full; dropping an audit row");
                }
                mpsc::error::TrySendError::Closed(_) => {
                    tracing::debug!("board audit writer stopped; dropping an audit row");
                }
            }
        }
    }
}

fn clamp_seq(turn_seq: u32) -> i32 {
    i32::try_from(turn_seq).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use live::BoardOp;

    /// The audit stores the same JSON the client received, so a replay built from
    /// `board_events` reconstructs the same board.
    ///
    /// Note this is `live::BoardOp` (the gate's validated op, tagged `kind`), NOT
    /// `live::board::BoardOp` (the model-facing wire schema, tagged `op`). The
    /// socket sends the former, so the former is what must be archived — rows
    /// written in the other shape would render nothing on replay.
    #[test]
    fn an_op_serialises_to_the_tagged_shape_the_client_receives() {
        let op = BoardOp::Heading {
            id: "h1".to_string(),
            text: "Prosody".to_string(),
        };
        let value = serde_json::to_value(&op).expect("heading serialises");
        assert_eq!(value["kind"], "heading", "got {value}");
        assert_eq!(value["text"], "Prosody");
        assert_eq!(value["id"], "h1");
    }

    #[test]
    fn bullets_keep_their_items_for_replay() {
        let op = BoardOp::Bullets {
            id: "b1".to_string(),
            items: vec!["one".to_string(), "two".to_string()],
        };
        let value = serde_json::to_value(&op).expect("bullets serialise");
        assert_eq!(value["kind"], "bullets");
        assert_eq!(value["items"][1], "two");
    }

    #[test]
    fn a_seq_beyond_i32_is_clamped_rather_than_wrapped() {
        // `turn_seq` is an INT column. Wrapping would file a turn under a
        // negative seq and silently corrupt the trail.
        assert_eq!(super::clamp_seq(7), 7);
        assert_eq!(super::clamp_seq(u32::MAX), i32::MAX);
    }

    /// A disabled audit must be inert, not panic: the admin-in-classroom case
    /// goes through every one of these paths.
    #[test]
    fn a_disabled_audit_silently_does_nothing() {
        let audit = super::BoardAudit::disabled();
        audit.ops_emitted(
            1,
            &[BoardOp::Heading {
                id: "h1".to_string(),
                text: "x".to_string(),
            }],
        );
        audit.ack_latency(1, 12);
    }
}
