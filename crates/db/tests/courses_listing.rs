//! Integration tests for the flattened program-level course listing
//! (`courses::list_by_program`, behind `GET /api/v1/admin/programs/{id}/courses`).
//!
//! `sqlx::test` per `.claude/rules/testing.md`: a real, ephemeral Postgres
//! with the repo's migrations applied, no mocking of the database. Ordering
//! and cross-semester coverage are properties of the SQL itself, so they can
//! only be asserted against a real server.

use dg_core::{ProgramId, SemesterId};
use dg_db::models::{courses, programs, semesters};
use sqlx::PgPool;

/// Two semesters of one program, each with courses inserted out of order, so
/// neither insertion order nor per-semester order can accidentally satisfy
/// the assertions.
async fn seed(pool: &PgPool) -> (ProgramId, SemesterId, SemesterId) {
    let program = programs::create(pool, "TSTP", "Test Program", None)
        .await
        .expect("program inserts");

    let sem2 = semesters::create(pool, program.id, 2, "Semester 2")
        .await
        .expect("semester 2 inserts");
    let sem1 = semesters::create(pool, program.id, 1, "Semester 1")
        .await
        .expect("semester 1 inserts");

    for (semester, code, name) in [
        (sem2.id, "C201", "Second Year B"),
        (sem2.id, "C200", "Second Year A"),
        (sem1.id, "C101", "First Year B"),
        (sem1.id, "C100", "First Year A"),
    ] {
        courses::create(pool, program.id, semester, code, name, None)
            .await
            .expect("course inserts");
    }

    (program.id, sem1.id, sem2.id)
}

#[sqlx::test(migrations = "../../migrations")]
async fn lists_courses_from_every_semester_of_the_program(pool: PgPool) {
    let (program_id, sem1, sem2) = seed(&pool).await;

    let rows = courses::list_by_program(&pool, program_id, None, 200, 0)
        .await
        .expect("listing succeeds");

    assert_eq!(rows.len(), 4, "every semester's courses, not just one's");
    assert!(rows.iter().any(|c| c.semester_id == sem1));
    assert!(rows.iter().any(|c| c.semester_id == sem2));
}

#[sqlx::test(migrations = "../../migrations")]
async fn orders_by_semester_number_then_course_code(pool: PgPool) {
    let (program_id, _, _) = seed(&pool).await;

    let rows = courses::list_by_program(&pool, program_id, None, 200, 0)
        .await
        .expect("listing succeeds");

    let order: Vec<&str> = rows.iter().map(|c| c.code.as_str()).collect();
    assert_eq!(order, ["C100", "C101", "C200", "C201"]);

    let numbers: Vec<i16> = rows.iter().map(|c| c.semester_number).collect();
    assert_eq!(numbers, [1, 1, 2, 2], "semester_number is denormalised onto each row");
}

#[sqlx::test(migrations = "../../migrations")]
async fn search_filters_on_code_and_name_case_insensitively(pool: PgPool) {
    let (program_id, _, _) = seed(&pool).await;

    let by_code = courses::list_by_program(&pool, program_id, Some("c10"), 200, 0)
        .await
        .expect("listing succeeds");
    assert_eq!(by_code.len(), 2);

    let by_name = courses::list_by_program(&pool, program_id, Some("second year"), 200, 0)
        .await
        .expect("listing succeeds");
    assert_eq!(by_name.len(), 2);

    let total = courses::count_by_program(&pool, program_id, Some("second year"))
        .await
        .expect("count succeeds");
    assert_eq!(total, 2, "X-Total-Count matches the filtered listing");
}

#[sqlx::test(migrations = "../../migrations")]
async fn never_returns_another_programs_courses(pool: PgPool) {
    let (program_id, _, _) = seed(&pool).await;

    let other = programs::create(&pool, "OTHR", "Other Program", None)
        .await
        .expect("program inserts");
    let other_semester = semesters::create(&pool, other.id, 1, "Semester 1")
        .await
        .expect("semester inserts");
    courses::create(&pool, other.id, other_semester.id, "X100", "Elsewhere", None)
        .await
        .expect("course inserts");

    let rows = courses::list_by_program(&pool, program_id, None, 200, 0)
        .await
        .expect("listing succeeds");

    assert!(rows.iter().all(|c| c.program_id == program_id));
    assert_eq!(courses::count_by_program(&pool, program_id, None).await.expect("count"), 4);
}
