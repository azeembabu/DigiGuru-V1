//! Aggregate counts for the admin dashboard (`GET /api/v1/admin/stats`).
//!
//! Every figure comes from **one** round trip — a single `SELECT` of
//! correlated sub-queries — rather than a count per entity, so adding a tile
//! to the dashboard never turns into an N+1 fan-out.
//!
//! Two variants of that query exist because a sub-admin must never see
//! platform-wide totals: `counts_scoped` restricts every academic figure to
//! the caller's `sub_admin_scopes` programs, walking
//! `blocks -> courses -> program_id` and `documents -> blocks -> courses ->
//! program_id` for the entities that have no `program_id` of their own.
//! `lscs` is the one exception: an LSC has no program (see
//! `models::lscs`), and every admin can already list all of them, so its
//! count is platform-wide in both variants.

use dg_core::ProgramId;
use sqlx::PgPool;

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct AdminStats {
    pub programs: i64,
    pub semesters: i64,
    pub courses: i64,
    pub blocks: i64,
    pub lscs: i64,
    pub students: i64,
    pub documents_total: i64,
    pub documents_pending_review: i64,
    pub documents_embedded: i64,
    pub documents_failed: i64,
}

/// Platform-wide counts. Super-admin only — `counts_scoped` is the sub-admin
/// path.
pub async fn counts_all(pool: &PgPool) -> Result<AdminStats> {
    sqlx::query_as!(
        AdminStats,
        r#"
        SELECT
            (SELECT count(*) FROM programs)  as "programs!",
            (SELECT count(*) FROM semesters) as "semesters!",
            (SELECT count(*) FROM courses)   as "courses!",
            (SELECT count(*) FROM blocks)    as "blocks!",
            (SELECT count(*) FROM lscs)      as "lscs!",
            (SELECT count(*) FROM students)  as "students!",
            (SELECT count(*) FROM documents) as "documents_total!",
            (SELECT count(*) FROM documents WHERE status = 'pending_review')
                as "documents_pending_review!",
            (SELECT count(*) FROM documents WHERE status = 'embedded')
                as "documents_embedded!",
            (SELECT count(*) FROM documents WHERE status = 'failed')
                as "documents_failed!"
        "#
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// The same counts, restricted to `program_ids`. An empty slice yields all
/// zeroes (except `lscs`), which is the correct answer for a sub-admin with
/// no scopes assigned — not a reason to fall back to the unscoped query.
pub async fn counts_scoped(pool: &PgPool, program_ids: &[ProgramId]) -> Result<AdminStats> {
    let raw_ids: Vec<uuid::Uuid> = program_ids.iter().map(|p| p.into_uuid()).collect();

    sqlx::query_as!(
        AdminStats,
        r#"
        SELECT
            (SELECT count(*) FROM programs WHERE id = ANY($1))          as "programs!",
            (SELECT count(*) FROM semesters WHERE program_id = ANY($1)) as "semesters!",
            (SELECT count(*) FROM courses WHERE program_id = ANY($1))   as "courses!",
            (SELECT count(*) FROM blocks b
                JOIN courses c ON c.id = b.course_id
                WHERE c.program_id = ANY($1))                           as "blocks!",
            (SELECT count(*) FROM lscs)                                 as "lscs!",
            (SELECT count(*) FROM students WHERE program_id = ANY($1))  as "students!",
            (SELECT count(*) FROM documents d
                JOIN blocks b  ON b.id = d.block_id
                JOIN courses c ON c.id = b.course_id
                WHERE c.program_id = ANY($1))                           as "documents_total!",
            (SELECT count(*) FROM documents d
                JOIN blocks b  ON b.id = d.block_id
                JOIN courses c ON c.id = b.course_id
                WHERE c.program_id = ANY($1) AND d.status = 'pending_review')
                as "documents_pending_review!",
            (SELECT count(*) FROM documents d
                JOIN blocks b  ON b.id = d.block_id
                JOIN courses c ON c.id = b.course_id
                WHERE c.program_id = ANY($1) AND d.status = 'embedded')
                as "documents_embedded!",
            (SELECT count(*) FROM documents d
                JOIN blocks b  ON b.id = d.block_id
                JOIN courses c ON c.id = b.course_id
                WHERE c.program_id = ANY($1) AND d.status = 'failed')
                as "documents_failed!"
        "#,
        &raw_ids
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}
