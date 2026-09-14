//! `/api/v1/student/performance` — the student's own assessment page.
//!
//! One row per **block** of the student's actively enrolled courses: what they
//! scored, what they studied, which topics they keep missing, a star rating,
//! and their own written remark.
//!
//! # Self-only, with no request shape that could name another student
//!
//! Like every other route in this namespace, the subject comes from the access
//! token via [`super::own_student`]. Neither the list nor the remark write
//! takes a `student_id` anywhere — not in the path, not in the query, not in
//! the body — so there is no request a client can make for somebody else's
//! record. The remark's `block_id` is checked against the caller's own active
//! enrolments before anything is written, and a block outside them answers
//! `404` rather than `403`: a `403` would confirm the id exists.
//!
//! # Nothing here is stored except the remark
//!
//! Every score, star, level and weak topic is computed at read time from
//! `exam_attempts` and `learning_sessions` (see
//! `dg_db::models::performance`). The admin's view of the same student runs
//! the same derivation over the same rows, so the two screens cannot disagree
//! — which is why there is no `student_performance` table to keep in sync.
//!
//! # Not paginated
//!
//! A student has a handful of courses and each a handful of blocks. This is a
//! screen rendered whole, like the syllabus routes, not a feed. The admin's
//! equivalents stay paginated because an admin sees every student's worth.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dg_core::{
    stars_from_percentage, trophy_for, BlockId, Capability, CourseId, PerformanceLevel,
    PublicError, StudentId, Trophy,
};
use dg_db::models::performance as perf;

use crate::extractors::{AuthenticatedActor, JsonBody};
use crate::state::AppState;

/// Weak topics shown per block. The card shows a badge row, not a histogram.
const WEAK_TOPICS_PER_BLOCK: i64 = 5;

/// Longest remark accepted, in characters.
///
/// Counted in `chars()`, not bytes: a Malayalam remark is roughly three bytes
/// per character, and a byte cap would silently give a Malayalam-writing
/// student a third of the room an English-writing one gets.
const REMARK_MAX_CHARS: usize = 2000;

/// One block's card on the assessment page.
#[derive(Debug, Serialize)]
pub struct BlockPerformanceResponse {
    pub block_id: Uuid,
    pub block_no: i16,
    pub block_title: String,
    pub course_id: Uuid,
    pub course_code: String,
    pub course_name: String,
    pub semester_number: i16,

    pub exams_available: i64,
    pub attempts_total: i64,
    pub attempts_graded: i64,
    /// `null` until something is graded — never `0`, which would read as a
    /// measured score of zero.
    pub average_percentage: Option<f64>,
    pub best_percentage: Option<f64>,

    /// Derived server-side so this page and the admin's agree by construction.
    pub level: PerformanceLevel,
    pub level_label: String,
    /// `null` for an unassessed block, so the client renders an empty state
    /// rather than zero-of-five stars against work the student never sat.
    pub stars: Option<u8>,

    pub sessions_total: i64,
    pub sessions_completed: i64,
    pub active_voice_ms: i64,
    pub last_studied_at: Option<DateTime<Utc>>,

    pub weak_topics: Vec<WeakTopicResponse>,

    pub remark: Option<String>,
    pub remark_updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct WeakTopicResponse {
    pub topic: String,
    pub missed_count: i64,
}

/// The page header: how the student is doing overall.
#[derive(Debug, Serialize)]
pub struct PerformanceSummary {
    pub blocks_total: i64,
    /// Blocks with at least one graded attempt. The denominator for "how much
    /// of my course has actually been assessed".
    pub blocks_assessed: i64,
    pub attempts_graded: i64,
    pub average_percentage: Option<f64>,
    pub best_percentage: Option<f64>,
    pub level: PerformanceLevel,
    pub level_label: String,
    pub stars: Option<u8>,
    /// `null` until enough graded papers stand behind the average — see
    /// `dg_core::performance::trophy_for`. The client shows progress toward it
    /// rather than an empty medal.
    pub trophy: Option<Trophy>,
    /// How many more graded papers are needed before a trophy can be awarded,
    /// so the client can say "2 more to go" instead of leaving it unexplained.
    pub attempts_until_trophy: i64,
    pub total_active_voice_ms: i64,
}

#[derive(Debug, Serialize)]
pub struct PerformanceResponse {
    pub summary: PerformanceSummary,
    pub blocks: Vec<BlockPerformanceResponse>,
}

#[derive(Debug, Deserialize)]
pub struct PerformanceQuery {
    /// Narrows to one course. Optional — omitted returns every enrolled course,
    /// which is what the page opens with.
    #[serde(default)]
    pub course_id: Option<Uuid>,
}

/// Builds the response rows, fetching each block's weak topics.
///
/// Shared by the student's own route and the admin's view of one student, so
/// the two cannot drift apart in how they band or label the same numbers.
pub(crate) async fn build_rows(
    state: &AppState,
    student_id: StudentId,
    rows: Vec<perf::BlockPerformance>,
) -> Result<Vec<BlockPerformanceResponse>, PublicError> {
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        // Only ask for weak topics where an answer could have been marked
        // wrong. A block with no graded attempt cannot have any, and asking
        // would be one query per empty card.
        let weak_topics = if row.attempts_graded > 0 {
            perf::weak_topics_for_block(
                &state.pool,
                student_id,
                row.block_id,
                WEAK_TOPICS_PER_BLOCK,
            )
            .await?
            .into_iter()
            .map(|t| WeakTopicResponse {
                topic: t.topic,
                missed_count: t.missed_count,
            })
            .collect()
        } else {
            Vec::new()
        };

        let level = PerformanceLevel::from_percentage(row.average_percentage);
        out.push(BlockPerformanceResponse {
            block_id: row.block_id.into_uuid(),
            block_no: row.block_no,
            block_title: row.block_title,
            course_id: row.course_id.into_uuid(),
            course_code: row.course_code,
            course_name: row.course_name,
            semester_number: row.semester_number,
            exams_available: row.exams_available,
            attempts_total: row.attempts_total,
            attempts_graded: row.attempts_graded,
            average_percentage: row.average_percentage,
            best_percentage: row.best_percentage,
            level,
            level_label: level.label().to_string(),
            stars: stars_from_percentage(row.average_percentage),
            sessions_total: row.sessions_total,
            sessions_completed: row.sessions_completed,
            active_voice_ms: row.active_voice_ms,
            last_studied_at: row.last_studied_at,
            weak_topics,
            remark: row.remark,
            remark_updated_at: row.remark_updated_at,
        });
    }
    Ok(out)
}

/// The header figures, summarised from the rows already built.
///
/// Computed from the block rows rather than by a second aggregate query, so
/// the header can never contradict the cards underneath it. The overall
/// average is weighted by graded attempts — averaging the per-block averages
/// would let a block with one paper count as much as one with ten.
pub(crate) fn summarise(blocks: &[BlockPerformanceResponse]) -> PerformanceSummary {
    let attempts_graded: i64 = blocks.iter().map(|b| b.attempts_graded).sum();
    let blocks_assessed = blocks.iter().filter(|b| b.attempts_graded > 0).count() as i64;

    let weighted: f64 = blocks
        .iter()
        .filter_map(|b| b.average_percentage.map(|a| a * b.attempts_graded as f64))
        .sum();
    let average_percentage = (attempts_graded > 0).then(|| weighted / attempts_graded as f64);

    // `f64::max` rather than `Ord`: percentages are floats, and `NaN` cannot
    // arise here because every value came from a division by a `max_score`
    // the schema constrains to be positive.
    let best_percentage = blocks
        .iter()
        .filter_map(|b| b.best_percentage)
        .fold(None::<f64>, |acc, p| Some(acc.map_or(p, |a| a.max(p))));

    let level = PerformanceLevel::from_percentage(average_percentage);
    PerformanceSummary {
        blocks_total: blocks.len() as i64,
        blocks_assessed,
        attempts_graded,
        average_percentage,
        best_percentage,
        level,
        level_label: level.label().to_string(),
        stars: stars_from_percentage(average_percentage),
        trophy: trophy_for(average_percentage, attempts_graded),
        attempts_until_trophy: (dg_core::TROPHY_MIN_ATTEMPTS - attempts_graded).max(0),
        total_active_voice_ms: blocks.iter().map(|b| b.active_voice_ms).sum(),
    }
}

/// `GET /api/v1/student/performance`
pub async fn get_performance(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Query(query): Query<PerformanceQuery>,
) -> Result<Json<PerformanceResponse>, PublicError> {
    // Both capabilities, for the same reason `/student/dashboard` takes both:
    // the payload is academic context *and* the student's own exam record. An
    // admin holds the first and not the second, and is `403`.
    actor.require(Capability::ViewOwnContext)?;
    actor.require(Capability::ViewOwnExams)?;

    let student = super::own_student(&state, &actor).await?;
    let rows = perf::blocks_for_student(
        &state.pool,
        student.id,
        query.course_id.map(CourseId::from),
    )
    .await?;

    let blocks = build_rows(&state, student.id, rows).await?;
    Ok(Json(PerformanceResponse {
        summary: summarise(&blocks),
        blocks,
    }))
}

#[derive(Debug, Deserialize)]
pub struct SaveRemarkRequest {
    /// The student's own words. An empty or whitespace-only remark **clears**
    /// the row rather than storing a blank one: "I deleted what I wrote" and
    /// "I never wrote anything" are the same state to the student, and the
    /// page should render them identically.
    pub remark: String,
}

#[derive(Debug, Serialize)]
pub struct SaveRemarkResponse {
    pub block_id: Uuid,
    pub remark: Option<String>,
    pub remark_updated_at: Option<DateTime<Utc>>,
}

/// `PUT /api/v1/student/performance/blocks/{block_id}/remark`
///
/// Idempotent by construction — the same body twice leaves the same single
/// row — so a client may retry a dropped save without reasoning about
/// ordering, exactly as the exam answer save does.
pub async fn save_remark(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(block_id): Path<Uuid>,
    JsonBody(payload): JsonBody<SaveRemarkRequest>,
) -> Result<Json<SaveRemarkResponse>, PublicError> {
    actor.require(Capability::ViewOwnContext)?;

    let student = super::own_student(&state, &actor).await?;
    let block_id = BlockId::from(block_id);

    // The authorization gate. A block outside the student's own active
    // enrolments is indistinguishable from one that does not exist.
    if !perf::block_is_enrolled(&state.pool, student.id, block_id).await? {
        return Err(PublicError::NotFound);
    }

    let trimmed = payload.remark.trim();
    if trimmed.chars().count() > REMARK_MAX_CHARS {
        return Err(PublicError::validation(
            "remark",
            format!("A remark may be at most {REMARK_MAX_CHARS} characters."),
        ));
    }

    if trimmed.is_empty() {
        perf::delete_remark(&state.pool, student.id, block_id).await?;
        return Ok(Json(SaveRemarkResponse {
            block_id: block_id.into_uuid(),
            remark: None,
            remark_updated_at: None,
        }));
    }

    let updated_at = perf::upsert_remark(&state.pool, student.id, block_id, trimmed).await?;
    Ok(Json(SaveRemarkResponse {
        block_id: block_id.into_uuid(),
        remark: Some(trimmed.to_string()),
        remark_updated_at: Some(updated_at),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dg_core::{Actor, ProgramId, Role, UserId};

    fn block(avg: Option<f64>, graded: i64) -> BlockPerformanceResponse {
        let level = PerformanceLevel::from_percentage(avg);
        BlockPerformanceResponse {
            block_id: Uuid::new_v4(),
            block_no: 1,
            block_title: "Forest Resources".into(),
            course_id: Uuid::new_v4(),
            course_code: "EVS101".into(),
            course_name: "Environmental Studies".into(),
            semester_number: 1,
            exams_available: 1,
            attempts_total: graded,
            attempts_graded: graded,
            average_percentage: avg,
            best_percentage: avg,
            level,
            level_label: level.label().into(),
            stars: stars_from_percentage(avg),
            sessions_total: 0,
            sessions_completed: 0,
            active_voice_ms: 0,
            last_studied_at: None,
            weak_topics: vec![],
            remark: None,
            remark_updated_at: None,
        }
    }

    /// A student reads their own page; an admin holds `ViewOwnContext` but not
    /// `ViewOwnExams`, which is what keeps them off it.
    #[test]
    fn the_page_needs_both_of_the_students_own_capabilities() {
        let student = Actor::new(UserId::new(), Role::Student, vec![]);
        assert!(student.require(Capability::ViewOwnContext).is_ok());
        assert!(student.require(Capability::ViewOwnExams).is_ok());

        let admin = Actor::new(UserId::new(), Role::SubAdmin, vec![ProgramId::new()]);
        assert!(admin.require(Capability::ViewOwnExams).is_err());
    }

    /// The query and path shapes carry no student selector. Destructured
    /// exhaustively on purpose: adding a `student_id` stops this compiling.
    #[test]
    fn the_performance_request_cannot_name_another_student() {
        let PerformanceQuery { course_id } = PerformanceQuery { course_id: None };
        assert!(course_id.is_none());
        let SaveRemarkRequest { remark } = SaveRemarkRequest {
            remark: "mine".into(),
        };
        assert_eq!(remark, "mine");
    }

    /// Weighted by attempts, not a mean of means: a block with one paper must
    /// not count as much as one with nine.
    #[test]
    fn the_overall_average_is_weighted_by_graded_attempts() {
        let blocks = vec![block(Some(100.0), 1), block(Some(50.0), 9)];
        let summary = summarise(&blocks);
        assert_eq!(summary.attempts_graded, 10);
        // (100*1 + 50*9) / 10 = 55, not the 75 a mean of means would give.
        let avg = summary.average_percentage.expect("something is graded");
        assert!((avg - 55.0).abs() < 1e-9, "got {avg}");
    }

    /// An unassessed page is an empty state, not a zero score.
    #[test]
    fn a_student_with_nothing_graded_has_no_average_and_no_stars() {
        let blocks = vec![block(None, 0), block(None, 0)];
        let summary = summarise(&blocks);
        assert_eq!(summary.average_percentage, None);
        assert_eq!(summary.stars, None);
        assert_eq!(summary.trophy, None);
        assert_eq!(summary.blocks_assessed, 0);
        assert_eq!(summary.blocks_total, 2);
        assert_eq!(summary.level, PerformanceLevel::NotAssessed);
    }

    /// The countdown is what lets the client explain a missing trophy instead
    /// of just omitting it.
    #[test]
    fn the_trophy_countdown_reaches_zero_and_stops() {
        assert_eq!(summarise(&[block(Some(95.0), 1)]).attempts_until_trophy, 2);
        let earned = summarise(&[block(Some(95.0), 3)]);
        assert_eq!(earned.attempts_until_trophy, 0);
        assert_eq!(earned.trophy, Some(Trophy::Gold));
    }

    /// `best_percentage` is the best across blocks, and survives blocks that
    /// have no score at all.
    #[test]
    fn the_best_score_ignores_unassessed_blocks() {
        let blocks = vec![block(None, 0), block(Some(62.0), 2), block(Some(81.0), 1)];
        let summary = summarise(&blocks);
        assert_eq!(summary.best_percentage, Some(81.0));
    }
}
