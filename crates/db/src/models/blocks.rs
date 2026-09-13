//! `blocks` — a teaching unit inside a `course`; the context-persistence
//! target (`students.current_block_id`).

use dg_core::{BlockId, CourseId, EntityStatus, ProgramId, SemesterId};
use sqlx::PgPool;

use crate::error::{Error, Result};

/// A block carries its full ancestry — `course_id`, and the owning
/// `semester_id`/`program_id` denormalised from `courses` by every query in
/// this module. `blocks` stores only `course_id`; the join is here because
/// scope resolution already walks `block -> course -> program`, and an admin
/// console rendering a breadcrumb would otherwise need a request per level
/// just to name the block's own ancestors.
#[derive(Debug, Clone)]
pub struct Block {
    pub id: BlockId,
    pub course_id: CourseId,
    pub semester_id: SemesterId,
    pub program_id: ProgramId,
    pub block_no: i16,
    pub title: String,
    pub description: Option<String>,
    pub status: EntityStatus,
}


pub async fn create(
    pool: &PgPool,
    course_id: CourseId,
    block_no: i16,
    title: &str,
    description: Option<&str>,
) -> Result<Block> {
    sqlx::query_as!(
        Block,
        r#"
        WITH inserted AS (
            INSERT INTO blocks (course_id, block_no, title, description)
            VALUES ($1, $2, $3, $4)
            RETURNING id, course_id, block_no, title, description, status
        )
        SELECT b.id as "id!: BlockId", b.course_id as "course_id!: CourseId",
               c.semester_id as "semester_id!: SemesterId",
               c.program_id as "program_id!: ProgramId",
               b.block_no as "block_no!", b.title as "title!", b.description,
               b.status as "status!: EntityStatus"
        FROM inserted b
        JOIN courses c ON c.id = b.course_id
        "#,
        course_id.into_uuid(),
        block_no,
        title,
        description
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn find_by_id(pool: &PgPool, id: BlockId) -> Result<Option<Block>> {
    sqlx::query_as!(
        Block,
        r#"
        SELECT b.id as "id!: BlockId", b.course_id as "course_id!: CourseId",
               c.semester_id as "semester_id!: SemesterId",
               c.program_id as "program_id!: ProgramId",
               b.block_no as "block_no!", b.title as "title!", b.description,
               b.status as "status!: EntityStatus"
        FROM blocks b
        JOIN courses c ON c.id = b.course_id
        WHERE b.id = $1
        "#,
        id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn list_by_course(pool: &PgPool, course_id: CourseId) -> Result<Vec<Block>> {
    sqlx::query_as!(
        Block,
        r#"
        SELECT b.id as "id!: BlockId", b.course_id as "course_id!: CourseId",
               c.semester_id as "semester_id!: SemesterId",
               c.program_id as "program_id!: ProgramId",
               b.block_no as "block_no!", b.title as "title!", b.description,
               b.status as "status!: EntityStatus"
        FROM blocks b
        JOIN courses c ON c.id = b.course_id
        WHERE b.course_id = $1
        ORDER BY b.block_no
        "#,
        course_id.into_uuid()
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// The first block (`block_no = 1`) of a course — used to seed a freshly
/// enrolled student's `current_block_id`.
pub async fn find_first_in_course(pool: &PgPool, course_id: CourseId) -> Result<Option<Block>> {
    sqlx::query_as!(
        Block,
        r#"
        SELECT b.id as "id!: BlockId", b.course_id as "course_id!: CourseId",
               c.semester_id as "semester_id!: SemesterId",
               c.program_id as "program_id!: ProgramId",
               b.block_no as "block_no!", b.title as "title!", b.description,
               b.status as "status!: EntityStatus"
        FROM blocks b
        JOIN courses c ON c.id = b.course_id
        WHERE b.course_id = $1
        ORDER BY b.block_no ASC
        LIMIT 1
        "#,
        course_id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Partial update: `None` leaves the column as-is (`COALESCE`), so the admin
/// `PATCH /admin/blocks/{id}` handler can send only the fields the operator
/// actually edited. A consequence worth knowing: `description` can be set
/// and changed here but not cleared back to `NULL` — clearing it is not part
/// of the admin-console contract, and a sentinel for "explicit null" would
/// cost more than it buys.
pub async fn update(
    pool: &PgPool,
    id: BlockId,
    title: Option<&str>,
    description: Option<&str>,
    status: Option<EntityStatus>,
) -> Result<()> {
    sqlx::query!(
        r#"
        UPDATE blocks
        SET title = COALESCE($2, title),
            description = COALESCE($3, description),
            status = COALESCE($4::text::entity_status, status)
        WHERE id = $1
        "#,
        id.into_uuid(),
        title,
        description,
        status.map(EntityStatus::as_db_str)
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}

/// The `program_id` that owns `block_id`, resolved via `blocks -> courses`.
/// `None` if the block does not exist.
///
/// Mirrors `uploads::program_id_for_block` (which predates this module's
/// admin routes); both exist because the upload path and the blocks-CRUD
/// path need the same lookup and neither should depend on the other's
/// module. Kept as one query, not a block fetch plus a course fetch, so the
/// scope check costs a single round trip.
pub async fn program_id_for_block(pool: &PgPool, block_id: BlockId) -> Result<Option<ProgramId>> {
    let rec = sqlx::query!(
        r#"
        SELECT c.program_id as "program_id: ProgramId"
        FROM blocks b
        JOIN courses c ON c.id = b.course_id
        WHERE b.id = $1
        "#,
        block_id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)?;

    Ok(rec.map(|r| r.program_id))
}
