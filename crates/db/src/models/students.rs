//! `students` — the student profile row, 1:1 with `users`.
//!
//! Self-service updates (`update_self_service`) are allow-listed to
//! `full_name`/`phone_number` at the call site (`apps/gateway`'s
//! `PATCH /me/profile` handler) — this module exposes a *separate* function
//! for academic-field updates (`update_academic`, admin-only) so there is no
//! single "update everything" function a caller could misuse to smuggle an
//! academic-field change through the self-service path.

use chrono::{DateTime, Utc};
use dg_core::{BlockId, LscId, ProgramId, SemesterId, StudentId, UserId};
use sqlx::PgPool;

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct Student {
    pub id: StudentId,
    pub user_id: UserId,
    pub full_name: String,
    pub roll_number: String,
    pub phone_number: String,
    pub program_id: ProgramId,
    pub semester_id: SemesterId,
    pub lsc_id: LscId,
    pub current_block_id: Option<BlockId>,
    pub is_first_login: bool,
    pub locale: String,
    pub timezone: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[allow(clippy::too_many_arguments)]
pub async fn create(
    pool: &PgPool,
    user_id: UserId,
    full_name: &str,
    roll_number: &str,
    phone_number: &str,
    program_id: ProgramId,
    semester_id: SemesterId,
    lsc_id: LscId,
) -> Result<Student> {
    sqlx::query_as!(
        Student,
        r#"
        INSERT INTO students
            (user_id, full_name, roll_number, phone_number, program_id, semester_id, lsc_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id, user_id, full_name, roll_number, phone_number, program_id, semester_id,
                  lsc_id, current_block_id as "current_block_id: BlockId", is_first_login, locale, timezone, created_at, updated_at
        "#,
        user_id.into_uuid(),
        full_name,
        roll_number,
        phone_number,
        program_id.into_uuid(),
        semester_id.into_uuid(),
        lsc_id.into_uuid()
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn find_by_id(pool: &PgPool, id: StudentId) -> Result<Option<Student>> {
    sqlx::query_as!(
        Student,
        r#"
        SELECT id, user_id, full_name, roll_number, phone_number, program_id, semester_id,
               lsc_id, current_block_id as "current_block_id: BlockId", is_first_login, locale, timezone, created_at, updated_at
        FROM students WHERE id = $1
        "#,
        id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn find_by_user_id(pool: &PgPool, user_id: UserId) -> Result<Option<Student>> {
    sqlx::query_as!(
        Student,
        r#"
        SELECT id, user_id, full_name, roll_number, phone_number, program_id, semester_id,
               lsc_id, current_block_id as "current_block_id: BlockId", is_first_login, locale, timezone, created_at, updated_at
        FROM students WHERE user_id = $1
        "#,
        user_id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn find_by_roll_number(pool: &PgPool, roll_number: &str) -> Result<Option<Student>> {
    sqlx::query_as!(
        Student,
        r#"
        SELECT id, user_id, full_name, roll_number, phone_number, program_id, semester_id,
               lsc_id, current_block_id as "current_block_id: BlockId", is_first_login, locale, timezone, created_at, updated_at
        FROM students WHERE roll_number = $1
        "#,
        roll_number
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn roll_number_taken(pool: &PgPool, roll_number: &str) -> Result<bool> {
    let row = sqlx::query!(
        r#"SELECT 1 as present FROM students WHERE roll_number = $1"#,
        roll_number
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(row.is_some())
}

/// Admin listing, optionally scoped to a set of programs (sub-admin RBAC).
/// `program_ids = None` means unrestricted (super-admin).
pub async fn list(
    pool: &PgPool,
    program_ids: Option<&[ProgramId]>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Student>> {
    match program_ids {
        Some(ids) => {
            let raw_ids: Vec<uuid::Uuid> = ids.iter().map(|p| (*p).into()).collect();
            sqlx::query_as!(
                Student,
                r#"
                SELECT id, user_id, full_name, roll_number, phone_number, program_id, semester_id,
                       lsc_id, current_block_id as "current_block_id: BlockId", is_first_login, locale, timezone, created_at, updated_at
                FROM students
                WHERE program_id = ANY($1)
                ORDER BY created_at DESC
                LIMIT $2 OFFSET $3
                "#,
                &raw_ids,
                limit,
                offset
            )
            .fetch_all(pool)
            .await
            .map_err(Error::from_sqlx)
        }
        None => sqlx::query_as!(
            Student,
            r#"
            SELECT id, user_id, full_name, roll_number, phone_number, program_id, semester_id,
                   lsc_id, current_block_id as "current_block_id: BlockId", is_first_login, locale, timezone, created_at, updated_at
            FROM students
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset
        )
        .fetch_all(pool)
        .await
        .map_err(Error::from_sqlx),
    }
}

/// Self-service update — allow-listed to `full_name`/`phone_number` at the
/// handler layer (`IMPLEMENTATION_PLAN.md` §4.1 item 3). This function
/// itself only ever touches those two columns, so there is no path from here
/// to an academic-field mutation.
pub async fn update_self_service(
    pool: &PgPool,
    id: StudentId,
    full_name: &str,
    phone_number: &str,
) -> Result<()> {
    sqlx::query!(
        r#"UPDATE students SET full_name = $2, phone_number = $3, updated_at = now() WHERE id = $1"#,
        id.into_uuid(),
        full_name,
        phone_number
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}

/// Admin-only academic-field update (`program_id`, `semester_id`, `lsc_id`).
/// Callers must have already enforced `409 SESSION_ACTIVE` if a learning
/// session is live (`IMPLEMENTATION_PLAN.md` §4.1 item 6) before calling this.
pub async fn update_academic(
    pool: &PgPool,
    id: StudentId,
    program_id: ProgramId,
    semester_id: SemesterId,
    lsc_id: LscId,
) -> Result<()> {
    sqlx::query!(
        r#"
        UPDATE students
        SET program_id = $2, semester_id = $3, lsc_id = $4, updated_at = now()
        WHERE id = $1
        "#,
        id.into_uuid(),
        program_id.into_uuid(),
        semester_id.into_uuid(),
        lsc_id.into_uuid()
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}

pub async fn set_current_block(pool: &PgPool, id: StudentId, block_id: BlockId) -> Result<()> {
    sqlx::query!(
        r#"UPDATE students SET current_block_id = $2, updated_at = now() WHERE id = $1"#,
        id.into_uuid(),
        block_id.into_uuid()
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}

/// Flip `is_first_login` to `false`. Must run in the same transaction that
/// opens the student's first learning session (NN-2) — callers should pass
/// an already-begun `Transaction` once the Phase 3 session-open path exists;
/// this scaffold exposes the plain-pool version for now.
pub async fn clear_first_login(pool: &PgPool, id: StudentId) -> Result<()> {
    sqlx::query!(
        r#"UPDATE students SET is_first_login = false, updated_at = now() WHERE id = $1"#,
        id.into_uuid()
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}
