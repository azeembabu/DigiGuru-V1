//! Integration tests for the flashcard notebook (`dg_db::models::flashcards`).
//!
//! Two properties need a real database: a card is unreachable with another
//! student's identity, and "due" is decided by the date the caller passes in —
//! the student's own local today — rather than by the server's `current_date`.
//! The second is the NN-3 rule applied to the notebook: a student east of the
//! server must not see tomorrow's cards, and one west of it must not lose
//! today's.

use chrono::NaiveDate;
use dg_core::{BlockId, LscId, ProgramId, Role, SemesterId, StudentId};
use dg_db::models::{blocks, courses, flashcards, lscs, programs, semesters, students, users};
use sqlx::PgPool;

struct World {
    block: BlockId,
    ann: StudentId,
    bob: StudentId,
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

async fn seed(pool: &PgPool) -> World {
    let program = programs::create(pool, "P1", "Program One", None)
        .await
        .expect("program inserts");
    let semester = semesters::create(pool, program.id, 1, "Semester 1")
        .await
        .expect("semester inserts");
    let course = courses::create(pool, program.id, semester.id, "C100", "Course One", None)
        .await
        .expect("course inserts");
    let block = blocks::create(pool, course.id, 1, "Unit 1", None)
        .await
        .expect("block inserts");
    let lsc = lscs::create(pool, "LSC1", "Centre One", None)
        .await
        .expect("lsc inserts");

    let ann = student(
        pool,
        "ann@test.local",
        "Ann",
        "P1A0001",
        program.id,
        semester.id,
        lsc.id,
    )
    .await;
    let bob = student(
        pool,
        "bob@test.local",
        "Bob",
        "P1A0002",
        program.id,
        semester.id,
        lsc.id,
    )
    .await;

    World {
        block: block.id,
        ann,
        bob,
    }
}

fn day(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
}

#[sqlx::test(migrations = "../../migrations")]
async fn due_counts_follow_the_date_the_caller_supplies_not_the_servers(pool: PgPool) {
    let w = seed(&pool).await;

    // One card due on the 13th, one on the 14th. A student whose local date is
    // still the 13th must see exactly one due card even if the server's clock
    // has already rolled over.
    flashcards::create(
        &pool,
        w.ann,
        w.block,
        "front A",
        "back A",
        Some("prosody"),
        None,
        day(2026, 9, 13),
    )
    .await
    .expect("card inserts");
    flashcards::create(
        &pool,
        w.ann,
        w.block,
        "front B",
        "back B",
        None,
        None,
        day(2026, 9, 14),
    )
    .await
    .expect("card inserts");

    let on_13th = flashcards::counts_for_student(&pool, w.ann, day(2026, 9, 13))
        .await
        .expect("counts succeed");
    assert_eq!(on_13th.total, 2);
    assert_eq!(on_13th.due, 1, "tomorrow's card is not due today");

    let on_14th = flashcards::counts_for_student(&pool, w.ann, day(2026, 9, 14))
        .await
        .expect("counts succeed");
    assert_eq!(on_14th.due, 2);

    let due_only = flashcards::list_for_student(&pool, w.ann, Some(day(2026, 9, 13)), None, 50, 0)
        .await
        .expect("list succeeds");
    assert_eq!(due_only.len(), 1);
    assert_eq!(due_only[0].front, "front A");
    assert_eq!(
        flashcards::count_for_student(&pool, w.ann, Some(day(2026, 9, 13)), None)
            .await
            .expect("count succeeds"),
        1,
        "X-Total-Count applies the same due filter as the page"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_review_schedules_from_the_students_own_today(pool: PgPool) {
    let w = seed(&pool).await;

    let card = flashcards::create(
        &pool,
        w.ann,
        w.block,
        "front",
        "back",
        None,
        None,
        day(2026, 9, 13),
    )
    .await
    .expect("card inserts");
    assert_eq!(card.interval_days, 0);
    assert_eq!(card.ease, 250);

    let first = flashcards::review(&pool, w.ann, card.id, true, day(2026, 9, 13))
        .await
        .expect("review succeeds")
        .expect("the card is the student's own");
    assert_eq!(first.interval_days, 1);
    assert_eq!(first.due_on, day(2026, 9, 14), "due_on is today + interval");
    assert!(first.reviewed_at.is_some());

    let second = flashcards::review(&pool, w.ann, card.id, true, day(2026, 9, 14))
        .await
        .expect("review succeeds")
        .expect("card still exists");
    assert_eq!(second.interval_days, 6);
    assert_eq!(second.due_on, day(2026, 9, 20));

    // A lapse puts the card back in tomorrow's queue rather than leaving it a
    // week out, which is the whole point of the ledger.
    let lapsed = flashcards::review(&pool, w.ann, card.id, false, day(2026, 9, 20))
        .await
        .expect("review succeeds")
        .expect("card still exists");
    assert_eq!(lapsed.interval_days, 1);
    assert_eq!(lapsed.due_on, day(2026, 9, 21));
    assert!(lapsed.ease < second.ease);
}

#[sqlx::test(migrations = "../../migrations")]
async fn another_students_card_is_neither_readable_nor_reviewable(pool: PgPool) {
    let w = seed(&pool).await;

    let card = flashcards::create(
        &pool,
        w.ann,
        w.block,
        "front",
        "back",
        None,
        None,
        day(2026, 9, 13),
    )
    .await
    .expect("card inserts");

    assert!(
        flashcards::find_for_student(&pool, w.bob, card.id)
            .await
            .expect("query succeeds")
            .is_none(),
        "Ann's card is indistinguishable from missing for Bob — a 404, never a 403"
    );
    assert!(
        flashcards::review(&pool, w.bob, card.id, true, day(2026, 9, 13))
            .await
            .expect("query succeeds")
            .is_none(),
        "Bob must not be able to advance Ann's schedule"
    );
    assert!(
        flashcards::list_for_student(&pool, w.bob, None, None, 50, 0)
            .await
            .expect("list succeeds")
            .is_empty()
    );
}
