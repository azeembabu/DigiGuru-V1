//! The board-op schema and validator (`IMPLEMENTATION_PLAN.md` §6.3,
//! `.claude/rules/whiteboard-sync.md` "Op schema").
//!
//! This module only knows about the *shape* of a `board_ops` message and
//! whether it is structurally sane. It says nothing about *when* the ops are
//! allowed to reach the client — that ordering guarantee is `sync_gate`'s job
//! (NN-1). Per the whiteboard-sync rule, "an invalid op is dropped and
//! logged; it never reaches the client" — `validate` is what the gateway
//! calls to decide that, before anything is handed to `SyncGate`.

use serde::{Deserialize, Serialize};

/// A single whiteboard operation. Tagged by the `op` field so the wire
/// format matches `IMPLEMENTATION_PLAN.md` §6.3 exactly, e.g.
/// `{"op":"heading","text":"...","page":57}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum BoardOp {
    Heading { text: String, page: u32 },
    Bullets { items: Vec<String> },
    Math { latex: String },
    Draw {
        shape: String,
        from: [f32; 2],
        to: [f32; 2],
    },
    /// `ref` is a Rust keyword, so the field is named `image_ref` and
    /// renamed on the wire back to `ref` to match the documented schema.
    Image {
        #[serde(rename = "ref")]
        image_ref: String,
    },
    Highlight { target: String },
}

/// `{ "type": "board_ops", "seq": 42, "clear_first": false, "ops": [...] }`.
///
/// `msg_type` carries the wire value of the `type` field so `validate` can
/// reject a message that isn't actually a `board_ops` message (e.g. one that
/// got routed here by mistake); it is not meant to ever hold anything other
/// than `"board_ops"` in a well-formed message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoardOpsMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub seq: u32,
    #[serde(default)]
    pub clear_first: bool,
    pub ops: Vec<BoardOp>,
}

const BOARD_OPS_TYPE: &str = "board_ops";

/// Accepted `image.ref` grammar: `doc:<uuid>#p<digits>-fig<digits>`, e.g.
/// `doc:3fa85f64-5717-4562-b3fc-2c963f66afa6#p57-fig2`. The UUID is not
/// validated for RFC-4122 correctness here (that would reject legitimate
/// legacy ids); it just has to be a non-empty run of hex/hyphen characters,
/// which is enough to catch the common malformed cases (missing `doc:`
/// prefix, missing page/fig numbers, wrong separators).
fn is_valid_image_ref(image_ref: &str) -> bool {
    let Some(rest) = image_ref.strip_prefix("doc:") else {
        return false;
    };
    let Some((uuid_part, page_fig)) = rest.split_once('#') else {
        return false;
    };
    if uuid_part.is_empty()
        || !uuid_part
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == '-')
    {
        return false;
    }
    let Some(page_num) = page_fig.strip_prefix('p') else {
        return false;
    };
    let Some((page_num, fig_num)) = page_num.split_once("-fig") else {
        return false;
    };
    !page_num.is_empty()
        && page_num.chars().all(|c| c.is_ascii_digit())
        && !fig_num.is_empty()
        && fig_num.chars().all(|c| c.is_ascii_digit())
}

/// Structural validation only — per `.claude/rules/whiteboard-sync.md`,
/// invalid ops are dropped and logged before ever reaching `SyncGate` or the
/// client. This does not check semantic validity (e.g. that `doc:UUID`
/// refers to a real, accessible document) — that is the gateway's job with
/// access to the database.
pub fn validate(msg: &BoardOpsMessage) -> crate::error::Result<()> {
    use crate::error::LiveError;

    if msg.msg_type != BOARD_OPS_TYPE {
        return Err(LiveError::Malformed(format!(
            "expected type \"board_ops\", got \"{}\"",
            msg.msg_type
        )));
    }

    if msg.ops.is_empty() {
        return Err(LiveError::InvalidBoardOp(
            "ops must be non-empty".to_string(),
        ));
    }

    for op in &msg.ops {
        match op {
            BoardOp::Heading { page, .. } => {
                if *page < 1 {
                    return Err(LiveError::InvalidBoardOp(
                        "heading.page must be >= 1".to_string(),
                    ));
                }
            }
            BoardOp::Bullets { items } => {
                if items.is_empty() {
                    return Err(LiveError::InvalidBoardOp(
                        "bullets.items must be non-empty".to_string(),
                    ));
                }
            }
            BoardOp::Math { latex } => {
                if latex.trim().is_empty() {
                    return Err(LiveError::InvalidBoardOp(
                        "math.latex must be non-empty".to_string(),
                    ));
                }
            }
            BoardOp::Draw { shape, .. } => {
                if shape.trim().is_empty() {
                    return Err(LiveError::InvalidBoardOp(
                        "draw.shape must be non-empty".to_string(),
                    ));
                }
            }
            BoardOp::Image { image_ref } => {
                if !is_valid_image_ref(image_ref) {
                    return Err(LiveError::InvalidBoardOp(format!(
                        "image.ref \"{image_ref}\" does not match doc:<uuid>#p<n>-fig<n>"
                    )));
                }
            }
            BoardOp::Highlight { target } => {
                if target.trim().is_empty() {
                    return Err(LiveError::InvalidBoardOp(
                        "highlight.target must be non-empty".to_string(),
                    ));
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_message() -> BoardOpsMessage {
        BoardOpsMessage {
            msg_type: BOARD_OPS_TYPE.to_string(),
            seq: 42,
            clear_first: false,
            ops: vec![
                BoardOp::Heading {
                    text: "Chapter 3".to_string(),
                    page: 57,
                },
                BoardOp::Bullets {
                    items: vec!["one".to_string(), "two".to_string()],
                },
                BoardOp::Math {
                    latex: "a^2 + b^2 = c^2".to_string(),
                },
                BoardOp::Draw {
                    shape: "arrow".to_string(),
                    from: [120.0, 200.0],
                    to: [340.0, 200.0],
                },
                BoardOp::Image {
                    image_ref: "doc:3fa85f64-5717-4562-b3fc-2c963f66afa6#p57-fig2".to_string(),
                },
                BoardOp::Highlight {
                    target: "bullet:1".to_string(),
                },
            ],
        }
    }

    #[test]
    fn valid_message_round_trips_and_validates() {
        let msg = valid_message();
        let json = serde_json::to_string(&msg).expect("serialize");
        let round_tripped: BoardOpsMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, round_tripped);
        assert!(validate(&round_tripped).is_ok());
    }

    #[test]
    fn wire_format_matches_documented_shape() {
        let msg = BoardOpsMessage {
            msg_type: BOARD_OPS_TYPE.to_string(),
            seq: 1,
            clear_first: true,
            ops: vec![BoardOp::Highlight {
                target: "bullet:0".to_string(),
            }],
        };
        let json: serde_json::Value =
            serde_json::to_value(&msg).expect("serialize to value");
        assert_eq!(json["type"], "board_ops");
        assert_eq!(json["ops"][0]["op"], "highlight");
        assert_eq!(json["ops"][0]["target"], "bullet:0");
    }

    #[test]
    fn empty_ops_is_rejected() {
        let mut msg = valid_message();
        msg.ops.clear();
        let err = validate(&msg).unwrap_err();
        assert!(matches!(err, crate::error::LiveError::InvalidBoardOp(_)));
    }

    #[test]
    fn empty_bullets_is_rejected() {
        let mut msg = valid_message();
        msg.ops = vec![BoardOp::Bullets { items: vec![] }];
        let err = validate(&msg).unwrap_err();
        assert!(matches!(err, crate::error::LiveError::InvalidBoardOp(_)));
    }

    #[test]
    fn heading_page_zero_is_rejected() {
        let mut msg = valid_message();
        msg.ops = vec![BoardOp::Heading {
            text: "x".to_string(),
            page: 0,
        }];
        let err = validate(&msg).unwrap_err();
        assert!(matches!(err, crate::error::LiveError::InvalidBoardOp(_)));
    }

    #[test]
    fn malformed_image_ref_is_rejected() {
        let mut msg = valid_message();
        msg.ops = vec![BoardOp::Image {
            image_ref: "not-a-valid-ref".to_string(),
        }];
        let err = validate(&msg).unwrap_err();
        assert!(matches!(err, crate::error::LiveError::InvalidBoardOp(_)));
    }

    #[test]
    fn image_ref_missing_fig_number_is_rejected() {
        let mut msg = valid_message();
        msg.ops = vec![BoardOp::Image {
            image_ref: "doc:abc123#p57-fig".to_string(),
        }];
        let err = validate(&msg).unwrap_err();
        assert!(matches!(err, crate::error::LiveError::InvalidBoardOp(_)));
    }

    #[test]
    fn empty_highlight_target_is_rejected() {
        let mut msg = valid_message();
        msg.ops = vec![BoardOp::Highlight {
            target: String::new(),
        }];
        let err = validate(&msg).unwrap_err();
        assert!(matches!(err, crate::error::LiveError::InvalidBoardOp(_)));
    }

    #[test]
    fn wrong_msg_type_is_rejected() {
        let mut msg = valid_message();
        msg.msg_type = "board_ack".to_string();
        let err = validate(&msg).unwrap_err();
        assert!(matches!(err, crate::error::LiveError::Malformed(_)));
    }
}
