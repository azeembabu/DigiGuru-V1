//! `/api/v1/admin/students/{student_id}/performance` and
//! `/api/v1/admin/top-performers` — the admin's view of how students are doing.
//!
//! # The same derivation the student sees
//!
//! The per-student route runs `dg_db::models::performance` over the same
//! `exam_attempts` and `learning_sessions` rows the student's own assessment
//! page reads, and bands them with the same `dg_core::performance` helpers. So
//! an admin and a student looking at the same block see the same stars. That
//! is the point of having no stored performance table: there is no second copy
//! to fall behind.
//!
//! The student's remark is included. It is their own writing, shown to the
//! admin as context for the numbers — but there is deliberately **no admin
//! write path**: a remark an admin could edit would stop being the student's
//! own words.
//!
//! # Scoping
//!
//! Follows `/admin/analytics` exactly: the caller's role decides *which query
//! runs*, never a filter over a platform-wide result. A `sub_admin` is
//! restricted to its `sub_admin_scopes` programs through `students ->
//! program_id`, and a sub-admin with no scopes gets an empty result — the
//! correct answer, not a reason to fall back to the unscoped query. A student
//! outside the scope answers `404`, never `403`, because a `403` would confirm
//! the id exists.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dg_core::{
    stars_from_percentage, trophy_for, Capability, PerformanceLevel, PublicError, Role, StudentId,
    Trophy,
};
use dg_db::models::{performance as perf, student_reports as reports};

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;
use crate::student::performance::{build_rows, summarise, BlockPerformanceResponse};

/// Default size of the best-performers board — a leaderboard, not a list of
/// everyone.
const TOP_PERFORMERS_DEFAULT: i64 = 10;

/// The sub-admin scope for these reads, or `None` for the unscoped query.
///
/// Mirrors `student_reports::report_scope`. Kept separate rather than shared
/// because collapsing them would hide that each module's `Role::Student` arm
/// answers for different routes.
fn performance_scope(actor: &dg_core::Actor) -> Result<Option<Vec<Uuid>>, PublicError> {
    match actor.role {
        Role::SuperAdmin => Ok(None),
        Role::SubAdmin => Ok(Some(actor.scopes.iter().map(|p| p.into_uuid()).collect())),
        // Unreachable behind the `ManagePrograms` check every caller makes;
        // matched explicitly so a future capability change cannot silently
        // hand a student the unscoped query.
        Role::Student => Err(PublicError::Forbidden),
    }
}

/// One student's performance record, as the admin sees it.
#[derive(Debug, Serialize)]
pub struct StudentPerformanceResponse {
    pub student_id: Uuid,
    pub student_name: String,
    pub roll_number: String,
    pub program_name: String,
    pub semester_number: i16,
    pub lsc_code: Option<String>,
    pub summary: crate::student::performance::PerformanceSummary,
    pub blocks: Vec<BlockPerformanceResponse>,
}

/// `GET /api/v1/admin/students/{student_id}/performance`
pub async fn get_student_performance(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(student_id): Path<Uuid>,
) -> Result<Json<StudentPerformanceResponse>, PublicError> {
    actor.require(Capability::ManagePrograms)?;
    let scope = performance_scope(&actor)?;
    let student_id = StudentId::from(student_id);

    // The scope predicate lives in this query, so a student outside a
    // sub-admin's programs is `None` here and answers `404` — the same answer
    // a student id that was never real gets.
    let header = reports::report_header(&state.pool, student_id, scope.as_deref())
        .await?
        .ok_or(PublicError::NotFound)?;

    let rows = perf::blocks_for_student_scoped(&state.pool, student_id, scope.as_deref()).await?;
    let blocks = build_rows(&state, student_id, rows).await?;

    Ok(Json(StudentPerformanceResponse {
        student_id: header.student_id.into_uuid(),
        student_name: header.student_name,
        roll_number: header.roll_number,
        program_name: header.program_name,
        semester_number: header.semester_number,
        lsc_code: header.lsc_code,
        summary: summarise(&blocks),
        blocks,
    }))
}

/// One line of the best-performers board.
#[derive(Debug, Serialize)]
pub struct TopPerformerResponse {
    /// 1-based, assigned after ordering, so the client renders the position
    /// rather than inferring it from array order it might re-sort.
    pub rank: i64,
    pub student_id: Uuid,
    pub student_name: String,
    pub roll_number: String,
    pub program_name: String,
    pub semester_number: i16,
    pub attempts_graded: i64,
    pub average_percentage: f64,
    pub best_percentage: f64,
    pub level: PerformanceLevel,
    pub level_label: String,
    pub stars: Option<u8>,
    pub trophy: Option<Trophy>,
}

#[derive(Debug, Deserialize)]
pub struct TopPerformersQuery {
    #[serde(default)]
    pub limit: Option<i64>,
    /// Graded papers a student must have sat to be ranked at all. Defaults to
    /// `TROPHY_MIN_ATTEMPTS`, so the board and the trophy agree about what
    /// counts as enough evidence.
    #[serde(default)]
    pub min_attempts: Option<i64>,
}

/// `GET /api/v1/admin/top-performers`
///
/// Ranked by average across **graded** attempts only. Two deliberate
/// properties, both in `dg_db::models::performance::top_performers`:
/// unmarked papers never contribute, and a student with too few graded papers
/// does not appear at all — ranking one paper against twenty is a sampling
/// artefact, not a leaderboard.
pub async fn list_top_performers(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Query(query): Query<TopPerformersQuery>,
) -> Result<Json<Vec<TopPerformerResponse>>, PublicError> {
    actor.require(Capability::ManagePrograms)?;
    let scope = performance_scope(&actor)?;

    // Validated rather than clamped, like every other admin list: a clamped
    // page cannot be reconciled with what the caller asked for.
    let limit = match query.limit {
        None => TOP_PERFORMERS_DEFAULT,
        Some(n) if (1..=200).contains(&n) => n,
        Some(_) => {
            return Err(PublicError::validation(
                "limit",
                "limit must be between 1 and 200.",
            ))
        }
    };
    let min_attempts = match query.min_attempts {
        None => dg_core::TROPHY_MIN_ATTEMPTS,
        Some(n) if n >= 1 => n,
        Some(_) => {
            return Err(PublicError::validation(
                "min_attempts",
                "min_attempts must be 1 or greater.",
            ))
        }
    };

    let rows = perf::top_performers(&state.pool, scope.as_deref(), min_attempts, limit).await?;

    Ok(Json(
        rows.into_iter()
            .enumerate()
            .map(|(index, r)| {
                let avg = Some(r.average_percentage);
                let level = PerformanceLevel::from_percentage(avg);
                TopPerformerResponse {
                    rank: index as i64 + 1,
                    student_id: r.student_id.into_uuid(),
                    student_name: r.student_name,
                    roll_number: r.roll_number,
                    program_name: r.program_name,
                    semester_number: r.semester_number,
                    attempts_graded: r.attempts_graded,
                    average_percentage: r.average_percentage,
                    best_percentage: r.best_percentage,
                    level,
                    level_label: level.label().to_string(),
                    stars: stars_from_percentage(avg),
                    trophy: trophy_for(avg, r.attempts_graded),
                }
            })
            .collect(),
    ))
}

// -------------------------------------------------------- unit assessments
//
// The tutor's conversational verdicts for one student, unit by unit. Read-only
// to an admin: the verdict is the tutor's judgement of a conversation the admin
// was not part of, and an editable rating would be a different feature with a
// different audit story.
//
// Scoped exactly as `get_student_performance` above — the same
// `students -> program_id` path, with a student outside the scope answering
// `404` rather than `403`.

/// `GET /api/v1/admin/students/{student_id}/unit-assessments`
pub async fn get_student_unit_assessments(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(student_id): Path<Uuid>,
) -> Result<Json<Vec<crate::student::unit_assessments::UnitCardResponse>>, PublicError> {
    actor.require(Capability::ManagePrograms)?;
    let scope = performance_scope(&actor)?;
    let student_id = StudentId::from(student_id);

    // Resolved through the scoped header query first, so an out-of-scope
    // student is `404` before any assessment is read. Without this, an empty
    // list would be indistinguishable from "not yours".
    reports::report_header(&state.pool, student_id, scope.as_deref())
        .await?
        .ok_or(PublicError::NotFound)?;

    let rows =
        dg_db::models::unit_assessments::units_for_student(&state.pool, student_id, scope.as_deref())
            .await?;

    Ok(Json(
        rows.into_iter()
            .map(crate::student::unit_assessments::UnitCardResponse::from)
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dg_core::{Actor, ProgramId, UserId};

    #[test]
    fn a_student_can_never_reach_the_admin_performance_routes() {
        let student = Actor::new(UserId::new(), Role::Student, vec![]);
        assert!(student.require(Capability::ManagePrograms).is_err());
        assert!(performance_scope(&student).is_err());
    }

    /// A sub-admin gets its own programs; a super-admin gets the unscoped
    /// query. The empty-scope case is the one worth pinning: it must yield
    /// `Some([])`, which selects nothing, and never `None`, which would serve
    /// every student on the platform.
    #[test]
    fn an_unscoped_sub_admin_gets_an_empty_scope_not_a_platform_wide_one() {
        let sub = Actor::new(UserId::new(), Role::SubAdmin, vec![]);
        assert_eq!(performance_scope(&sub).expect("allowed"), Some(vec![]));

        let program = ProgramId::new();
        let scoped = Actor::new(UserId::new(), Role::SubAdmin, vec![program]);
        assert_eq!(
            performance_scope(&scoped).expect("allowed"),
            Some(vec![program.into_uuid()])
        );

        let super_admin = Actor::new(UserId::new(), Role::SuperAdmin, vec![]);
        assert_eq!(performance_scope(&super_admin).expect("allowed"), None);
    }
}
