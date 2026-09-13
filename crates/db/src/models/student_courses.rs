//! `student_courses` — which courses a student may actually study. This is
//! the enforcement point for "a student reaches content only through
//! `student_courses`" (`CLAUDE.md`).

use chrono::{DateTime, Utc};
use dg_core::{CourseId, EnrollmentStatus, SemesterId, StudentId};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct StudentCourse {
    pub id: Uuid,
    pub student_id: StudentId,
    pub course_id: CourseId,
    pub status: EnrollmentStatus,
    pub assigned_at: DateTime<Utc>,
}

pub async fn assign(pool: &PgPool, student_id: StudentId, course_id: CourseId) -> Result<StudentCourse> {
    sqlx::query_as!(
        StudentCourse,
        r#"
        INSERT INTO student_courses (student_id, course_id)
        VALUES ($1, $2)
        RETURNING id, student_id, course_id, status as "status: EnrollmentStatus", assigned_at
        "#,
        student_id.into_uuid(),
        course_id.into_uuid()
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Enrols a student in every course of one semester, returning how many rows
/// were created.
///
/// This is what makes self-registration usable without an administrator: a
/// student reaches content only through `student_courses` (`CLAUDE.md`), so a
/// signed-up student with no rows here has a dashboard with nothing on it and
/// no classroom they may enter. The semester is the right granularity because
/// it is what the student chose at signup, and courses hang off it.
///
/// One statement, not a loop: the set is decided and inserted by the database
/// in a single round trip, so it cannot half-apply, and a course added to the
/// semester between the read and the write cannot produce a partial enrolment.
/// `ON CONFLICT DO NOTHING` makes it idempotent — re-running for a student who
/// is already enrolled is a no-op rather than a unique violation, which matters
/// because an administrator may have assigned some of these by hand.
pub async fn enroll_in_semester(
    pool: &PgPool,
    student_id: StudentId,
    semester_id: SemesterId,
) -> Result<u64> {
    let result = sqlx::query!(
        r#"
        INSERT INTO student_courses (student_id, course_id)
        SELECT $1, c.id FROM courses c WHERE c.semester_id = $2
        ON CONFLICT (student_id, course_id) DO NOTHING
        "#,
        student_id.into_uuid(),
        semester_id.into_uuid()
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(result.rows_affected())
}

pub async fn list_by_student(pool: &PgPool, student_id: StudentId) -> Result<Vec<StudentCourse>> {
    sqlx::query_as!(
        StudentCourse,
        r#"
        SELECT id, student_id, course_id, status as "status: EnrollmentStatus", assigned_at
        FROM student_courses WHERE student_id = $1
        "#,
        student_id.into_uuid()
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// The check that keeps a student inside their own syllabus: is `course_id`
/// one this student is actively enrolled in?
pub async fn is_enrolled(pool: &PgPool, student_id: StudentId, course_id: CourseId) -> Result<bool> {
    let row = sqlx::query!(
        r#"
        SELECT 1 as present FROM student_courses
        WHERE student_id = $1 AND course_id = $2 AND status = 'active'
        "#,
        student_id.into_uuid(),
        course_id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(row.is_some())
}

pub async fn update_status(
    pool: &PgPool,
    student_id: StudentId,
    course_id: CourseId,
    status: EnrollmentStatus,
) -> Result<()> {
    sqlx::query!(
        r#"UPDATE student_courses SET status = $3::text::enrollment_status WHERE student_id = $1 AND course_id = $2"#,
        student_id.into_uuid(),
        course_id.into_uuid(),
        status.as_db_str()
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}
