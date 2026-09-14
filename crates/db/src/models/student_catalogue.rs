//! What a student may actually study, in the order they navigate it:
//! **course -> block -> unit**.
//!
//! The admin console already lists each of these, but never from the student's
//! side. The classroom used to open straight onto `students.current_block_id`,
//! so a student could study exactly one block and had no way to see, let alone
//! choose, anything else in their own syllabus.
//!
//! # Every query starts from `student_courses`
//!
//! Not "filters by" — *starts from*. `student_courses` is the enforcement point
//! for content access (`CLAUDE.md`), so each query below joins through it with
//! the caller's own `student_id` bound as the leading predicate. A course the
//! student is not enrolled in, or a block or document under one, selects no
//! row: there is no result set to filter and therefore nothing for a later
//! `WHERE` to accidentally widen. An id belonging to someone else's course
//! comes back as "not found", which is what the handlers turn into `404`.
//!
//! Only `active` enrolments count. A `dropped` or `completed` enrolment is a
//! historical record, not a licence to keep opening the classroom.
//!
//! # Why documents are called units
//!
//! `documents` is the storage-side name — a row with a `sha256`, a
//! `storage_key` and an ingestion status. To the student it is the unit of the
//! syllabus they were told to read ("Unit 1"), which is exactly what the titles
//! say. These queries return the student's word for it; nothing is renamed in
//! the schema.

use dg_core::{BlockId, CourseId, DocumentId, StudentId};
use sqlx::PgPool;

use crate::error::{Error, Result};

/// One enrolled course, with enough counted context to render a card.
#[derive(Debug, Clone)]
pub struct StudentCourseRow {
    pub course_id: CourseId,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub semester_number: i16,
    pub semester_name: String,
    /// Active blocks in the course.
    pub block_count: i64,
    /// Blocks holding at least one embedded unit — i.e. blocks the tutor can
    /// actually teach from. A course where this is `0` is listed anyway: the
    /// student is enrolled in it, and hiding it would look like a missing
    /// course rather than a course whose material has not been uploaded yet.
    pub teachable_block_count: i64,
}

pub async fn courses_for_student(
    pool: &PgPool,
    student_id: StudentId,
) -> Result<Vec<StudentCourseRow>> {
    sqlx::query_as!(
        StudentCourseRow,
        r#"
        SELECT
            c.id                    as "course_id!: CourseId",
            c.code                  as "code!",
            c.name                  as "name!",
            c.description           as "description",
            s.semester_number       as "semester_number!",
            s.name                  as "semester_name!",
            (SELECT count(*) FROM blocks b
              WHERE b.course_id = c.id AND b.status = 'active')
                                    as "block_count!",
            (SELECT count(DISTINCT b.id) FROM blocks b
              JOIN documents d ON d.block_id = b.id AND d.status = 'embedded'
              WHERE b.course_id = c.id AND b.status = 'active')
                                    as "teachable_block_count!"
        FROM student_courses sc
        JOIN courses   c ON c.id = sc.course_id
        JOIN semesters s ON s.id = c.semester_id
        WHERE sc.student_id = $1 AND sc.status = 'active'
        ORDER BY s.semester_number, c.code
        "#,
        student_id.into_uuid()
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// One block of an enrolled course.
#[derive(Debug, Clone)]
pub struct StudentBlockRow {
    pub block_id: BlockId,
    pub block_no: i16,
    pub title: String,
    pub description: Option<String>,
    /// Units uploaded to the block, whatever their ingestion state.
    pub unit_count: i64,
    /// Units that are embedded, and so have vectors the tutor can retrieve.
    /// `0` means opening the classroom here would abstain on everything — the
    /// UI says so rather than letting the student discover it mid-lesson.
    pub ready_unit_count: i64,
}

pub async fn blocks_for_student(
    pool: &PgPool,
    student_id: StudentId,
    course_id: CourseId,
) -> Result<Vec<StudentBlockRow>> {
    sqlx::query_as!(
        StudentBlockRow,
        r#"
        SELECT
            b.id          as "block_id!: BlockId",
            b.block_no    as "block_no!",
            b.title       as "title!",
            b.description as "description",
            (SELECT count(*) FROM documents d WHERE d.block_id = b.id)
                          as "unit_count!",
            (SELECT count(*) FROM documents d
              WHERE d.block_id = b.id AND d.status = 'embedded')
                          as "ready_unit_count!"
        FROM student_courses sc
        JOIN blocks b ON b.course_id = sc.course_id
        WHERE sc.student_id = $1
          AND sc.course_id  = $2
          AND sc.status = 'active'
          AND b.status  = 'active'
        ORDER BY b.block_no
        "#,
        student_id.into_uuid(),
        course_id.into_uuid()
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// One uploaded unit of a block.
///
/// Deliberately carries no `storage_key` and no `sha256`: the student reads the
/// unit through the tutor, and neither the server-side path nor the content
/// hash is theirs to have (`api-conventions.md` keeps `storage_key` off the
/// wire even for admins).
#[derive(Debug, Clone)]
pub struct StudentUnitRow {
    pub document_id: DocumentId,
    pub title: String,
    pub page_count: i32,
    /// `true` only for an embedded document. The tutor retrieves from vectors,
    /// so a unit in any other state has nothing to teach from yet.
    pub is_ready: bool,
    /// Where the upload has got to: `pending`, `parsing`, `pending_review`,
    /// `embedded` or `failed`.
    ///
    /// Exposed to the student, not just to admins, because "still being
    /// prepared" on its own is indistinguishable from "broken" — and a unit can
    /// sit there for minutes while a long PDF is parsed and embedded. The stage
    /// is what lets the UI show progress that is actually moving.
    pub status: String,
    /// Whether a rendered cover exists for this unit.
    ///
    /// A boolean rather than the path: `thumbnail_key` is a server-side
    /// filesystem path and stays off the wire for exactly the reason
    /// `storage_key` does. The client uses this only to decide between
    /// requesting the cover and drawing its own placeholder tile — asking for
    /// an image that is not there would mean a 404 per unit on every visit.
    pub has_thumbnail: bool,
}

pub async fn units_for_student(
    pool: &PgPool,
    student_id: StudentId,
    block_id: BlockId,
) -> Result<Vec<StudentUnitRow>> {
    sqlx::query_as!(
        StudentUnitRow,
        r#"
        SELECT
            d.id                             as "document_id!: DocumentId",
            d.title                          as "title!",
            d.page_count                     as "page_count!",
            (d.status = 'embedded')          as "is_ready!",
            d.status                         as "status!",
            (d.thumbnail_key IS NOT NULL)    as "has_thumbnail!"
        FROM student_courses sc
        JOIN blocks    b ON b.course_id = sc.course_id
        JOIN documents d ON d.block_id  = b.id
        WHERE sc.student_id = $1
          AND b.id = $2
          AND sc.status = 'active'
          AND b.status  = 'active'
        ORDER BY d.title, d.created_at
        "#,
        student_id.into_uuid(),
        block_id.into_uuid()
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// The path to a unit's cover image, if the caller is entitled to see it.
///
/// Starts from `student_courses` with the caller's own id bound, exactly as
/// [`units_for_student`] does — not "filters by". A document outside the
/// student's active enrolments selects no row, so the route serving covers
/// cannot be turned into an oracle for documents in another programme: a
/// `document_id` belonging to somebody else's syllabus is indistinguishable
/// from one that does not exist, and both answer `404`.
///
/// `None` therefore covers three cases that must not be told apart from
/// outside: not enrolled, no such document, and no cover rendered for it.
pub async fn thumbnail_key_for_student(
    pool: &PgPool,
    student_id: StudentId,
    document_id: DocumentId,
) -> Result<Option<String>> {
    sqlx::query_scalar!(
        r#"
        SELECT d.thumbnail_key
        FROM student_courses sc
        JOIN blocks    b ON b.course_id = sc.course_id
        JOIN documents d ON d.block_id  = b.id
        WHERE sc.student_id = $1
          AND d.id = $2
          AND sc.status = 'active'
          AND b.status  = 'active'
        "#,
        student_id.into_uuid(),
        document_id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
    // `fetch_optional` gives "no such row"; the column is itself nullable.
    // Both mean "no cover you may have", so they flatten to one `None`.
    .map(Option::flatten)
}

/// Whether the student may open the classroom on this block.
///
/// The same join the listings use, reduced to a yes/no. It exists so the
/// WebSocket path can authorise `session_init` with one round trip instead of
/// re-deriving enrolment from a list, and so that check reads identically to
/// the ones above — the classroom is the one place where getting this wrong
/// means teaching a student another programme's material.
pub async fn may_study_block(
    pool: &PgPool,
    student_id: StudentId,
    block_id: BlockId,
) -> Result<bool> {
    let found = sqlx::query_scalar!(
        r#"
        SELECT 1 as "one!"
        FROM student_courses sc
        JOIN blocks b ON b.course_id = sc.course_id
        WHERE sc.student_id = $1
          AND b.id = $2
          AND sc.status = 'active'
          AND b.status  = 'active'
        "#,
        student_id.into_uuid(),
        block_id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(found.is_some())
}
