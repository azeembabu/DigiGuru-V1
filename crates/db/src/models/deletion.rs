//! Permanent removal of academic content: programme, course, block, unit.
//!
//! # Read this before changing anything here
//!
//! The foreign-key graph makes these deletions far more destructive than they
//! look, and in two opposite ways at once.
//!
//! **Some references cascade silently.** `blocks -> exams -> exam_attempts ->
//! exam_attempt_answers` is `ON DELETE CASCADE` the whole way down, so deleting
//! one block destroys every student's marks for that block's exams without the
//! database saying a word. `question_pool` and `flashcards` go the same way.
//! That is why [`impact`] exists: the caller must be able to show, before
//! anything happens, exactly what is about to be lost.
//!
//! **Other references refuse.** `documents`, `learning_sessions`,
//! `board_events`, `safety_incidents` and `students.current_block_id` are
//! `NO ACTION`, so a plain `DELETE FROM blocks` simply errors. Unwinding them
//! in the right order is most of what the functions below do, and the order is
//! not arbitrary — a child that still points at a row blocks that row's
//! deletion, so the walk is strictly leaf-first.
//!
//! # What is never deleted
//!
//! * **Student accounts.** `students.program_id` is `ON DELETE RESTRICT` and
//!   `NOT NULL`, and that restriction is kept deliberately rather than worked
//!   around: removing a programme is a content operation, and deleting the
//!   people registered on it is a different decision that nobody asked this
//!   function to make. A programme with students registered on it reports
//!   [`DeleteBlocked::StudentsRegistered`] and nothing is touched.
//! * **Safety incidents.** NN-5 requires incidents to be persisted. Their
//!   `session_id` is nulled when the session goes, so the record survives the
//!   lesson it came from.
//!
//! # Qdrant
//!
//! Deleting a document here removes only the Postgres row. Its vectors live in
//! Qdrant and are removed by the caller, which is why every function returns
//! the `document_ids` it deleted. Doing it the other way round — vectors first
//! — would leave a corpus the tutor can still retrieve from if the transaction
//! rolls back, which is an NN-4 violation that no test would catch.

use dg_core::{BlockId, CourseId, DocumentId, ProgramId};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{Error, Result};

/// What removing something would destroy. Every field is a count of rows that
/// will be gone afterwards.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeletionImpact {
    pub semesters: i64,
    pub courses: i64,
    pub blocks: i64,
    /// Uploaded documents; their Qdrant vectors go with them.
    pub units: i64,
    pub exams: i64,
    /// Students' marked papers. The number that matters most on a confirmation
    /// dialogue, and the reason this struct exists.
    pub exam_attempts: i64,
    pub questions: i64,
    pub flashcards: i64,
    pub learning_sessions: i64,
    pub board_events: i64,
    pub enrollments: i64,
    /// Distinct students who lose an attempt, a session or an enrolment. Not a
    /// sum of the above — one student may lose several.
    pub students_affected: i64,
}

/// Why a removal was refused. Not an error case in the usual sense: it is a
/// decision the caller has to surface to a human.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteBlocked {
    /// Students are registered on this programme. Their accounts are not this
    /// operation's to delete — see the module docs.
    StudentsRegistered(i64),
}

/// A completed removal: what was destroyed, and which documents' vectors the
/// caller must now purge from Qdrant.
#[derive(Debug, Clone)]
pub struct Deleted {
    pub impact: DeletionImpact,
    pub document_ids: Vec<DocumentId>,
}

// ---------------------------------------------------------------- impact ----

/// What deleting this programme would destroy.
pub async fn program_impact(pool: &PgPool, program_id: ProgramId) -> Result<DeletionImpact> {
    let blocks = block_ids_of_program(pool, program_id).await?;
    let mut impact = impact_for_blocks(pool, &blocks).await?;

    // Semesters, courses and enrolments hang off the programme rather than off
    // its blocks, so an empty semester still counts as something being removed.
    let row = sqlx::query!(
        r#"
        SELECT
          (SELECT count(*) FROM semesters WHERE program_id = $1)            as "semesters!",
          (SELECT count(*) FROM courses   WHERE program_id = $1)            as "courses!",
          (SELECT count(*) FROM student_courses sc
             JOIN courses c ON c.id = sc.course_id
            WHERE c.program_id = $1)                                        as "enrollments!"
        "#,
        program_id.into_uuid()
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)?;

    impact.semesters = row.semesters;
    impact.courses = row.courses;
    impact.enrollments = row.enrollments;
    Ok(impact)
}

/// What deleting this course would destroy.
pub async fn course_impact(pool: &PgPool, course_id: CourseId) -> Result<DeletionImpact> {
    let blocks = block_ids_of_course(pool, course_id).await?;
    let mut impact = impact_for_blocks(pool, &blocks).await?;
    impact.courses = 1;
    impact.enrollments = sqlx::query_scalar!(
        r#"SELECT count(*) as "n!" FROM student_courses WHERE course_id = $1"#,
        course_id.into_uuid()
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(impact)
}

/// What deleting this block would destroy.
pub async fn block_impact(pool: &PgPool, block_id: BlockId) -> Result<DeletionImpact> {
    impact_for_blocks(pool, &[block_id.into_uuid()]).await
}

/// What deleting this unit would destroy.
///
/// A document has no student-owned dependants at all — `ingestion_jobs` is the
/// only thing pointing at it — so the only loss is the unit itself and the
/// vectors that go with it. That is why removing a unit is the one deletion
/// here that is genuinely small.
pub async fn unit_impact(_pool: &PgPool, _document_id: DocumentId) -> Result<DeletionImpact> {
    Ok(DeletionImpact { units: 1, ..DeletionImpact::default() })
}

/// Block ids under a programme.
async fn block_ids_of_program(pool: &PgPool, program_id: ProgramId) -> Result<Vec<Uuid>> {
    sqlx::query_scalar!(
        r#"SELECT b.id as "id!" FROM blocks b
           JOIN courses c ON c.id = b.course_id
           WHERE c.program_id = $1"#,
        program_id.into_uuid()
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Block ids under a course.
async fn block_ids_of_course(pool: &PgPool, course_id: CourseId) -> Result<Vec<Uuid>> {
    sqlx::query_scalar!(
        r#"SELECT id as "id!" FROM blocks WHERE course_id = $1"#,
        course_id.into_uuid()
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// The counts that depend only on a set of blocks. Shared by all three levels
/// so a programme, a course and a block cannot report the same fact
/// differently.
async fn impact_for_blocks(pool: &PgPool, blocks: &[Uuid]) -> Result<DeletionImpact> {
    let row = sqlx::query!(
        r#"
        SELECT
          (SELECT count(*) FROM documents        WHERE block_id = ANY($1))              as "units!",
          (SELECT count(*) FROM exams            WHERE block_id = ANY($1))              as "exams!",
          (SELECT count(*) FROM exam_attempts ea
             JOIN exams e ON e.id = ea.exam_id
            WHERE e.block_id = ANY($1))                                                 as "exam_attempts!",
          (SELECT count(*) FROM question_pool    WHERE block_id = ANY($1))              as "questions!",
          (SELECT count(*) FROM flashcards       WHERE block_id = ANY($1))              as "flashcards!",
          (SELECT count(*) FROM learning_sessions WHERE block_id = ANY($1))             as "learning_sessions!",
          (SELECT count(*) FROM board_events be
             JOIN learning_sessions ls ON ls.id = be.session_id
            WHERE ls.block_id = ANY($1))                                                as "board_events!",
          (SELECT count(DISTINCT sid) FROM (
              SELECT ea.student_id as sid FROM exam_attempts ea
                JOIN exams e ON e.id = ea.exam_id WHERE e.block_id = ANY($1)
              UNION
              SELECT ls.student_id FROM learning_sessions ls WHERE ls.block_id = ANY($1)
           ) s)                                                                         as "students_affected!"
        "#,
        blocks
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)?;

    Ok(DeletionImpact {
        blocks: blocks.len() as i64,
        units: row.units,
        exams: row.exams,
        exam_attempts: row.exam_attempts,
        questions: row.questions,
        flashcards: row.flashcards,
        learning_sessions: row.learning_sessions,
        board_events: row.board_events,
        students_affected: row.students_affected,
        ..DeletionImpact::default()
    })
}

// ---------------------------------------------------------------- delete ----

/// Removes one unit (document) and reports its id for Qdrant cleanup.
///
/// `ingestion_jobs` cascades; nothing else points at a document.
pub async fn delete_unit(pool: &PgPool, document_id: DocumentId) -> Result<Option<Deleted>> {
    let deleted = sqlx::query_scalar!(
        r#"DELETE FROM documents WHERE id = $1 RETURNING id as "id!""#,
        document_id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)?;

    Ok(deleted.map(|id| Deleted {
        impact: DeletionImpact { units: 1, ..DeletionImpact::default() },
        document_ids: vec![DocumentId::from_uuid(id)],
    }))
}

/// Removes one block and everything under it.
pub async fn delete_block(pool: &PgPool, block_id: BlockId) -> Result<Option<Deleted>> {
    let impact = block_impact(pool, block_id).await?;
    let mut tx = pool.begin().await.map_err(Error::from_sqlx)?;
    let documents = unwind_blocks(&mut tx, &[block_id.into_uuid()]).await?;

    let deleted = sqlx::query_scalar!(
        r#"DELETE FROM blocks WHERE id = $1 RETURNING id as "id!""#,
        block_id.into_uuid()
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(Error::from_sqlx)?;

    if deleted.is_none() {
        // Nothing was removed, so nothing should be: roll back rather than
        // leaving a block's sessions and units deleted around a block that is
        // still there.
        tx.rollback().await.map_err(Error::from_sqlx)?;
        return Ok(None);
    }

    tx.commit().await.map_err(Error::from_sqlx)?;
    Ok(Some(Deleted { impact, document_ids: documents }))
}

/// Removes one course, its blocks, and everything under them.
pub async fn delete_course(pool: &PgPool, course_id: CourseId) -> Result<Option<Deleted>> {
    let impact = course_impact(pool, course_id).await?;
    let blocks = block_ids_of_course(pool, course_id).await?;

    let mut tx = pool.begin().await.map_err(Error::from_sqlx)?;
    let documents = unwind_blocks(&mut tx, &blocks).await?;

    // `learning_sessions.course_id` is NO ACTION and a session can name a
    // course without naming one of its blocks, so course-level sessions are
    // unwound here as well as the block-level ones above.
    let course_sessions = sqlx::query_scalar!(
        r#"SELECT id as "id!" FROM learning_sessions WHERE course_id = $1"#,
        course_id.into_uuid()
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(Error::from_sqlx)?;
    unwind_sessions(&mut tx, course_sessions).await?;

    sqlx::query!(r#"DELETE FROM blocks WHERE course_id = $1"#, course_id.into_uuid())
        .execute(&mut *tx)
        .await
        .map_err(Error::from_sqlx)?;

    let deleted = sqlx::query_scalar!(
        r#"DELETE FROM courses WHERE id = $1 RETURNING id as "id!""#,
        course_id.into_uuid()
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(Error::from_sqlx)?;

    if deleted.is_none() {
        tx.rollback().await.map_err(Error::from_sqlx)?;
        return Ok(None);
    }

    tx.commit().await.map_err(Error::from_sqlx)?;
    Ok(Some(Deleted { impact, document_ids: documents }))
}

/// Removes one programme and its whole catalogue.
///
/// Refuses while any student is registered on it: `students.program_id` is
/// `NOT NULL` and `ON DELETE RESTRICT`, and deleting those accounts is not this
/// operation's decision to make (module docs).
pub async fn delete_program(
    pool: &PgPool,
    program_id: ProgramId,
) -> Result<std::result::Result<Option<Deleted>, DeleteBlocked>> {
    let students = sqlx::query_scalar!(
        r#"SELECT count(*) as "n!" FROM students WHERE program_id = $1"#,
        program_id.into_uuid()
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)?;

    if students > 0 {
        return Ok(Err(DeleteBlocked::StudentsRegistered(students)));
    }

    let impact = program_impact(pool, program_id).await?;
    let blocks = block_ids_of_program(pool, program_id).await?;

    let mut tx = pool.begin().await.map_err(Error::from_sqlx)?;
    let documents = unwind_blocks(&mut tx, &blocks).await?;

    let program_sessions = sqlx::query_scalar!(
        r#"SELECT ls.id as "id!" FROM learning_sessions ls
           JOIN courses c ON c.id = ls.course_id
           WHERE c.program_id = $1"#,
        program_id.into_uuid()
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(Error::from_sqlx)?;
    unwind_sessions(&mut tx, program_sessions).await?;

    // `sub_admin_scopes.program_id` is NO ACTION: a scope naming a programme
    // that no longer exists would leave that sub-admin scoped to nothing, in a
    // way no screen explains.
    sqlx::query!(
        r#"DELETE FROM sub_admin_scopes WHERE program_id = $1"#,
        program_id.into_uuid()
    )
    .execute(&mut *tx)
    .await
    .map_err(Error::from_sqlx)?;

    sqlx::query!(
        r#"DELETE FROM blocks b USING courses c
           WHERE b.course_id = c.id AND c.program_id = $1"#,
        program_id.into_uuid()
    )
    .execute(&mut *tx)
    .await
    .map_err(Error::from_sqlx)?;

    let deleted = sqlx::query_scalar!(
        r#"DELETE FROM programs WHERE id = $1 RETURNING id as "id!""#,
        program_id.into_uuid()
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(Error::from_sqlx)?;

    if deleted.is_none() {
        tx.rollback().await.map_err(Error::from_sqlx)?;
        return Ok(Ok(None));
    }

    tx.commit().await.map_err(Error::from_sqlx)?;
    Ok(Ok(Some(Deleted { impact, document_ids: documents })))
}

// ------------------------------------------------------------- unwinding ----

/// Clears everything that would otherwise refuse to let these blocks go, and
/// returns the documents removed so the caller can purge their vectors.
///
/// Strictly leaf-first. Reordering these statements breaks the deletion with a
/// foreign-key error rather than a wrong result, which is the failure mode to
/// prefer — but it is still a failure, so do not reorder them.
async fn unwind_blocks(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    blocks: &[Uuid],
) -> Result<Vec<DocumentId>> {
    if blocks.is_empty() {
        return Ok(Vec::new());
    }

    let sessions = sqlx::query_scalar!(
        r#"SELECT id as "id!" FROM learning_sessions WHERE block_id = ANY($1)"#,
        blocks
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(Error::from_sqlx)?;
    unwind_sessions(tx, sessions).await?;

    // A student parked on a block that is going away. Nulled rather than
    // repointed: choosing another block for them would be inventing a
    // curriculum decision.
    sqlx::query!(
        r#"UPDATE students SET current_block_id = NULL, updated_at = now()
           WHERE current_block_id = ANY($1)"#,
        blocks
    )
    .execute(&mut **tx)
    .await
    .map_err(Error::from_sqlx)?;

    let documents = sqlx::query_scalar!(
        r#"DELETE FROM documents WHERE block_id = ANY($1) RETURNING id as "id!""#,
        blocks
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(Error::from_sqlx)?;

    Ok(documents.into_iter().map(DocumentId::from_uuid).collect())
}

/// Clears what points at these sessions, then the sessions themselves.
async fn unwind_sessions(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    sessions: Vec<Uuid>,
) -> Result<()> {
    if sessions.is_empty() {
        return Ok(());
    }

    // Note reminders first. `note_reminders.session_id` cascades from the
    // session, but `event_from_id`/`event_to_id` are NO ACTION against
    // `board_events` — so deleting the board events first fails with a foreign
    // key error the moment a student has saved a note from that lesson. Found
    // by running a real cleanup rather than by reading the schema: a block
    // whose sessions happened to have no saved notes deleted cleanly, which is
    // exactly how this stays hidden.
    sqlx::query!(r#"DELETE FROM note_reminders WHERE session_id = ANY($1)"#, &sessions)
        .execute(&mut **tx)
        .await
        .map_err(Error::from_sqlx)?;

    sqlx::query!(r#"DELETE FROM board_events WHERE session_id = ANY($1)"#, &sessions)
        .execute(&mut **tx)
        .await
        .map_err(Error::from_sqlx)?;

    // NN-5 requires incidents to persist. The lesson goes; the record of what
    // happened in it does not.
    sqlx::query!(
        r#"UPDATE safety_incidents SET session_id = NULL WHERE session_id = ANY($1)"#,
        &sessions
    )
    .execute(&mut **tx)
    .await
    .map_err(Error::from_sqlx)?;

    sqlx::query!(r#"DELETE FROM learning_sessions WHERE id = ANY($1)"#, &sessions)
        .execute(&mut **tx)
        .await
        .map_err(Error::from_sqlx)?;

    Ok(())
}

/// How many students are registered on a programme.
///
/// Split out so the impact preview can explain the refusal without attempting
/// the delete: `delete_program` answers the same question, but only as part of
/// refusing, and a preview must not have to provoke a failure to describe one.
pub async fn program_student_count(pool: &PgPool, program_id: ProgramId) -> Result<i64> {
    sqlx::query_scalar!(
        r#"SELECT count(*) as "n!" FROM students WHERE program_id = $1"#,
        program_id.into_uuid()
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}
