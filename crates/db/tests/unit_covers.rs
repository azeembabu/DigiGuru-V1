//! Integration tests for unit cover images
//! (`student_catalogue::thumbnail_key_for_student`, behind
//! `GET /api/v1/student/units/{document_id}/thumbnail`).
//!
//! The route hands back bytes rather than JSON, which makes it the one place
//! in the student catalogue where a scope mistake leaks a *file* rather than a
//! row. The property that stops that is in the SQL — the query starts from
//! `student_courses` with the caller's id bound — so it can only be asserted
//! against a real database (`.claude/rules/testing.md`: no mocking of it).
//!
//! Ann and Bob are on two different programmes here, not two courses of one,
//! so a missing enrolment join could not be masked by them sharing a block.

use dg_core::{
    CourseId, DocumentId, EnrollmentStatus, LscId, ProgramId, Role, SemesterId, StudentId, UserId,
};
use dg_db::models::{
    blocks, courses, documents, lscs, programs, semesters, student_catalogue, student_courses,
    students, uploads, users,
};
use sqlx::PgPool;

struct World {
    /// Ann is enrolled in the course this document belongs to.
    ann: StudentId,
    /// Bob is enrolled in a different programme entirely.
    bob: StudentId,
    /// A unit of Ann's course, with a cover recorded.
    ann_unit: DocumentId,
    /// A unit of Ann's course with no cover rendered.
    ann_unit_without_cover: DocumentId,
    ann_course: CourseId,
}

async fn student(
    pool: &PgPool,
    email: &str,
    name: &str,
    roll: &str,
    program: ProgramId,
    semester: SemesterId,
    lsc: LscId,
) -> StudentId {
    let user = users::create(pool, Role::Student, email, "argon2id$placeholder")
        .await
        .expect("user inserts");

    students::create(
        pool,
        user.id,
        name,
        roll,
        "9000000000",
        program,
        semester,
        lsc,
    )
    .await
    .expect("student inserts")
    .id
}

/// An admin to own the uploads. `documents.uploaded_by` is NOT NULL, and the
/// uploader is irrelevant to who may *see* a cover — which is the point.
async fn uploader(pool: &PgPool) -> UserId {
    users::create(pool, Role::SuperAdmin, "admin@test.local", "argon2id$placeholder")
        .await
        .expect("admin inserts")
        .id
}

async fn seed(pool: &PgPool) -> World {
    let lsc = lscs::create(pool, "LSC1", "Centre One", None)
        .await
        .expect("lsc inserts");
    let admin = uploader(pool).await;

    // Ann's programme, with a block holding two units.
    let p1 = programs::create(pool, "P1", "Program One", None)
        .await
        .expect("program inserts");
    let s1 = semesters::create(pool, p1.id, 1, "Semester 1")
        .await
        .expect("semester inserts");
    let c1 = courses::create(pool, p1.id, s1.id, "C100", "Course One", None)
        .await
        .expect("course inserts");
    let b1 = blocks::create(pool, c1.id, 1, "Block One", None)
        .await
        .expect("block inserts");

    let with_cover = uploads::insert_document(
        pool,
        b1.id,
        admin,
        "Unit 1 Environmental Segments",
        "./uploads/aaa.pdf",
        "aaa",
        13,
    )
    .await
    .expect("document inserts");
    documents::set_thumbnail_key(pool, with_cover, "./uploads/aaa.webp")
        .await
        .expect("cover records");

    let without_cover = uploads::insert_document(
        pool,
        b1.id,
        admin,
        "Unit 2 Natural Resources",
        "./uploads/bbb.pdf",
        "bbb",
        5,
    )
    .await
    .expect("document inserts");

    // Bob's programme — separate the whole way up.
    let p2 = programs::create(pool, "P2", "Program Two", None)
        .await
        .expect("program inserts");
    let s2 = semesters::create(pool, p2.id, 1, "Semester 1")
        .await
        .expect("semester inserts");
    let c2 = courses::create(pool, p2.id, s2.id, "C200", "Course Two", None)
        .await
        .expect("course inserts");

    let ann = student(pool, "ann@test.local", "Ann", "P1A0001", p1.id, s1.id, lsc.id).await;
    let bob = student(pool, "bob@test.local", "Bob", "P2A0001", p2.id, s2.id, lsc.id).await;

    student_courses::assign(pool, ann, c1.id)
        .await
        .expect("ann enrols");
    student_courses::assign(pool, bob, c2.id)
        .await
        .expect("bob enrols");

    World {
        ann,
        bob,
        ann_unit: with_cover,
        ann_unit_without_cover: without_cover,
        ann_course: c1.id,
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn an_enrolled_student_gets_the_path_to_their_units_cover(pool: PgPool) {
    let world = seed(&pool).await;

    let key = student_catalogue::thumbnail_key_for_student(&pool, world.ann, world.ann_unit)
        .await
        .expect("the query runs");

    assert_eq!(key.as_deref(), Some("./uploads/aaa.webp"));
}

/// The property the route's `404` rests on. Bob is a real student holding a
/// real document id — he is simply not enrolled in the course it belongs to,
/// and that alone must make the cover unreachable.
#[sqlx::test(migrations = "../../migrations")]
async fn another_students_cover_is_unreachable(pool: PgPool) {
    let world = seed(&pool).await;

    let key = student_catalogue::thumbnail_key_for_student(&pool, world.bob, world.ann_unit)
        .await
        .expect("the query runs");

    assert_eq!(
        key, None,
        "a document outside the caller's enrolments must select no row at all"
    );
}

/// A dropped enrolment is a historical record, not a licence to keep reading
/// the course's material — the same rule the unit list itself follows.
#[sqlx::test(migrations = "../../migrations")]
async fn a_dropped_enrolment_loses_access_to_the_cover(pool: PgPool) {
    let world = seed(&pool).await;

    student_courses::update_status(&pool, world.ann, world.ann_course, EnrollmentStatus::Dropped)
        .await
        .expect("enrolment updates");

    let key = student_catalogue::thumbnail_key_for_student(&pool, world.ann, world.ann_unit)
        .await
        .expect("the query runs");

    assert_eq!(key, None);
}

/// "No cover was rendered" and "not your document" deliberately flatten to the
/// same `None`, so the route cannot be used to tell the two apart.
#[sqlx::test(migrations = "../../migrations")]
async fn a_unit_with_no_rendered_cover_is_indistinguishable_from_one_you_may_not_have(
    pool: PgPool,
) {
    let world = seed(&pool).await;

    let mine_without_cover =
        student_catalogue::thumbnail_key_for_student(&pool, world.ann, world.ann_unit_without_cover)
            .await
            .expect("the query runs");
    let not_mine = student_catalogue::thumbnail_key_for_student(&pool, world.bob, world.ann_unit)
        .await
        .expect("the query runs");

    assert_eq!(mine_without_cover, None);
    assert_eq!(not_mine, None);
}

/// `has_thumbnail` on the unit list is what stops the client asking for covers
/// that do not exist, so it must track the column rather than the status.
#[sqlx::test(migrations = "../../migrations")]
async fn the_unit_list_reports_which_units_have_a_cover(pool: PgPool) {
    let world = seed(&pool).await;

    let block = student_catalogue::blocks_for_student(&pool, world.ann, world.ann_course)
        .await
        .expect("blocks list")
        .pop()
        .expect("one block");

    let units = student_catalogue::units_for_student(&pool, world.ann, block.block_id)
        .await
        .expect("units list");

    let with = units
        .iter()
        .find(|u| u.document_id == world.ann_unit)
        .expect("the covered unit is listed");
    let without = units
        .iter()
        .find(|u| u.document_id == world.ann_unit_without_cover)
        .expect("the uncovered unit is listed");

    assert!(with.has_thumbnail);
    assert!(!without.has_thumbnail);
    // Both are still `pending`: a cover says nothing about teachability, and
    // conflating the two would hide un-ingested units behind a missing image.
    assert!(!with.is_ready && !without.is_ready);
}
