//! Integration tests for the admin Student Reports reads
//! (`dg_db::models::student_reports`, contract A2.4).
//!
//! The derived figures are what need a real server: `time_spent_seconds` is
//! computed from two timestamps rather than stored, the per-attempt tallies are
//! correlated aggregates that a careless `GROUP BY` would silently multiply, and
//! the sub-admin scope is a SQL predicate rather than a filter in Rust.

use chrono::{Duration, Utc};
use dg_core::{
    AssessmentType, CourseId, DifficultyLevel, ExamAttemptStatus, ExamId, ExamStatus, LscId,
    ProgramId, Role, SemesterId, StudentId, UserId,
};
use dg_db::models::{
    blocks, courses, exam_papers, exams, lscs, programs, question_pool, semesters, student_reports,
    students, users,
};
use sqlx::PgPool;

struct World {
    ann: StudentId,
    ann_program: ProgramId,
    ann_course: CourseId,
    bob: StudentId,
    bob_program: ProgramId,
    ann_exam: ExamId,
    bob_exam: ExamId,
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

/// Two programs, one student and one exam each, so every scope assertion has a
/// real neighbour to leak from.
async fn seed(pool: &PgPool) -> World {
    let lsc = lscs::create(pool, "LSC1", "Centre One", None)
        .await
        .expect("lsc inserts");
    let author = users::create(pool, Role::SuperAdmin, "author@test.local", "hash")
        .await
        .expect("admin inserts")
        .id;
    let author = UserId::from(author.into_uuid());

    let mut made = Vec::new();
    for (code, name) in [("P1", "Program One"), ("P2", "Program Two")] {
        let p = programs::create(pool, code, name, None)
            .await
            .expect("program inserts");
        let sm = semesters::create(pool, p.id, 1, "Semester 1")
            .await
            .expect("semester inserts");
        let c = courses::create(pool, p.id, sm.id, &format!("{code}C1"), "Course", None)
            .await
            .expect("course inserts");
        let b = blocks::create(pool, c.id, 1, "Unit 1", None)
            .await
            .expect("block inserts");
        let e = exams::create(
            pool,
            b.id,
            "Quiz",
            None,
            10.0,
            Some(30),
            ExamStatus::Published,
            Some(2),
            Some(AssessmentType::MidTermQuiz),
            author,
        )
        .await
        .expect("exam inserts");
        made.push((p.id, sm.id, c.id, e.id));
    }

    let (p1, s1, c1, e1) = made[0];
    let (p2, s2, _c2, e2) = made[1];

    // Two questions per course; option index 1 is always the key.
    for (course, topic) in [(c1, "prosody"), (made[1].2, "trespass")] {
        for i in 0..2 {
            question_pool::create(
                pool,
                course,
                None,
                topic,
                &format!("{topic} {i}"),
                &["A", "B", "C", "D"]
                    .iter()
                    .map(|s| (*s).to_owned())
                    .collect::<Vec<_>>(),
                1,
                "Because B.",
                AssessmentType::MidTermQuiz,
                DifficultyLevel::Beginner,
                author,
            )
            .await
            .expect("question inserts");
        }
    }

    let ann = student(pool, "ann@test.local", "Ann", "P1A0001", p1, s1, lsc.id).await;
    let bob = student(pool, "bob@test.local", "Bob", "P2A0001", p2, s2, lsc.id).await;

    World {
        ann,
        ann_program: p1,
        ann_course: c1,
        bob,
        bob_program: p2,
        ann_exam: e1,
        bob_exam: e2,
    }
}

/// Sit one exam: sample the course pool, answer `correct` of the questions
/// correctly, and submit. Returns nothing — every assertion reads it back
/// through the report queries, which is the point.
async fn sit(pool: &PgPool, student: StudentId, exam: ExamId, course: CourseId, correct: usize) {
    let sampled = question_pool::sample_for_course(pool, course, AssessmentType::MidTermQuiz, 2)
        .await
        .expect("sample succeeds");
    let ids: Vec<_> = sampled.iter().map(|q| q.id.into_uuid()).collect();
    let attempt = exam_papers::start_attempt(pool, student, exam, 10.0, &ids)
        .await
        .expect("attempt starts");

    // Question 1 right or wrong as asked; question 2 always wrong, so there is
    // always a weak topic to aggregate.
    let chosen = if correct > 0 { Some(1) } else { Some(0) };
    exam_papers::save_answers(pool, student, attempt.id, &[1, 2], &[chosen, Some(0)])
        .await
        .expect("save succeeds");

    // Backdate the start so the derived duration is a real, non-zero number
    // rather than a sub-second artefact of the test running fast.
    sqlx::query!(
        "UPDATE exam_attempts SET started_at = $2 WHERE id = $1",
        attempt.id.into_uuid(),
        Utc::now() - Duration::seconds(412)
    )
    .execute(pool)
    .await
    .expect("backdate succeeds");

    exam_papers::grade(pool, student, attempt.id)
        .await
        .expect("grade succeeds")
        .expect("the attempt grades");
}

#[sqlx::test(migrations = "../../migrations")]
async fn an_attempt_report_row_carries_its_derived_tallies_and_duration(pool: PgPool) {
    let w = seed(&pool).await;
    sit(&pool, w.ann, w.ann_exam, w.ann_course, 1).await;

    let rows = student_reports::list_attempt_reports(
        &pool,
        &student_reports::AttemptReportFilter::default(),
        50,
        0,
    )
    .await
    .expect("list succeeds");

    assert_eq!(rows.len(), 1);
    let r = &rows[0];
    assert_eq!(r.student_name, "Ann");
    assert_eq!(r.program_name, "Program One");
    assert_eq!(r.total_questions, 2);
    assert_eq!(r.answered_questions, 2);
    assert_eq!(r.correct_answers, 1);
    assert_eq!(r.status, ExamAttemptStatus::Graded);
    assert_eq!(r.score, Some(5.0));
    assert_eq!(r.percentage, Some(50.0));
    assert_eq!(
        r.time_spent_seconds,
        Some(412),
        "the duration is derived from the two timestamps, not stored"
    );
    assert_eq!(r.weak_topics, vec!["prosody".to_owned()]);
    assert_eq!(r.assessment_type, Some(AssessmentType::MidTermQuiz));

    assert_eq!(
        student_reports::count_attempt_reports(
            &pool,
            &student_reports::AttemptReportFilter::default()
        )
        .await
        .expect("count succeeds"),
        1,
        "X-Total-Count applies the same filters as the page"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn an_in_progress_attempt_reports_no_score_percentage_or_duration(pool: PgPool) {
    let w = seed(&pool).await;

    let sampled =
        question_pool::sample_for_course(&pool, w.ann_course, AssessmentType::MidTermQuiz, 2)
            .await
            .expect("sample succeeds");
    let ids: Vec<_> = sampled.iter().map(|q| q.id.into_uuid()).collect();
    exam_papers::start_attempt(&pool, w.ann, w.ann_exam, 10.0, &ids)
        .await
        .expect("attempt starts");

    let rows = student_reports::list_attempt_reports(
        &pool,
        &student_reports::AttemptReportFilter::default(),
        50,
        0,
    )
    .await
    .expect("list succeeds");

    let r = &rows[0];
    assert!(r.score.is_none(), "null, not a measured zero");
    assert!(r.percentage.is_none());
    assert!(r.time_spent_seconds.is_none(), "nothing has been submitted");
    assert_eq!(r.answered_questions, 0);
    assert!(
        r.weak_topics.is_empty(),
        "before grading, which answers are wrong is part of the answer key"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_sub_admin_only_sees_attempts_from_its_own_programs(pool: PgPool) {
    let w = seed(&pool).await;
    sit(&pool, w.ann, w.ann_exam, w.ann_course, 1).await;
    sit(
        &pool,
        w.bob,
        w.bob_exam,
        {
            // Bob's own course, reached the same way the fixture built it.
            let row = sqlx::query!(
                r#"SELECT b.course_id as "course_id!" FROM exams e
               JOIN blocks b ON b.id = e.block_id WHERE e.id = $1"#,
                w.bob_exam.into_uuid()
            )
            .fetch_one(&pool)
            .await
            .expect("course resolves");
            CourseId::from(row.course_id)
        },
        2,
    )
    .await;

    let scope = [w.ann_program.into_uuid()];
    let filter = student_reports::AttemptReportFilter {
        scope_program_ids: Some(&scope),
        ..Default::default()
    };

    let rows = student_reports::list_attempt_reports(&pool, &filter, 50, 0)
        .await
        .expect("list succeeds");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].student_id, w.ann);
    assert!(rows.iter().all(|r| r.program_id != w.bob_program));
    assert_eq!(
        student_reports::count_attempt_reports(&pool, &filter)
            .await
            .expect("count succeeds"),
        1
    );

    // A sub-admin with no scopes gets nothing, never a platform-wide fallback.
    let none: [uuid::Uuid; 0] = [];
    let empty = student_reports::AttemptReportFilter {
        scope_program_ids: Some(&none),
        ..Default::default()
    };
    assert!(student_reports::list_attempt_reports(&pool, &empty, 50, 0)
        .await
        .expect("list succeeds")
        .is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_student_report_rolls_up_courses_and_weak_topics(pool: PgPool) {
    let w = seed(&pool).await;
    // Two sittings, both missing the same topic, so the aggregate counts rather
    // than merely lists.
    sit(&pool, w.ann, w.ann_exam, w.ann_course, 1).await;
    sit(&pool, w.ann, w.ann_exam, w.ann_course, 0).await;

    let header = student_reports::report_header(&pool, w.ann, None)
        .await
        .expect("header reads")
        .expect("the student exists");
    assert_eq!(header.roll_number, "P1A0001");
    assert_eq!(header.lsc_code.as_deref(), Some("LSC1"));
    assert_eq!(header.attempts_total, 2);
    assert_eq!(header.attempts_graded, 2);
    assert_eq!(header.best_percentage, Some(50.0));
    assert_eq!(header.average_percentage, Some(25.0));
    assert_eq!(
        header.total_time_spent_seconds, 824,
        "durations sum from the timestamps of both attempts"
    );

    let by_course = student_reports::rollup_by_course(&pool, w.ann)
        .await
        .expect("rollup reads");
    assert_eq!(by_course.len(), 1);
    assert_eq!(by_course[0].course_id, w.ann_course);
    assert_eq!(by_course[0].attempts, 2);
    assert_eq!(by_course[0].average_percentage, Some(25.0));

    let weak = student_reports::weak_topics(&pool, w.ann, 10)
        .await
        .expect("weak topics read");
    assert_eq!(weak.len(), 1);
    assert_eq!(weak[0].topic, "prosody");
    assert_eq!(
        weak[0].missed_count, 3,
        "one miss in the first sitting and two in the second, counted not deduplicated"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_student_outside_the_callers_scope_reads_as_missing(pool: PgPool) {
    let w = seed(&pool).await;

    let scope = [w.ann_program.into_uuid()];
    assert!(
        student_reports::report_header(&pool, w.bob, Some(&scope))
            .await
            .expect("query succeeds")
            .is_none(),
        "out of scope must be indistinguishable from missing — a 404, never a 403"
    );
    assert!(student_reports::report_header(&pool, w.ann, Some(&scope))
        .await
        .expect("query succeeds")
        .is_some());
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_student_with_no_attempts_reports_nulls_not_zeroes(pool: PgPool) {
    let w = seed(&pool).await;

    let header = student_reports::report_header(&pool, w.ann, None)
        .await
        .expect("header reads")
        .expect("the student exists");

    assert_eq!(header.attempts_total, 0);
    assert!(
        header.average_percentage.is_none() && header.best_percentage.is_none(),
        "nothing to average is null; zero would be a different and wrong claim"
    );
    assert_eq!(header.total_time_spent_seconds, 0);
    assert!(student_reports::rollup_by_course(&pool, w.ann)
        .await
        .expect("rollup reads")
        .is_empty());
    assert!(student_reports::weak_topics(&pool, w.ann, 10)
        .await
        .expect("weak topics read")
        .is_empty());
}
