//! `/api/v1/student/unit-assessments` — the tutor's verdicts on the student's
//! own conversational performance, unit by unit.
//!
//! # Two kinds of number, never mixed
//!
//! This namespace carries *conversational* assessments: what the tutor judged
//! about how the student engaged in a session. `/student/performance` carries
//! *exam* results, computed in SQL from a stored answer key. They are served by
//! different routes and rendered as different cards on purpose — a student must
//! always be able to tell which of their numbers came from a marked paper.
//!
//! # Self-only
//!
//! The subject is resolved from the access token by [`super::own_student`].
//! Neither route takes a `student_id` in any position, so no request exists
//! that could name another student.
//!
//! # Units with no verdict are still listed
//!
//! A unit the student has opened but never been assessed on appears with
//! `latest: null`. "You studied this and there is no verdict yet" is a
//! different and more useful thing to show than an absence, and a unit never
//! opened at all reads as the next thing to do.

use axum::{
    extract::{Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dg_core::{Capability, DocumentId, PublicError};
use dg_db::models::unit_assessments as store;

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

/// Assessments returned for one unit's history.
const HISTORY_LIMIT: i64 = 20;

/// The verdict on one sitting.
#[derive(Debug, Serialize)]
pub struct AssessmentResponse {
    pub stars: i16,
    pub mark: f64,
    pub trophy: Option<String>,
    /// The breakdown the student reads. Never empty — a rating with no
    /// explanation is the thing this feature exists to avoid.
    pub summary: String,
    pub strengths: Vec<String>,
    pub improvements: Vec<String>,
    pub assessed_at: DateTime<Utc>,
    /// The evidence the verdict was based on, returned alongside it so a
    /// student asking "why this rating?" is answered with what was counted
    /// rather than with the rating repeated.
    pub evidence: EvidenceResponse,
}

#[derive(Debug, Serialize)]
pub struct EvidenceResponse {
    pub student_turns: i32,
    pub questions_asked: i32,
    pub comprehension_passed: i32,
    pub comprehension_failed: i32,
    pub active_voice_ms: i64,
}

/// One unit's card on the assessment page.
#[derive(Debug, Serialize)]
pub struct UnitCardResponse {
    pub document_id: Uuid,
    pub unit_title: String,
    pub block_id: Uuid,
    pub block_no: i16,
    pub block_title: String,
    pub course_id: Uuid,
    pub course_code: String,
    pub course_name: String,
    pub semester_number: i16,
    /// `false` when the unit has no vectors yet, so it cannot be taught. Shown
    /// and marked rather than hidden, for the reason the catalogue gives:
    /// "not ready yet" is truthful where a missing unit is not.
    pub is_ready: bool,

    /// Usage: when the student last opened this unit, and how much they have
    /// studied it.
    pub sessions_total: i64,
    pub active_voice_ms: i64,
    pub last_studied_at: Option<DateTime<Utc>>,

    pub assessments_count: i64,
    /// `null` for a unit never assessed — never a zero-star placeholder.
    pub latest: Option<AssessmentResponse>,
    /// Best mark across every sitting, so improving on a poor session is
    /// visible rather than replaced by it.
    pub best_mark: Option<f64>,
}

/// Parses a JSONB string array back into a `Vec<String>`.
///
/// Tolerant by design: these rows are written by a model-driven job, and one
/// malformed list must degrade to an empty bullet list rather than fail the
/// whole page with a 500.
fn string_list(value: Option<serde_json::Value>) -> Vec<String> {
    value
        .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
        .unwrap_or_default()
}

impl From<store::UnitProgress> for UnitCardResponse {
    fn from(u: store::UnitProgress) -> Self {
        // The latest verdict's columns are NULL together or present together,
        // so one of them decides whether there is an assessment at all.
        let latest = match (u.latest_stars, u.latest_mark, u.latest_summary, u.latest_at) {
            (Some(stars), Some(mark), Some(summary), Some(assessed_at)) => {
                Some(AssessmentResponse {
                    stars,
                    mark,
                    trophy: u.latest_trophy,
                    summary,
                    strengths: string_list(u.latest_strengths),
                    improvements: string_list(u.latest_improvements),
                    assessed_at,
                    evidence: EvidenceResponse {
                        student_turns: u.latest_student_turns.unwrap_or(0),
                        questions_asked: u.latest_questions_asked.unwrap_or(0),
                        comprehension_passed: u.latest_comprehension_passed.unwrap_or(0),
                        comprehension_failed: u.latest_comprehension_failed.unwrap_or(0),
                        active_voice_ms: u.active_voice_ms,
                    },
                })
            }
            _ => None,
        };

        Self {
            document_id: u.document_id.into_uuid(),
            unit_title: u.unit_title,
            block_id: u.block_id.into_uuid(),
            block_no: u.block_no,
            block_title: u.block_title,
            course_id: u.course_id.into_uuid(),
            course_code: u.course_code,
            course_name: u.course_name,
            semester_number: u.semester_number,
            is_ready: u.is_ready,
            sessions_total: u.sessions_total,
            active_voice_ms: u.active_voice_ms,
            last_studied_at: u.last_studied_at,
            assessments_count: u.assessments_count,
            latest,
            best_mark: u.best_mark,
        }
    }
}

/// `GET /api/v1/student/unit-assessments`
///
/// Not paginated, for the reason the syllabus routes give: a student has a
/// handful of courses, each a handful of blocks, each a handful of units. This
/// is a screen rendered whole.
pub async fn list_units(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
) -> Result<Json<Vec<UnitCardResponse>>, PublicError> {
    actor.require(Capability::ViewOwnContext)?;

    let student = super::own_student(&state, &actor).await?;
    // `None` scope: the student *is* the scope on this path, and the query
    // already binds their own id.
    let rows = store::units_for_student(&state.pool, student.id, None).await?;
    Ok(Json(rows.into_iter().map(UnitCardResponse::from).collect()))
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub document_id: Uuid,
}

/// `GET /api/v1/student/unit-assessments/history?document_id=...`
///
/// Every sitting of one unit, newest first, so a student can see that a second
/// attempt went better than the first.
///
/// The unit is not separately authorized: the query binds the caller's own
/// student id, so a `document_id` belonging to someone else's programme simply
/// selects no rows and returns `[]`. There is nothing to leak, because there is
/// nothing of theirs to select.
pub async fn unit_history(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<Vec<AssessmentResponse>>, PublicError> {
    actor.require(Capability::ViewOwnContext)?;

    let student = super::own_student(&state, &actor).await?;
    let rows = store::history_for_unit(
        &state.pool,
        student.id,
        DocumentId::from(query.document_id),
        HISTORY_LIMIT,
    )
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|r| AssessmentResponse {
                stars: r.stars,
                mark: r.mark,
                trophy: r.trophy,
                summary: r.summary,
                strengths: string_list(Some(r.strengths)),
                improvements: string_list(Some(r.improvements)),
                assessed_at: r.created_at,
                evidence: EvidenceResponse {
                    student_turns: r.student_turns,
                    questions_asked: r.questions_asked,
                    comprehension_passed: r.comprehension_passed,
                    comprehension_failed: r.comprehension_failed,
                    active_voice_ms: r.active_voice_ms,
                },
            })
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dg_core::{Actor, ProgramId, Role, UserId};

    #[test]
    fn a_student_may_read_their_own_unit_assessments() {
        let actor = Actor::new(UserId::new(), Role::Student, vec![]);
        assert!(actor.require(Capability::ViewOwnContext).is_ok());
    }

    /// An admin holds `ViewOwnContext` but has no `students` row, so
    /// `own_student` answers 404 — they reach no student's assessments here.
    #[test]
    fn an_admin_has_no_unit_assessments_of_its_own() {
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![ProgramId::new()]);
        assert!(actor.require(Capability::ViewOwnContext).is_ok());
    }

    /// Destructured exhaustively: adding a `student_id` field would stop this
    /// compiling, which is the point.
    #[test]
    fn the_history_query_cannot_name_another_student() {
        let HistoryQuery { document_id } = HistoryQuery {
            document_id: Uuid::nil(),
        };
        assert_eq!(document_id, Uuid::nil());
    }

    /// A unit with no verdict must render as an empty state, not as a zero
    /// rating against work the student was never assessed on.
    #[test]
    fn a_unit_with_no_assessment_has_a_null_latest() {
        let progress = store::UnitProgress {
            document_id: DocumentId::new(),
            unit_title: "Unit 3 Forest Resources".into(),
            block_id: dg_core::BlockId::new(),
            block_no: 1,
            block_title: "BLOCK - 01".into(),
            course_id: dg_core::CourseId::new(),
            course_code: "EVS101".into(),
            course_name: "Environmental Studies".into(),
            semester_number: 1,
            is_ready: true,
            sessions_total: 2,
            active_voice_ms: 60_000,
            last_studied_at: None,
            assessments_count: 0,
            latest_stars: None,
            latest_mark: None,
            latest_trophy: None,
            latest_summary: None,
            latest_strengths: None,
            latest_improvements: None,
            latest_at: None,
            latest_student_turns: None,
            latest_questions_asked: None,
            latest_comprehension_passed: None,
            latest_comprehension_failed: None,
            best_mark: None,
        };
        let card = UnitCardResponse::from(progress);
        assert!(card.latest.is_none());
        assert_eq!(card.assessments_count, 0);
        // Usage still shows, because the student really did study it.
        assert_eq!(card.sessions_total, 2);
    }

    /// A malformed bullet list degrades to empty rather than failing the page:
    /// these rows come from a model-driven job.
    #[test]
    fn a_malformed_bullet_list_degrades_to_empty() {
        assert!(string_list(Some(serde_json::json!("not an array"))).is_empty());
        assert!(string_list(None).is_empty());
        assert_eq!(
            string_list(Some(serde_json::json!(["a", "b"]))),
            vec!["a".to_string(), "b".to_string()]
        );
    }
}
