//! `GET /api/v1/admin/analytics` — the dashboard's metrics and chart series.
//!
//! Additive to `GET /admin/stats`, which is unchanged and still served: the
//! existing tiles read it, so this is a second, richer endpoint rather than a
//! breaking expansion of the first.
//!
//! Scoping follows `stats.rs` exactly — the caller's **role** decides which
//! query runs, never a filter applied afterwards to a platform-wide result, so
//! a sub-admin cannot be handed totals it may not see even momentarily. `lscs`
//! stays platform-wide for both roles for the reason given there.
//!
//! Every counter is a non-negative integer and every series is present, so a
//! client never null-checks a metric. Averages and percentiles are the sole
//! `null`s: `0` would read as a measured zero, which is a different and wrong
//! claim than "nothing to average".

use axum::{extract::State, Json};
use serde::Serialize;

use dg_core::{Capability, PublicError, Role};
use dg_db::models::analytics;

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct CatalogueMetrics {
    pub programs: i64,
    pub semesters: i64,
    pub courses: i64,
    pub blocks: i64,
    pub blocks_active: i64,
    pub blocks_inactive: i64,
    pub lscs: i64,
    pub blocks_without_documents: i64,
    pub courses_without_blocks: i64,
    pub semesters_without_courses: i64,
}

#[derive(Debug, Serialize)]
pub struct StudentMetrics {
    pub total: i64,
    pub active: i64,
    pub inactive: i64,
    pub suspended: i64,
    pub first_login_pending: i64,
    pub new_last_30d: i64,
    pub without_enrollment: i64,
    pub enrollments_active: i64,
    pub enrollments_completed: i64,
    pub enrollments_dropped: i64,
}

#[derive(Debug, Serialize)]
pub struct DocumentMetrics {
    pub total: i64,
    pub pending: i64,
    pub parsing: i64,
    pub pending_review: i64,
    pub embedded: i64,
    pub failed: i64,
    pub total_pages: i64,
    pub avg_ocr_confidence: Option<f64>,
    pub jobs_pending: i64,
    pub jobs_processing: i64,
    pub jobs_completed: i64,
    pub jobs_failed: i64,
    pub jobs_retried: i64,
}

#[derive(Debug, Serialize)]
pub struct SessionMetrics {
    pub total: i64,
    pub in_progress: i64,
    pub completed: i64,
    pub abandoned: i64,
    pub last_7d: i64,
    pub active_voice_ms_total: i64,
    pub active_voice_ms_avg: Option<f64>,
    pub ended_quota: i64,
    pub ended_idle: i64,
    pub ended_user: i64,
    pub ended_jailbreak: i64,
    pub ended_error: i64,
}

#[derive(Debug, Serialize)]
pub struct WhiteboardMetrics {
    pub ops_total: i64,
    pub ops_acked: i64,
    pub ops_unacked: i64,
    pub ack_p50_ms: Option<f64>,
    pub ack_p95_ms: Option<f64>,
    pub violation_rate: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct SafetyMetrics {
    pub total: i64,
    pub tier0: i64,
    pub tier1: i64,
    pub tier2: i64,
    pub last_7d: i64,
    pub jailbreak: i64,
    pub toxicity: i64,
    pub out_of_scope: i64,
}

#[derive(Debug, Serialize)]
pub struct SessionDayPoint {
    pub day: chrono::NaiveDate,
    pub count: i64,
    pub voice_ms: i64,
}

#[derive(Debug, Serialize)]
pub struct DayPoint {
    pub day: chrono::NaiveDate,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct LabelPoint {
    pub label: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct SeriesResponse {
    pub sessions_daily: Vec<SessionDayPoint>,
    pub students_daily: Vec<DayPoint>,
    pub documents_daily: Vec<DayPoint>,
    pub incidents_daily: Vec<DayPoint>,
    pub documents_by_status: Vec<LabelPoint>,
    pub session_end_reasons: Vec<LabelPoint>,
    pub ack_latency_buckets: Vec<LabelPoint>,
    pub top_blocks: Vec<LabelPoint>,
}

#[derive(Debug, Serialize)]
pub struct AnalyticsResponse {
    pub catalogue: CatalogueMetrics,
    pub students: StudentMetrics,
    pub documents: DocumentMetrics,
    pub sessions: SessionMetrics,
    pub whiteboard: WhiteboardMetrics,
    pub safety: SafetyMetrics,
    pub series: SeriesResponse,
}

/// `ops_unacked / ops_total` as a 0..1 float — the same quantity CI tracks as
/// `wb_violation`. `None` with no ops at all: a board that never emitted is not
/// a board with a perfect record.
fn violation_rate(total: i64, unacked: i64) -> Option<f64> {
    (total > 0).then(|| unacked as f64 / total as f64)
}

fn label_points(rows: Vec<analytics::LabelCount>) -> Vec<LabelPoint> {
    rows.into_iter()
        .map(|r| LabelPoint {
            label: r.label,
            count: r.count,
        })
        .collect()
}

fn day_points(rows: Vec<analytics::DayCount>) -> Vec<DayPoint> {
    rows.into_iter()
        .map(|r| DayPoint {
            day: r.day,
            count: r.count,
        })
        .collect()
}

impl From<analytics::AdminAnalytics> for AnalyticsResponse {
    fn from(a: analytics::AdminAnalytics) -> Self {
        let c = a.counts;
        let s = a.series;
        Self {
            catalogue: CatalogueMetrics {
                programs: c.programs,
                semesters: c.semesters,
                courses: c.courses,
                blocks: c.blocks,
                blocks_active: c.blocks_active,
                blocks_inactive: c.blocks_inactive,
                lscs: c.lscs,
                blocks_without_documents: c.blocks_without_documents,
                courses_without_blocks: c.courses_without_blocks,
                semesters_without_courses: c.semesters_without_courses,
            },
            students: StudentMetrics {
                total: c.students_total,
                active: c.students_active,
                inactive: c.students_inactive,
                suspended: c.students_suspended,
                first_login_pending: c.students_first_login_pending,
                new_last_30d: c.students_new_last_30d,
                without_enrollment: c.students_without_enrollment,
                enrollments_active: c.enrollments_active,
                enrollments_completed: c.enrollments_completed,
                enrollments_dropped: c.enrollments_dropped,
            },
            documents: DocumentMetrics {
                total: c.documents_total,
                pending: c.documents_pending,
                parsing: c.documents_parsing,
                pending_review: c.documents_pending_review,
                embedded: c.documents_embedded,
                failed: c.documents_failed,
                total_pages: c.documents_total_pages,
                avg_ocr_confidence: c.documents_avg_ocr_confidence,
                jobs_pending: c.jobs_pending,
                jobs_processing: c.jobs_processing,
                jobs_completed: c.jobs_completed,
                jobs_failed: c.jobs_failed,
                jobs_retried: c.jobs_retried,
            },
            sessions: SessionMetrics {
                total: c.sessions_total,
                in_progress: c.sessions_in_progress,
                completed: c.sessions_completed,
                abandoned: c.sessions_abandoned,
                last_7d: c.sessions_last_7d,
                active_voice_ms_total: c.active_voice_ms_total,
                active_voice_ms_avg: c.active_voice_ms_avg,
                ended_quota: c.ended_quota,
                ended_idle: c.ended_idle,
                ended_user: c.ended_user,
                ended_jailbreak: c.ended_jailbreak,
                ended_error: c.ended_error,
            },
            whiteboard: WhiteboardMetrics {
                ops_total: c.ops_total,
                ops_acked: c.ops_acked,
                ops_unacked: c.ops_unacked,
                ack_p50_ms: c.ack_p50_ms,
                ack_p95_ms: c.ack_p95_ms,
                violation_rate: violation_rate(c.ops_total, c.ops_unacked),
            },
            safety: SafetyMetrics {
                total: c.safety_total,
                tier0: c.safety_tier0,
                tier1: c.safety_tier1,
                tier2: c.safety_tier2,
                last_7d: c.safety_last_7d,
                jailbreak: c.safety_jailbreak,
                toxicity: c.safety_toxicity,
                out_of_scope: c.safety_out_of_scope,
            },
            series: SeriesResponse {
                sessions_daily: s
                    .sessions_daily
                    .into_iter()
                    .map(|r| SessionDayPoint {
                        day: r.day,
                        count: r.count,
                        voice_ms: r.voice_ms,
                    })
                    .collect(),
                students_daily: day_points(s.students_daily),
                documents_daily: day_points(s.documents_daily),
                incidents_daily: day_points(s.incidents_daily),
                documents_by_status: label_points(s.documents_by_status),
                session_end_reasons: label_points(s.session_end_reasons),
                ack_latency_buckets: label_points(s.ack_latency_buckets),
                top_blocks: label_points(s.top_blocks),
            },
        }
    }
}

pub async fn get_analytics(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
) -> Result<Json<AnalyticsResponse>, PublicError> {
    actor.require(Capability::ManagePrograms)?;

    let data = match actor.role {
        Role::SuperAdmin => analytics::counts_all(&state.pool).await,
        Role::SubAdmin => analytics::counts_scoped(&state.pool, &actor.scopes).await,
        // Unreachable via the capability check above; matched explicitly so a
        // future capability change cannot silently hand a student the
        // unscoped query.
        Role::Student => return Err(PublicError::Forbidden),
    }
    .map_err(PublicError::from)?;

    Ok(Json(data.into()))
}

#[cfg(test)]
mod tests {
    use super::violation_rate;
    use dg_core::{Actor, Capability, ProgramId, Role, UserId};

    /// Which query the handler will run for a given actor — the decision that
    /// keeps platform-wide figures away from a scoped sub-admin.
    #[derive(Debug, PartialEq, Eq)]
    enum Query {
        All,
        Scoped,
        Forbidden,
    }

    fn plan(actor: &Actor) -> Query {
        if actor.require(Capability::ManagePrograms).is_err() {
            return Query::Forbidden;
        }
        match actor.role {
            Role::SuperAdmin => Query::All,
            Role::SubAdmin => Query::Scoped,
            Role::Student => Query::Forbidden,
        }
    }

    #[test]
    fn super_admin_gets_platform_wide_analytics() {
        let actor = Actor::new(UserId::new(), Role::SuperAdmin, vec![]);
        assert_eq!(plan(&actor), Query::All);
    }

    #[test]
    fn sub_admin_gets_analytics_scoped_to_its_programs() {
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![ProgramId::new()]);
        assert_eq!(plan(&actor), Query::Scoped);
    }

    #[test]
    fn sub_admin_with_no_scopes_still_gets_the_scoped_query() {
        // Not the unscoped one: no scopes means zeroes, never everything.
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![]);
        assert_eq!(plan(&actor), Query::Scoped);
    }

    #[test]
    fn student_is_forbidden_analytics() {
        let actor = Actor::new(UserId::new(), Role::Student, vec![]);
        assert_eq!(plan(&actor), Query::Forbidden);
    }

    #[test]
    fn violation_rate_is_null_when_no_ops_were_emitted() {
        assert_eq!(violation_rate(0, 0), None);
    }

    #[test]
    fn violation_rate_is_zero_when_every_op_was_acked() {
        assert_eq!(violation_rate(10, 0), Some(0.0));
    }

    #[test]
    fn violation_rate_is_the_unacked_fraction_of_all_ops() {
        assert_eq!(violation_rate(8, 2), Some(0.25));
    }
}
