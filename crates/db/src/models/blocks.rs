//! `blocks` — a teaching unit inside a `course`; the context-persistence
//! target (`students.current_block_id`).

use dg_core::{BlockId, CourseId};
use sqlx::PgPool;

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct Block {
    pub id: BlockId,
    pub course_id: CourseId,
    pub block_no: i16,
    pub title: String,
}

pub async fn create(pool: &PgPool, course_id: CourseId, block_no: i16, title: &str) -> Result<Block> {
    sqlx::query_as!(
        Block,
        r#"
        INSERT INTO blocks (course_id, block_no, title)
        VALUES ($1, $2, $3)
        RETURNING id, course_id, block_no, title
        "#,
        course_id.into_uuid(),
        block_no,
        title
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn find_by_id(pool: &PgPool, id: BlockId) -> Result<Option<Block>> {
    sqlx::query_as!(
        Block,
        r#"SELECT id, course_id, block_no, title FROM blocks WHERE id = $1"#,
        id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn list_by_course(pool: &PgPool, course_id: CourseId) -> Result<Vec<Block>> {
    sqlx::query_as!(
        Block,
        r#"SELECT id, course_id, block_no, title FROM blocks WHERE course_id = $1 ORDER BY block_no"#,
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
        SELECT id, course_id, block_no, title
        FROM blocks WHERE course_id = $1
        ORDER BY block_no ASC
        LIMIT 1
        "#,
        course_id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn update(pool: &PgPool, id: BlockId, title: &str) -> Result<()> {
    sqlx::query!(r#"UPDATE blocks SET title = $2 WHERE id = $1"#, id.into_uuid(), title)
        .execute(pool)
        .await
        .map_err(Error::from_sqlx)?;
    Ok(())
}
