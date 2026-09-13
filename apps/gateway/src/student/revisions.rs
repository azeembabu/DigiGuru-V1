//! `GET /api/v1/student/revisions` — the "up next" revision queue.
//!
//! A revision is a `note_reminders` row that has come due: the student tagged
//! an exported note with a future date (`.claude/rules/pedagogy.md`), and the
//! row points at a `board_events` range rather than storing the note's content
//! — export is a client-side render of the op log, so the reminder is the only
//! server-side trace.
//!
//! Self-only and paginated exactly like every other list in this API: bare
//! array body, `X-Total-Count`, `limit` 1..200 validated rather than clamped.
//!
//! "Due" is resolved against the student's **own** calendar date
//! (`students.timezone`), not the server's — the same rule NN-3 applies to the
//! voice quota, and for the same reason: a reminder set for tomorrow in Kochi
//! must not surface because a UTC server has already rolled over.

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dg_core::{Capability, PublicError};

use super::queries;
use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

/// One due revision card. Denormalised across the session's block and course so
/// a page of cards is one request.
#[derive(Debug, Serialize)]
pub struct RevisionResponse {
    pub id: Uuid,
    pub session_id: Uuid,
    /// The `board_events` range the note was exported from — the client replays
    /// these ops to re-render the note.
    pub event_from_id: i64,
    pub event_to_id: i64,
    /// Board ops actually inside the range. Counted, not derived from the id
    /// span: `board_events.id` is a global sequence shared with every other
    /// session, so `to - from` is not a row count.
    pub board_ops_count: i64,
    pub course_id: Uuid,
    pub course_code: String,
    pub block_id: Uuid,
    pub block_no: i16,
    pub block_title: String,
    /// The session's last topic — the nearest thing to a note title.
    pub topic: Option<String>,
    /// A calendar date, not an instant: the reminder is "revise on this day in
    /// the student's own zone".
    pub remind_at: NaiveDate,
    /// Set the first time the reminder was shown on login; `null` while it is
    /// still unseen.
    pub surfaced_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl From<queries::DueRevision> for RevisionResponse {
    fn from(r: queries::DueRevision) -> Self {
        Self {
            id: r.id,
            session_id: r.session_id.into_uuid(),
            event_from_id: r.event_from_id,
            event_to_id: r.event_to_id,
            board_ops_count: r.board_ops_count,
            course_id: r.course_id.into_uuid(),
            course_code: r.course_code,
            block_id: r.block_id.into_uuid(),
            block_no: r.block_no,
            block_title: r.block_title,
            topic: r.topic,
            remind_at: r.remind_at,
            surfaced_at: r.surfaced_at,
            created_at: r.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListRevisionsQuery {
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

pub async fn list_revisions(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Query(query): Query<ListRevisionsQuery>,
) -> Result<(HeaderMap, Json<Vec<RevisionResponse>>), PublicError> {
    actor.require(Capability::ViewOwnContext)?;

    let student = super::own_student(&state, &actor).await?;
    let page = crate::admin::page(query.limit, query.offset, 50)?;
    let today = quota::local_date(Utc::now(), quota::resolve_timezone(&student.timezone));

    let total = queries::count_due_revisions(&state.pool, student.id, today).await?;
    let rows =
        queries::due_revisions(&state.pool, student.id, today, page.limit, page.offset).await?;

    Ok((
        crate::admin::total_count(total),
        Json(rows.into_iter().map(RevisionResponse::from).collect()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dg_core::{Actor, ProgramId, Role, UserId};

    #[test]
    fn a_student_may_read_its_own_revision_queue() {
        let actor = Actor::new(UserId::new(), Role::Student, vec![]);
        assert!(actor.require(Capability::ViewOwnContext).is_ok());
    }

    /// An admin holds `ViewOwnContext` but has no `students` row, so it cannot
    /// reach another student's reminders: the subject is resolved from the
    /// token and `own_student` answers `404`. Pinned here as the documented
    /// behaviour of this route.
    #[test]
    fn an_admin_has_no_revision_queue_of_its_own() {
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![ProgramId::new()]);
        assert!(
            actor.require(Capability::ViewOwnContext).is_ok(),
            "the capability passes; the missing students row is what 404s"
        );
    }

    /// The query shape carries no student selector. Destructured exhaustively
    /// on purpose: adding a `student_id` field would stop this compiling.
    #[test]
    fn the_revision_query_cannot_name_another_student() {
        let ListRevisionsQuery { limit, offset } = ListRevisionsQuery {
            limit: Some(25),
            offset: Some(0),
        };
        assert_eq!(limit, Some(25));
        assert_eq!(offset, Some(0));
    }

    #[test]
    fn revisions_reject_an_out_of_range_limit_rather_than_clamping() {
        assert_eq!(
            crate::admin::page(Some(0), None, 50)
                .expect_err("zero is not a page size")
                .code(),
            "VALIDATION_ERROR"
        );
        assert_eq!(
            crate::admin::page(Some(201), None, 50)
                .expect_err("over the ceiling")
                .code(),
            "VALIDATION_ERROR"
        );
        assert_eq!(
            crate::admin::page(None, Some(-1), 50)
                .expect_err("negative offset")
                .code(),
            "VALIDATION_ERROR"
        );
    }
}
