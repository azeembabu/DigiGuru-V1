//! `GET /api/v1/student/dashboard` — the whole student dashboard in one
//! request.
//!
//! Six components (identity, continue-learning hero, quota ring, recent exam
//! cards, saved resources, revision count) are hydrated by one call rather
//! than six, because they render as one screen: six requests would paint the
//! page in six stages and each would have to re-resolve the same
//! `students` row.
//!
//! # Self-only
//!
//! As everywhere in `student/`, the subject is the caller's own token. There is
//! no query, path, or body parameter on this route at all, so there is no
//! request a client can make for another student's dashboard.
//!
//! # The quota ring is a *read*
//!
//! NN-3 is server-authoritative and owned by the socket task that holds the
//! live session: only that task may charge the ledger. This handler reads the
//! same Redis key with [`quota::QuotaStore::get_ms`], which neither increments
//! it nor refreshes its TTL, and derives `ms_remaining`/`is_locked` from
//! [`quota::DAILY_QUOTA_MS`] — the one constant the ledger itself enforces
//! against. `resets_at` is the next midnight in `students.timezone`, computed
//! by `quota::next_local_midnight`, so a student is never told their allowance
//! resets at a UTC hour that is mid-afternoon for them.

use axum::{extract::State, Json};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use dg_core::{Capability, ExamAttemptStatus, PublicError};
use dg_db::models::{exam_papers, exams, flashcards, programs, semesters};
use quota::{QuotaStore, DAILY_QUOTA_MS};

use super::queries;
use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

/// How many score cards the dashboard shows.
const RECENT_EXAM_CARDS: i64 = 3;

/// The daily allowance in whole minutes, for the ring's "N / 20" label. Derived
/// from the ledger's own constant so the two can never disagree.
const DAILY_QUOTA_MINUTES: i64 = DAILY_QUOTA_MS / 60_000;

#[derive(Debug, Serialize)]
pub struct StudentInfo {
    pub name: String,
    pub roll_number: String,
    pub program: String,
    pub program_id: Uuid,
    pub semester: i16,
    /// Always `null` today: there is no avatar column and no upload route, and
    /// the field is here so the dashboard's header does not change shape when
    /// one lands.
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ContinueLearning {
    pub session_id: Uuid,
    pub course_id: Uuid,
    pub course_title: String,
    pub block_id: Uuid,
    pub block_no: i16,
    pub block_title: String,
    /// `learning_sessions.last_topic` — the chapter the tutor was on.
    pub chapter_name: Option<String>,
    /// `0` when the session never recorded a page, so the hero always has a
    /// number to render.
    pub page_number: i32,
    /// Always `null` here. Paragraph position lives in the Redis session state
    /// (`sess:{session_id}`), not in Postgres, and resurrecting it outside a
    /// live socket would mean reporting a position the classroom may have
    /// already moved past. Resume targets course -> block -> chapter -> page.
    pub para_index: Option<i32>,
    /// Blocks of this course with a `completed` session / blocks in the course,
    /// as a percentage. A course with no blocks reads `0.0` rather than
    /// dividing by zero.
    pub progress_percentage: f64,
    pub resume_summary: Option<String>,
    pub last_active_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct QuotaStatus {
    pub minutes_used: i64,
    pub minutes_max: i64,
    pub ms_used: i64,
    pub ms_remaining: i64,
    /// True when today's allowance is spent. The gateway would refuse a new
    /// voice session now; the dashboard greys the Study Module entry.
    pub is_locked: bool,
    /// The instant the allowance resets — next midnight in `timezone`.
    pub resets_at: DateTime<Utc>,
    /// The student's own IANA zone, echoed so the client can render "resets in
    /// N hours" without guessing which zone `resets_at` was computed in.
    pub timezone: String,
}

#[derive(Debug, Serialize)]
pub struct ExamHistoryCard {
    pub attempt_id: Uuid,
    pub exam_id: Uuid,
    pub exam_title: String,
    /// `null` until the attempt is marked.
    pub score: Option<f64>,
    pub max_score: f64,
    /// `score / max_score * 100`, `null` whenever `score` is.
    pub percentage: Option<f64>,
    pub status: ExamAttemptStatus,
    pub submitted_at: Option<DateTime<Utc>>,
    /// Distinct topics answered incorrectly — the card's weak-area badges.
    /// Empty for an attempt that is not graded yet: before submission, which
    /// answers are wrong is part of the answer key.
    pub weak_topics: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SavedResources {
    pub preserved_notes_count: i64,
    pub flashcards_total: i64,
    pub flashcards_due_for_review: i64,
    pub revisions_due_count: i64,
}

#[derive(Debug, Serialize)]
pub struct DashboardResponse {
    pub student_info: StudentInfo,
    /// `null` for a student with no sessions yet — a first-login dashboard
    /// renders an empty state, it does not fail.
    pub continue_learning: Option<ContinueLearning>,
    pub quota_status: QuotaStatus,
    pub exam_history: Vec<ExamHistoryCard>,
    pub saved_resources: SavedResources,
}

/// `blocks_completed / blocks_total * 100`, clamped to 0..100.
///
/// "Completed" is a `learning_sessions` row with `status = 'completed'` for the
/// block, counted distinctly so re-studying a block does not push progress
/// past 100. A course with no blocks is 0 %, not a division by zero.
fn progress_percentage(blocks_completed: i64, blocks_total: i64) -> f64 {
    if blocks_total <= 0 {
        return 0.0;
    }
    let pct = (blocks_completed as f64) * 100.0 / (blocks_total as f64);
    pct.clamp(0.0, 100.0)
}

/// `score / max_score * 100`, or `None` when the attempt is not marked.
fn percentage(score: Option<f64>, max_score: f64) -> Option<f64> {
    let score = score?;
    if max_score <= 0.0 {
        return None;
    }
    Some(score * 100.0 / max_score)
}

pub async fn get_dashboard(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
) -> Result<Json<DashboardResponse>, PublicError> {
    // The dashboard serves both the academic payload and the student's exam
    // record, so it requires both self-only capabilities. An admin holds
    // neither (`ViewOwnExams` is explicitly false for a sub-admin) and is
    // rejected here rather than 404-ing on a missing `students` row.
    actor.require(Capability::ViewOwnContext)?;
    actor.require(Capability::ViewOwnExams)?;

    let student = super::own_student(&state, &actor).await?;

    let program = programs::find_by_id(&state.pool, student.program_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::Internal)?;
    let semester = semesters::find_by_id(&state.pool, student.semester_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::Internal)?;

    let tz = quota::resolve_timezone(&student.timezone);
    let now = Utc::now();
    let today = quota::local_date(now, tz);

    // A pure read of the NN-3 ledger: `get_ms` does not increment the counter
    // and does not touch its TTL. Charging is the live socket's job alone.
    let store = quota::RedisQuotaStore::new(state.redis.clone());
    let ms_used = store
        .get_ms(&quota::ledger_key(student.id, today))
        .await
        .map_err(|e| {
            tracing::error!(error = %e, student_id = %student.id, "quota ledger read failed");
            PublicError::Unavailable
        })?;
    let ms_remaining = (DAILY_QUOTA_MS - ms_used).max(0);

    let continue_learning = queries::latest_resume_point(&state.pool, student.id)
        .await?
        .map(|r| ContinueLearning {
            session_id: r.session_id.into_uuid(),
            course_id: r.course_id.into_uuid(),
            course_title: r.course_title,
            block_id: r.block_id.into_uuid(),
            block_no: r.block_no,
            block_title: r.block_title,
            chapter_name: r.chapter_name,
            page_number: r.page_number.unwrap_or(0),
            para_index: None,
            progress_percentage: progress_percentage(r.blocks_completed, r.blocks_total),
            resume_summary: r.resume_summary,
            last_active_at: r.last_active_at,
        });

    let attempts = exams::attempts_for_student(&state.pool, student.id, RECENT_EXAM_CARDS, 0)
        .await
        .map_err(PublicError::from)?;

    let mut exam_history = Vec::with_capacity(attempts.len());
    for a in attempts {
        // Weak topics only exist once an attempt has been marked; the query
        // refuses an `in_progress` attempt in SQL, so this is not a branch a
        // reviewer has to trust (`exam_papers::weak_topics_for_attempt`).
        let weak_topics = match a.status {
            ExamAttemptStatus::InProgress => Vec::new(),
            _ => exam_papers::weak_topics_for_attempt(&state.pool, student.id, a.id)
                .await
                .map_err(PublicError::from)?,
        };
        exam_history.push(ExamHistoryCard {
            attempt_id: a.id.into_uuid(),
            exam_id: a.exam_id.into_uuid(),
            exam_title: a.exam_title,
            score: a.score,
            max_score: a.max_score,
            percentage: percentage(a.score, a.max_score),
            status: a.status,
            submitted_at: a.submitted_at,
            weak_topics,
        });
    }

    let cards = flashcards::counts_for_student(&state.pool, student.id, today)
        .await
        .map_err(PublicError::from)?;
    let notes = queries::note_counts(&state.pool, student.id, today).await?;

    Ok(Json(DashboardResponse {
        student_info: StudentInfo {
            name: student.full_name,
            roll_number: student.roll_number,
            program: program.name,
            program_id: program.id.into_uuid(),
            semester: semester.semester_number,
            avatar_url: None,
        },
        continue_learning,
        quota_status: QuotaStatus {
            minutes_used: ms_used / 60_000,
            minutes_max: DAILY_QUOTA_MINUTES,
            ms_used,
            ms_remaining,
            is_locked: ms_remaining == 0,
            resets_at: quota::next_local_midnight(now, tz),
            timezone: student.timezone,
        },
        exam_history,
        saved_resources: SavedResources {
            preserved_notes_count: notes.preserved,
            flashcards_total: cards.total,
            flashcards_due_for_review: cards.due,
            revisions_due_count: notes.due,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use dg_core::{Actor, ProgramId, Role, UserId};

    #[test]
    fn a_student_may_read_its_own_dashboard() {
        let actor = Actor::new(UserId::new(), Role::Student, vec![]);
        assert!(actor.require(Capability::ViewOwnContext).is_ok());
        assert!(actor.require(Capability::ViewOwnExams).is_ok());
    }

    #[test]
    fn a_sub_admin_is_forbidden_the_student_dashboard() {
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![ProgramId::new()]);
        // It holds `ViewOwnContext`, so the second requirement is the one that
        // stops it — which is why the handler asserts both.
        let err = actor
            .require(Capability::ViewOwnExams)
            .expect_err("an admin has no own exam record");
        assert_eq!(err.code(), "FORBIDDEN");
    }

    #[test]
    fn progress_is_zero_for_a_course_with_no_blocks() {
        assert_eq!(progress_percentage(0, 0), 0.0);
        assert_eq!(progress_percentage(3, 0), 0.0, "never divides by zero");
    }

    #[test]
    fn progress_is_completed_blocks_over_total_blocks() {
        assert_eq!(progress_percentage(3, 4), 75.0);
        assert_eq!(progress_percentage(0, 4), 0.0);
        assert_eq!(progress_percentage(4, 4), 100.0);
    }

    #[test]
    fn progress_never_exceeds_one_hundred_percent() {
        // Defensive: the query counts distinct blocks, so this should be
        // unreachable — but a percentage above 100 would break the ring.
        assert_eq!(progress_percentage(9, 4), 100.0);
    }

    #[test]
    fn percentage_is_none_for_an_unmarked_attempt() {
        assert_eq!(percentage(None, 20.0), None);
        assert_eq!(percentage(Some(18.0), 0.0), None, "no division by zero");
        assert_eq!(percentage(Some(18.0), 20.0), Some(90.0));
    }

    #[test]
    fn the_quota_ring_maximum_comes_from_the_ledger_constant() {
        assert_eq!(DAILY_QUOTA_MINUTES, 20);
    }

    /// NN-3: the reset instant is midnight in the *student's* zone, not UTC.
    /// A student in Kolkata at 20:00 local resets at 18:30 UTC, not 00:00 UTC.
    #[test]
    fn the_reset_instant_is_midnight_in_the_students_own_timezone() {
        let tz = quota::resolve_timezone("Asia/Kolkata");
        let now = Utc
            .with_ymd_and_hms(2026, 9, 13, 14, 30, 0)
            .single()
            .expect("a real instant");
        let resets_at = quota::next_local_midnight(now, tz);

        assert!(resets_at > now);
        assert_eq!(
            resets_at,
            Utc.with_ymd_and_hms(2026, 9, 13, 18, 30, 0)
                .single()
                .expect("a real instant"),
            "IST is UTC+5:30, so the student's next midnight is 18:30 UTC"
        );
        // And the date itself is the student's, not the server's: at 19:00 UTC
        // it is already tomorrow in Kolkata, which is the exact case a UTC
        // ledger would charge to the wrong day.
        let late = Utc
            .with_ymd_and_hms(2026, 9, 13, 19, 0, 0)
            .single()
            .expect("a real instant");
        assert_ne!(
            quota::local_date(late, tz),
            quota::local_date(late, quota::resolve_timezone("UTC")),
            "00:30 IST on the 14th is still the 13th in UTC"
        );
        assert_eq!(
            quota::local_date(late, tz),
            chrono::NaiveDate::from_ymd_opt(2026, 9, 14).expect("a real date")
        );
    }

    /// An unparseable `students.timezone` must not fail the dashboard: it falls
    /// back to the platform default rather than 500-ing.
    #[test]
    fn an_unrecognised_timezone_falls_back_rather_than_failing() {
        assert_eq!(
            quota::resolve_timezone("Mars/Olympus_Mons"),
            quota::DEFAULT_TIMEZONE
        );
    }

    #[test]
    fn a_locked_quota_reports_zero_remaining_not_a_negative() {
        let ms_used = DAILY_QUOTA_MS + 5_000;
        let ms_remaining = (DAILY_QUOTA_MS - ms_used).max(0);
        assert_eq!(ms_remaining, 0);
        assert!(ms_remaining == 0, "is_locked is derived from exactly this");
    }

    /// The contract's first-login requirement: a student with no sessions gets
    /// `continue_learning: null`, and the payload still serialises.
    #[test]
    fn a_first_login_dashboard_serialises_with_a_null_continue_learning() {
        let json = serde_json::to_value(DashboardResponse {
            student_info: StudentInfo {
                name: "Asha".into(),
                roll_number: "BA-ML-0001".into(),
                program: "BA Malayalam".into(),
                program_id: Uuid::nil(),
                semester: 1,
                avatar_url: None,
            },
            continue_learning: None,
            quota_status: QuotaStatus {
                minutes_used: 0,
                minutes_max: DAILY_QUOTA_MINUTES,
                ms_used: 0,
                ms_remaining: DAILY_QUOTA_MS,
                is_locked: false,
                resets_at: Utc::now(),
                timezone: "Asia/Kolkata".into(),
            },
            exam_history: Vec::new(),
            saved_resources: SavedResources {
                preserved_notes_count: 0,
                flashcards_total: 0,
                flashcards_due_for_review: 0,
                revisions_due_count: 0,
            },
        })
        .expect("serialises");

        assert!(json["continue_learning"].is_null());
        assert!(json["exam_history"].is_array());
        assert_eq!(json["quota_status"]["minutes_max"], 20);
        // Self-only: the payload names no student id anywhere.
        assert!(json["student_info"].get("student_id").is_none());
        assert!(json.get("student_id").is_none());
    }
}
