//! The whiteboard op schema and its gateway-side validator.
//!
//! `whiteboard-sync.md`: "Ops are validated against the schema at the gateway
//! before forwarding. An invalid op is dropped and logged; it never reaches the
//! client." Validation is therefore a hard filter, not a warning — the canvas
//! is allowed to assume every op it receives is well-formed.

use serde::{Deserialize, Serialize};

/// Longest text a single heading/bullet/latex/target field may carry. The cap
/// exists so a malformed model response cannot push an unbounded string
/// through the control channel.
const MAX_TEXT_LEN: usize = 2_000;
/// A bullets op with more entries than this is a paragraph, not a bullet list.
const MAX_BULLETS: usize = 24;
/// Upper bound on the vertices of a single `draw` primitive.
const MAX_POINTS: usize = 512;

/// The shape primitives a `draw` op may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrawShape {
    Line,
    Arrow,
    Rect,
    Ellipse,
    Polyline,
}

/// A point in board coordinates, normalised to `0.0..=1.0` on both axes so the
/// board renders identically at any canvas size.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// One validated whiteboard operation.
///
/// The six variants are exactly the op kinds listed in `whiteboard-sync.md`;
/// adding a seventh is a protocol change, not an implementation detail.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BoardOp {
    Heading {
        id: String,
        text: String,
    },
    Bullets {
        id: String,
        items: Vec<String>,
    },
    /// LaTeX source; the client renders it.
    Math {
        id: String,
        latex: String,
    },
    Draw {
        id: String,
        shape: DrawShape,
        points: Vec<Point>,
    },
    /// `reference` is a figure locator of the form `doc:<uuid>#p<NN>-fig<M>`.
    Image {
        id: String,
        reference: String,
    },
    /// Targets the `id` of an element emitted earlier in this session.
    Highlight {
        target: String,
    },
}

impl BoardOp {
    /// The element id this op introduces, if it introduces one. `highlight`
    /// references an existing element and introduces nothing.
    pub fn declared_id(&self) -> Option<&str> {
        match self {
            BoardOp::Heading { id, .. }
            | BoardOp::Bullets { id, .. }
            | BoardOp::Math { id, .. }
            | BoardOp::Draw { id, .. }
            | BoardOp::Image { id, .. } => Some(id.as_str()),
            BoardOp::Highlight { .. } => None,
        }
    }

    /// The op kind as it appears on the wire — used in drop logs.
    pub fn kind(&self) -> &'static str {
        match self {
            BoardOp::Heading { .. } => "heading",
            BoardOp::Bullets { .. } => "bullets",
            BoardOp::Math { .. } => "math",
            BoardOp::Draw { .. } => "draw",
            BoardOp::Image { .. } => "image",
            BoardOp::Highlight { .. } => "highlight",
        }
    }
}

/// Why a single op was rejected. Carried in the drop log, never sent onward.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum OpError {
    /// The op did not deserialise into any known kind, or a required field was
    /// missing or of the wrong type.
    #[error("malformed op: {0}")]
    Malformed(String),
    #[error("field `{field}` is empty")]
    EmptyField { field: &'static str },
    #[error("field `{field}` is longer than {max} characters")]
    TextTooLong { field: &'static str, max: usize },
    #[error("`{field}` has {len} entries, limit is {max}")]
    TooManyEntries {
        field: &'static str,
        len: usize,
        max: usize,
    },
    #[error("point {index} is outside the normalised 0.0..=1.0 board space")]
    PointOutOfRange { index: usize },
    #[error("`draw` shape {shape:?} needs at least {min} points, got {len}")]
    NotEnoughPoints {
        shape: DrawShape,
        min: usize,
        len: usize,
    },
    #[error("image reference `{0}` is not of the form doc:<uuid>#p<NN>-fig<M>")]
    BadImageReference(String),
    #[error("highlight targets `{0}`, which was never emitted on this board")]
    UnknownHighlightTarget(String),
}

fn check_text(field: &'static str, value: &str) -> Result<(), OpError> {
    if value.trim().is_empty() {
        return Err(OpError::EmptyField { field });
    }
    if value.chars().count() > MAX_TEXT_LEN {
        return Err(OpError::TextTooLong {
            field,
            max: MAX_TEXT_LEN,
        });
    }
    Ok(())
}

/// `doc:<uuid>#p<digits>-fig<digits>` — parsed by hand rather than by regex so
/// the crate keeps no regex dependency on the audio path.
fn check_image_reference(reference: &str) -> Result<(), OpError> {
    let bad = || OpError::BadImageReference(reference.to_owned());
    let rest = reference.strip_prefix("doc:").ok_or_else(bad)?;
    let (uuid_part, locator) = rest.split_once('#').ok_or_else(bad)?;
    if uuid::Uuid::parse_str(uuid_part).is_err() {
        return Err(bad());
    }
    let (page, fig) = locator.split_once("-fig").ok_or_else(bad)?;
    let page = page.strip_prefix('p').ok_or_else(bad)?;
    let numeric = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !numeric(page) || !numeric(fig) {
        return Err(bad());
    }
    Ok(())
}

fn min_points(shape: DrawShape) -> usize {
    match shape {
        DrawShape::Line
        | DrawShape::Arrow
        | DrawShape::Rect
        | DrawShape::Ellipse
        | DrawShape::Polyline => 2,
    }
}

/// Validate one op against the schema.
///
/// `known_ids` is the set of element ids already emitted on this board; a
/// `highlight` may only target one of them, since highlighting an element the
/// canvas has never drawn is a silent no-op on the client and would look to a
/// student like the tutor pointed at nothing.
pub fn validate_op(op: &BoardOp, known_ids: &[String]) -> Result<(), OpError> {
    match op {
        BoardOp::Heading { id, text } => {
            check_text("id", id)?;
            check_text("text", text)
        }
        BoardOp::Bullets { id, items } => {
            check_text("id", id)?;
            if items.is_empty() {
                return Err(OpError::EmptyField { field: "items" });
            }
            if items.len() > MAX_BULLETS {
                return Err(OpError::TooManyEntries {
                    field: "items",
                    len: items.len(),
                    max: MAX_BULLETS,
                });
            }
            for item in items {
                check_text("items[]", item)?;
            }
            Ok(())
        }
        BoardOp::Math { id, latex } => {
            check_text("id", id)?;
            check_text("latex", latex)
        }
        BoardOp::Draw { id, shape, points } => {
            check_text("id", id)?;
            let min = min_points(*shape);
            if points.len() < min {
                return Err(OpError::NotEnoughPoints {
                    shape: *shape,
                    min,
                    len: points.len(),
                });
            }
            if points.len() > MAX_POINTS {
                return Err(OpError::TooManyEntries {
                    field: "points",
                    len: points.len(),
                    max: MAX_POINTS,
                });
            }
            for (index, p) in points.iter().enumerate() {
                let ok = p.x.is_finite()
                    && p.y.is_finite()
                    && (0.0..=1.0).contains(&p.x)
                    && (0.0..=1.0).contains(&p.y);
                if !ok {
                    return Err(OpError::PointOutOfRange { index });
                }
            }
            Ok(())
        }
        BoardOp::Image { id, reference } => {
            check_text("id", id)?;
            check_image_reference(reference)
        }
        BoardOp::Highlight { target } => {
            check_text("target", target)?;
            if known_ids.iter().any(|k| k == target) {
                Ok(())
            } else {
                Err(OpError::UnknownHighlightTarget(target.clone()))
            }
        }
    }
}

/// Parse an op that arrived as untyped JSON (which is how it leaves the model).
/// A shape serde cannot make sense of is `Malformed`, never a panic.
pub fn parse_op(value: &serde_json::Value) -> Result<BoardOp, OpError> {
    serde_json::from_value::<BoardOp>(value.clone()).map_err(|e| OpError::Malformed(e.to_string()))
}
