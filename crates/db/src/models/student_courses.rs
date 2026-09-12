//! `student_courses` — which courses a student may actually study. This is
//! the enforcement point for "a student reaches content only through
//! `student_courses`" (`CLAUDE.md`).

use chrono::{DateTime, Utc};
use dg_core::{CourseId, EnrollmentStatus, StudentId};
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
