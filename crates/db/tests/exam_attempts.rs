//! Integration tests for the exam-attempt reads behind the student dashboard
//! and the admin attempts list (`dg_db::models::exams`).
//!
//! `sqlx::test` per `.claude/rules/testing.md`: a real, ephemeral Postgres
//! with the repo's migrations applied, no mocking of the database. The
//! property that matters most here — a student can never read another
//! student's attempts — is enforced by a `WHERE` predicate, so only a real
//! server can prove it.

use chrono::{Duration, Utc};
use dg_core::{
    BlockId, ExamAttemptId, ExamId, ExamStatus, LscId, ProgramId, Role, SemesterId, StudentId,
    UserId,
};
use dg_db::models::{blocks, courses, exams, lscs, programs, semesters, students, users};
use sqlx::PgPool;

/// One program/semester/course/block, one exam on that block, and two
/// students — Ann with two attempts, Bob with one. Two students is the
/// minimum that can demonstrate isolation at all.
struct World {
    block_id: BlockId,
    exam_id: ExamId,
    ann: StudentId,
    bob: StudentId,
    ann_latest: ExamAttemptId,
    bob_attempt: ExamAttemptId,
}

#[allow(clippy::too_many_arguments)]
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

    students::create(pool, user.id, name, roll, "9000000000", program, semester, lsc)
        .await
        .expect("student inserts")
        .id
}

/// Insert one attempt directly: nothing writes `exam_attempts` yet (the
/// classroom/assessment path is a later phase), and these tests are about the
/// read side, so the fixture owns the INSERT.
#[allow(clippy::too_many_arguments)]
async fn attempt(
    pool: &PgPool,
    exam_id: ExamId,
    student_id: StudentId,
    attempt_no: i16,
    score: Option<f64>,
    submitted_minutes_ago: i64,
) -> ExamAttemptId {
    let submitted = Utc::now() - Duration::minutes(submitted_minutes_ago);
    let status = if score.is_some() { "graded" } else { "submitted" };

    let rec = sqlx::query!(
        r#"
        INSERT INTO exam_attempts
            (exam_id, student_id, attempt_no, score, max_score, status, started_at, submitted_at)
        VALUES ($1, $2, $3, $4, 20, $5::text::exam_attempt_status,
                $6::timestamptz - interval '30 minutes', $6)
        RETURNING id as "id: ExamAttemptId"
        "#,
        exam_id.into_uuid(),
        student_id.into_uuid(),
        attempt_no,
        score,
        status,
        submitted
    )
    .fetch_one(pool)
    .await
    .expect("attempt inserts");

    rec.id
}

async fn seed(pool: &PgPool) -> World {
    let program = programs::create(pool, "TSTP", "Test Program", None)
        .await
        .expect("program inserts");
    let semester = semesters::create(pool, program.id, 1, "Semester 1")
        .await
        .expect("semester inserts");
    let course = courses::create(pool, program.id, semester.id, "C100", "Course", None)
        .await
        .expect("course inserts");
    let block = blocks::create(pool, course.id, 1, "Unit 1", None)
        .await
        .expect("block inserts");
    let lsc = lscs::create(pool, "LSC1", "Centre One", None)
        .await
        .expect("lsc inserts");

    let admin = users::create(pool, Role::SuperAdmin, "admin@test.local", "hash")
        .await
        .expect("admin inserts");

    let exam = exams::create(
        pool,
        block.id,
        "Unit 1 Test",
        Some("End of block"),
        20.0,
        Some(45),
        ExamStatus::Published,
        UserId::from(admin.id.into_uuid()),
    )
    .await
    .expect("exam inserts");

    let ann = student(
        pool, "ann@test.local", "Ann", "TSTPA0001", program.id, semester.id, lsc.id,
    )
    .await;
    let bob = student(
        pool, "bob@test.local", "Bob", "TSTPA0002", program.id, semester.id, lsc.id,
    )
    .await;

    // Ann's first sitting is older than her second, so "most recent" has a
    // right answer that insertion order alone would not give.
    attempt(pool, exam.id, ann, 1, Some(11.0), 120).await;
    let ann_latest = attempt(pool, exam.id, ann, 2, Some(18.0), 10).await;
    let bob_attempt = attempt(pool, exam.id, bob, 1, Some(9.0), 60).await;

    World {
        block_id: block.id,
        exam_id: exam.id,
        ann,
        bob,
        ann_latest,
        bob_attempt,
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_student_never_sees_another_students_attempts(pool: PgPool) {
    let w = seed(&pool).await;

    let rows = exams::attempts_for_student(&pool, w.ann, 50, 0)
        .await
        .expect("listing succeeds");

    assert_eq!(rows.len(), 2, "Ann's two attempts, and only hers");
    assert!(
        rows.iter().all(|r| r.id != w.bob_attempt),
        "Bob's attempt must never appear in Ann's list"
    );
    assert_eq!(
        exams::count_attempts_for_student(&pool, w.ann)
            .await
            .expect("count"),
        2,
        "X-Total-Count is scoped to the caller too, not just the page"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn fetching_another_students_attempt_by_id_returns_nothing(pool: PgPool) {
    let w = seed(&pool).await;

    // Ann asks for Bob's attempt id directly — the id is real, the row is not
    // hers, and the query must answer as if it did not exist.
    let stolen = exams::attempt_for_student(&pool, w.ann, w.bob_attempt)
        .await
        .expect("query succeeds");
    assert!(stolen.is_none(), "another student's attempt is not readable");

    // The same id is readable by its actual owner, proving the id itself is
    // valid and the `None` above is an ownership result, not a bad fixture.
    let own = exams::attempt_for_student(&pool, w.bob, w.bob_attempt)
        .await
        .expect("query succeeds");
    assert!(own.is_some());
}

#[sqlx::test(migrations = "../../migrations")]
async fn recent_attempts_are_ordered_newest_first(pool: PgPool) {
    let w = seed(&pool).await;

    let rows = exams::attempts_for_student(&pool, w.ann, 50, 0)
        .await
        .expect("listing succeeds");

    assert_eq!(
        rows[0].id, w.ann_latest,
        "the dashboard's leading card is the most recent attempt"
    );
    assert_eq!(rows[0].attempt_no, 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_card_carries_the_labels_it_renders(pool: PgPool) {
    let w = seed(&pool).await;

    let rows = exams::attempts_for_student(&pool, w.ann, 50, 0)
        .await
        .expect("listing succeeds");
    let card = &rows[0];

    assert_eq!(card.exam_title, "Unit 1 Test");
    assert_eq!(card.block_title, "Unit 1");
    assert_eq!(card.course_code, "C100");
    assert_eq!(card.score, Some(18.0));
    assert_eq!(card.max_score, 20.0);
    assert!(card.submitted_at.is_some());
}

#[sqlx::test(migrations = "../../migrations")]
async fn the_admin_attempts_list_spans_every_student_of_one_exam(pool: PgPool) {
    let w = seed(&pool).await;

    let rows = exams::attempts_for_exam(&pool, w.exam_id, 50, 0)
        .await
        .expect("listing succeeds");

    assert_eq!(rows.len(), 3, "Ann's two plus Bob's one");
    assert!(rows.iter().any(|r| r.student_id == w.bob));
    assert_eq!(
        exams::count_attempts_for_exam(&pool, w.exam_id)
            .await
            .expect("count"),
        3
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_blocks_exams_list_and_its_total_agree(pool: PgPool) {
    let w = seed(&pool).await;

    let rows = exams::list_by_block(&pool, w.block_id, None, 50, 0)
        .await
        .expect("listing succeeds");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, w.exam_id);
    // Ancestry is denormalised onto the row, so a console breadcrumb needs no
    // second request.
    assert_eq!(rows[0].course_code, "C100");
    assert_eq!(rows[0].block_no, 1);

    assert_eq!(
        exams::count_by_block(&pool, w.block_id, None)
            .await
            .expect("count"),
        1
    );
    assert_eq!(
        exams::count_by_block(&pool, w.block_id, Some("nothing-matches"))
            .await
            .expect("count"),
        0,
        "X-Total-Count applies the same q filter as the page"
    );
}
