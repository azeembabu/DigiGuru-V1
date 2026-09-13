//! Integration tests for the question pool, the per-attempt paper, and
//! server-side grading (`dg_db::models::{question_pool, exam_papers}`).
//!
//! `sqlx::test` per `.claude/rules/testing.md`: a real, ephemeral Postgres with
//! the repo's migrations applied, no mocking of the database. The two properties
//! that matter most here can only be proved against a real server, because both
//! live in SQL rather than in Rust:
//!
//! * the answer key is unreachable while an attempt is `in_progress`, and
//! * sampling never crosses out of the student's own program and semester
//!   (the exam-module analogue of the metadata-isolation rule).

use dg_core::{
    AssessmentType, BlockId, CourseId, DifficultyLevel, ExamId, LscId, ProgramId, QuestionStatus,
    Role, SemesterId, StudentId, UserId,
};
use dg_db::models::{
    blocks, courses, exam_papers, exams, lscs, programs, question_pool, semesters, student_courses,
    students, users,
};
use sqlx::PgPool;

/// Two programs, each with one semester/course/block, so every "must not leak"
/// assertion has a real neighbour to leak from. Ann is enrolled in the first.
struct World {
    author: UserId,
    ann: StudentId,
    ann_program: ProgramId,
    ann_semester: SemesterId,
    ann_course: CourseId,
    ann_block: BlockId,
    other_program: ProgramId,
    other_course: CourseId,
    other_semester_course: CourseId,
    /// Same program as Ann, different semester — the boundary a block-only
    /// filter would pass by accident.
    other_semester_block: BlockId,
    other_block: BlockId,
    exam_id: ExamId,
    bob: StudentId,
}

fn options() -> Vec<String> {
    ["A", "B", "C", "D"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect()
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
    let lsc = lscs::create(pool, "LSC1", "Centre One", None)
        .await
        .expect("lsc inserts");
    let author = users::create(pool, Role::SuperAdmin, "author@test.local", "hash")
        .await
        .expect("admin inserts")
        .id;
    let author = UserId::from(author.into_uuid());

    let p1 = programs::create(pool, "P1", "Program One", None)
        .await
        .expect("program inserts");
    let s1 = semesters::create(pool, p1.id, 1, "Semester 1")
        .await
        .expect("semester inserts");
    let c1 = courses::create(pool, p1.id, s1.id, "C100", "Course One", None)
        .await
        .expect("course inserts");
    let b1 = blocks::create(pool, c1.id, 1, "Unit 1", None)
        .await
        .expect("block inserts");

    // Ann's own second semester: same program, different semester. This is the
    // boundary a block-only filter would have got right by accident.
    let s1b = semesters::create(pool, p1.id, 2, "Semester 2")
        .await
        .expect("semester inserts");
    let c1b = courses::create(pool, p1.id, s1b.id, "C200", "Course Two", None)
        .await
        .expect("course inserts");
    let other_semester_block = blocks::create(pool, c1b.id, 1, "Unit 1", None)
        .await
        .expect("block inserts");

    let p2 = programs::create(pool, "P2", "Program Two", None)
        .await
        .expect("program inserts");
    let s2 = semesters::create(pool, p2.id, 1, "Semester 1")
        .await
        .expect("semester inserts");
    let c2 = courses::create(pool, p2.id, s2.id, "C900", "Other Course", None)
        .await
        .expect("course inserts");
    let b2 = blocks::create(pool, c2.id, 1, "Unit 1", None)
        .await
        .expect("block inserts");

    let exam = exams::create(
        pool,
        b1.id,
        "Unit 1 Quiz",
        None,
        10.0,
        Some(30),
        dg_core::ExamStatus::Published,
        Some(2),
        Some(AssessmentType::MidTermQuiz),
        author,
    )
    .await
    .expect("exam inserts");

    let ann = student(
        pool,
        "ann@test.local",
        "Ann",
        "P1A0001",
        p1.id,
        s1.id,
        lsc.id,
    )
    .await;
    let bob = student(
        pool,
        "bob@test.local",
        "Bob",
        "P1A0002",
        p1.id,
        s1.id,
        lsc.id,
    )
    .await;
    student_courses::assign(pool, ann, c1.id)
        .await
        .expect("enrolment inserts");

    World {
        author,
        ann,
        ann_program: p1.id,
        ann_semester: s1.id,
        ann_course: c1.id,
        ann_block: b1.id,
        other_program: p2.id,
        other_course: c2.id,
        other_semester_course: c1b.id,
        other_semester_block: other_semester_block.id,
        other_block: b2.id,
        exam_id: exam.id,
        bob,
    }
}

/// Author `n` questions on `block`, all with option index 1 ("B") correct.
async fn author_questions(
    pool: &PgPool,
    course: CourseId,
    block: Option<BlockId>,
    author: UserId,
    topic: &str,
    n: usize,
    kind: AssessmentType,
) {
    for i in 0..n {
        question_pool::create(
            pool,
            course,
            block,
            topic,
            &format!("{topic} question {i}"),
            &options(),
            1,
            "Because B.",
            kind,
            DifficultyLevel::Beginner,
            author,
        )
        .await
        .expect("question inserts");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn sampling_never_leaves_the_exams_own_course(pool: PgPool) {
    let w = seed(&pool).await;

    author_questions(
        &pool,
        w.ann_course,
        Some(w.ann_block),
        w.author,
        "prosody",
        5,
        AssessmentType::MidTermQuiz,
    )
    .await;
    // Another course in the same program and semester, another semester of the
    // same program, and another program entirely. None may reach Ann's paper:
    // the pool boundary is the exam's own course (A2.2).
    author_questions(
        &pool,
        w.other_course,
        Some(w.other_block),
        w.author,
        "trespass",
        5,
        AssessmentType::MidTermQuiz,
    )
    .await;
    author_questions(
        &pool,
        w.other_semester_course,
        Some(w.other_semester_block),
        w.author,
        "next-semester",
        5,
        AssessmentType::MidTermQuiz,
    )
    .await;

    let available =
        question_pool::count_available_for_course(&pool, w.ann_course, AssessmentType::MidTermQuiz)
            .await
            .expect("count succeeds");
    assert_eq!(available, 5, "only the exam's own course pool counts");

    // Sample far more than exist: a filter leak would show up as extra rows.
    let sampled =
        question_pool::sample_for_course(&pool, w.ann_course, AssessmentType::MidTermQuiz, 100)
            .await
            .expect("sample succeeds");

    assert_eq!(sampled.len(), 5);
    assert!(
        sampled.iter().all(|q| q.topic == "prosody"),
        "a question from outside the exam's course reached the paper"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn sampling_only_draws_the_requested_assessment_type(pool: PgPool) {
    let w = seed(&pool).await;

    author_questions(
        &pool,
        w.ann_course,
        Some(w.ann_block),
        w.author,
        "quiz",
        3,
        AssessmentType::MidTermQuiz,
    )
    .await;
    author_questions(
        &pool,
        w.ann_course,
        Some(w.ann_block),
        w.author,
        "final",
        3,
        AssessmentType::SemesterExam,
    )
    .await;

    let sampled =
        question_pool::sample_for_course(&pool, w.ann_course, AssessmentType::MidTermQuiz, 50)
            .await
            .expect("sample succeeds");

    assert_eq!(sampled.len(), 3);
    assert!(sampled.iter().all(|q| q.topic == "quiz"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn retired_questions_are_never_sampled(pool: PgPool) {
    let w = seed(&pool).await;
    author_questions(
        &pool,
        w.ann_course,
        Some(w.ann_block),
        w.author,
        "prosody",
        2,
        AssessmentType::MidTermQuiz,
    )
    .await;

    let listed = question_pool::list(
        &pool,
        &question_pool::QuestionFilter {
            course_id: Some(w.ann_course),
            ..Default::default()
        },
        50,
        0,
    )
    .await
    .expect("list succeeds");

    question_pool::update(
        &pool,
        listed[0].id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(QuestionStatus::Retired),
    )
    .await
    .expect("retire succeeds");

    let sampled =
        question_pool::sample_for_course(&pool, w.ann_course, AssessmentType::MidTermQuiz, 50)
            .await
            .expect("sample succeeds");

    assert_eq!(sampled.len(), 1, "a retired question is not drawable");
}

#[sqlx::test(migrations = "../../migrations")]
async fn an_unsubmitted_paper_never_exposes_the_answer_key(pool: PgPool) {
    let w = seed(&pool).await;
    author_questions(
        &pool,
        w.ann_course,
        Some(w.ann_block),
        w.author,
        "prosody",
        2,
        AssessmentType::MidTermQuiz,
    )
    .await;

    let sampled =
        question_pool::sample_for_course(&pool, w.ann_course, AssessmentType::MidTermQuiz, 2)
            .await
            .expect("sample succeeds");
    let ids: Vec<_> = sampled.iter().map(|q| q.id.into_uuid()).collect();

    let attempt = exam_papers::start_attempt(&pool, w.ann, w.exam_id, 10.0, &ids)
        .await
        .expect("attempt starts");

    let paper = exam_papers::paper_for_student(&pool, w.ann, attempt.id)
        .await
        .expect("paper reads");
    assert_eq!(paper.len(), 2);
    assert_eq!(paper[0].question_seq, 1, "sequence is dense and 1-based");
    assert!(paper.iter().all(|q| q.options.len() == 4));
    assert!(paper.iter().all(|q| q.selected_option_index.is_none()));

    // The graded read — the only one carrying `correct_option_index` — must
    // return nothing at all while the attempt is in progress.
    let graded = exam_papers::graded_paper(&pool, w.ann, attempt.id)
        .await
        .expect("graded read succeeds");
    assert!(
        graded.is_empty(),
        "the answer key must be unreachable before submission"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn another_student_can_neither_read_nor_answer_a_paper(pool: PgPool) {
    let w = seed(&pool).await;
    author_questions(
        &pool,
        w.ann_course,
        Some(w.ann_block),
        w.author,
        "prosody",
        2,
        AssessmentType::MidTermQuiz,
    )
    .await;

    let sampled =
        question_pool::sample_for_course(&pool, w.ann_course, AssessmentType::MidTermQuiz, 2)
            .await
            .expect("sample succeeds");
    let ids: Vec<_> = sampled.iter().map(|q| q.id.into_uuid()).collect();
    let attempt = exam_papers::start_attempt(&pool, w.ann, w.exam_id, 10.0, &ids)
        .await
        .expect("attempt starts");

    assert!(
        exam_papers::paper_for_student(&pool, w.bob, attempt.id)
            .await
            .expect("query succeeds")
            .is_empty(),
        "Ann's paper is not readable with Bob's identity"
    );

    let written = exam_papers::save_answers(&pool, w.bob, attempt.id, &[1], &[Some(0)])
        .await
        .expect("save succeeds");
    assert_eq!(written, 0, "Bob must not be able to answer Ann's paper");

    assert!(
        exam_papers::grade(&pool, w.bob, attempt.id)
            .await
            .expect("grade call succeeds")
            .is_none(),
        "Bob must not be able to submit Ann's paper"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn grading_scores_from_the_stored_key_and_rolls_up_weak_topics(pool: PgPool) {
    let w = seed(&pool).await;
    // Two topics, so an incorrect answer has a topic to be attributed to and a
    // correct one has a topic that must NOT appear in the weak list.
    author_questions(
        &pool,
        w.ann_course,
        Some(w.ann_block),
        w.author,
        "prosody",
        1,
        AssessmentType::MidTermQuiz,
    )
    .await;
    author_questions(
        &pool,
        w.ann_course,
        Some(w.ann_block),
        w.author,
        "metre",
        1,
        AssessmentType::MidTermQuiz,
    )
    .await;

    let sampled =
        question_pool::sample_for_course(&pool, w.ann_course, AssessmentType::MidTermQuiz, 2)
            .await
            .expect("sample succeeds");
    let ids: Vec<_> = sampled.iter().map(|q| q.id.into_uuid()).collect();
    let attempt = exam_papers::start_attempt(&pool, w.ann, w.exam_id, 10.0, &ids)
        .await
        .expect("attempt starts");
    assert_eq!(attempt.attempt_no, 1);

    // Answer the first question correctly (key is index 1) and leave the second
    // unanswered: an unanswered question is simply wrong, not un-gradable.
    let written = exam_papers::save_answers(&pool, w.ann, attempt.id, &[1], &[Some(1)])
        .await
        .expect("save succeeds");
    assert_eq!(written, 1);

    let outcome = exam_papers::grade(&pool, w.ann, attempt.id)
        .await
        .expect("grade succeeds")
        .expect("an in-progress attempt grades");

    assert_eq!(outcome.total_questions, 2);
    assert_eq!(outcome.correct_answers, 1);
    assert!(
        (outcome.score - 5.0).abs() < f64::EPSILON,
        "score is max_score * correct / total, not a client-supplied number"
    );
    let unanswered_topic = sampled[1].topic.clone();
    assert_eq!(
        outcome.weak_topics,
        vec![unanswered_topic],
        "only the topic of the wrong answer is a weak topic"
    );

    // Now, and only now, the key is readable.
    let review = exam_papers::graded_paper(&pool, w.ann, attempt.id)
        .await
        .expect("graded read succeeds");
    assert_eq!(review.len(), 2);
    assert!(review.iter().all(|q| q.correct_option_index == 1));
    assert_eq!(review[0].is_correct, Some(true));
    assert_eq!(review[1].is_correct, Some(false));
}

#[sqlx::test(migrations = "../../migrations")]
async fn submitting_twice_does_not_regrade(pool: PgPool) {
    let w = seed(&pool).await;
    author_questions(
        &pool,
        w.ann_course,
        Some(w.ann_block),
        w.author,
        "prosody",
        1,
        AssessmentType::MidTermQuiz,
    )
    .await;

    let sampled =
        question_pool::sample_for_course(&pool, w.ann_course, AssessmentType::MidTermQuiz, 1)
            .await
            .expect("sample succeeds");
    let ids: Vec<_> = sampled.iter().map(|q| q.id.into_uuid()).collect();
    let attempt = exam_papers::start_attempt(&pool, w.ann, w.exam_id, 10.0, &ids)
        .await
        .expect("attempt starts");

    assert!(exam_papers::grade(&pool, w.ann, attempt.id)
        .await
        .expect("first grade succeeds")
        .is_some());

    assert!(
        exam_papers::grade(&pool, w.ann, attempt.id)
            .await
            .expect("second grade call succeeds")
            .is_none(),
        "a submitted attempt is a 409, not a second opinion"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_submitted_paper_cannot_be_answered_again(pool: PgPool) {
    let w = seed(&pool).await;
    author_questions(
        &pool,
        w.ann_course,
        Some(w.ann_block),
        w.author,
        "prosody",
        1,
        AssessmentType::MidTermQuiz,
    )
    .await;

    let sampled =
        question_pool::sample_for_course(&pool, w.ann_course, AssessmentType::MidTermQuiz, 1)
            .await
            .expect("sample succeeds");
    let ids: Vec<_> = sampled.iter().map(|q| q.id.into_uuid()).collect();
    let attempt = exam_papers::start_attempt(&pool, w.ann, w.exam_id, 10.0, &ids)
        .await
        .expect("attempt starts");
    exam_papers::grade(&pool, w.ann, attempt.id)
        .await
        .expect("grade succeeds");

    let written = exam_papers::save_answers(&pool, w.ann, attempt.id, &[1], &[Some(3)])
        .await
        .expect("save succeeds");
    assert_eq!(written, 0, "a graded paper is not editable");
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_second_attempt_takes_the_next_number_and_its_own_paper(pool: PgPool) {
    let w = seed(&pool).await;
    author_questions(
        &pool,
        w.ann_course,
        Some(w.ann_block),
        w.author,
        "prosody",
        4,
        AssessmentType::MidTermQuiz,
    )
    .await;

    let first =
        question_pool::sample_for_course(&pool, w.ann_course, AssessmentType::MidTermQuiz, 2)
            .await
            .expect("sample");
    let a1 = exam_papers::start_attempt(
        &pool,
        w.ann,
        w.exam_id,
        10.0,
        &first.iter().map(|q| q.id.into_uuid()).collect::<Vec<_>>(),
    )
    .await
    .expect("attempt starts");
    exam_papers::grade(&pool, w.ann, a1.id)
        .await
        .expect("grade");

    let second =
        question_pool::sample_for_course(&pool, w.ann_course, AssessmentType::MidTermQuiz, 2)
            .await
            .expect("sample");
    let a2 = exam_papers::start_attempt(
        &pool,
        w.ann,
        w.exam_id,
        10.0,
        &second.iter().map(|q| q.id.into_uuid()).collect::<Vec<_>>(),
    )
    .await
    .expect("second attempt starts");

    assert_eq!(
        a2.attempt_no, 2,
        "attempt_no is max + 1, computed server-side"
    );
    assert!(exam_papers::active_attempt(&pool, w.ann, w.exam_id)
        .await
        .expect("lookup")
        .is_some_and(|id| id == a2.id));
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_student_only_sees_published_exams_for_courses_they_are_enrolled_in(pool: PgPool) {
    let w = seed(&pool).await;

    // A second exam on the same block, left in draft.
    exams::create(
        &pool,
        w.ann_block,
        "Draft Quiz",
        None,
        10.0,
        None,
        dg_core::ExamStatus::Draft,
        Some(2),
        Some(AssessmentType::MidTermQuiz),
        w.author,
    )
    .await
    .expect("draft exam inserts");

    let rows = exams::list_published_for_student(&pool, w.ann, 50, 0)
        .await
        .expect("list succeeds");
    assert_eq!(rows.len(), 1, "only the published exam is sittable");
    assert_eq!(rows[0].id, w.exam_id);
    assert_eq!(rows[0].attempts_used, 0);
    assert!(
        rows[0].best_percentage.is_none(),
        "no graded attempt is None, not 0"
    );

    // Bob is not enrolled in the course, so the same exam is invisible to him —
    // a student reaches content only through `student_courses`.
    assert!(exams::list_published_for_student(&pool, w.bob, 50, 0)
        .await
        .expect("list succeeds")
        .is_empty());
    assert!(
        exams::published_for_student(&pool, w.bob, w.exam_id)
            .await
            .expect("lookup succeeds")
            .is_none(),
        "an unenrolled student's exam lookup is indistinguishable from missing"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_bulk_import_with_one_bad_row_writes_nothing(pool: PgPool) {
    let w = seed(&pool).await;
    let opts = serde_json::json!(["A", "B", "C", "D"]);

    // Second row's answer key is out of range: the whole import must be
    // rejected, because a half-imported pool can be sampled from.
    let err = question_pool::create_bulk(
        &pool,
        &[w.ann_course.into_uuid(), w.ann_course.into_uuid()],
        &[Some(w.ann_block.into_uuid()), None],
        &["t1".into(), "t2".into()],
        &["q1".into(), "q2".into()],
        &[opts.clone(), opts],
        &[0, 9],
        &["because".into(), "because".into()],
        &["assignment".into(), "assignment".into()],
        &["beginner".into(), "beginner".into()],
        w.author,
    )
    .await;

    assert!(err.is_err(), "an invalid row must abort the import");
    assert_eq!(
        question_pool::count(&pool, &question_pool::QuestionFilter::default())
            .await
            .expect("count succeeds"),
        0,
        "no row from a rejected import may survive"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_question_cannot_be_authored_with_a_key_outside_its_options(pool: PgPool) {
    let w = seed(&pool).await;

    let err = question_pool::create(
        &pool,
        w.ann_course,
        Some(w.ann_block),
        "prosody",
        "Which one?",
        &options(),
        4, // only 0..3 exist
        "Because B.",
        AssessmentType::MidTermQuiz,
        DifficultyLevel::Beginner,
        w.author,
    )
    .await;

    assert!(
        err.is_err(),
        "the answer key must point at an option that exists"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn the_admin_list_is_scoped_to_a_sub_admins_programs(pool: PgPool) {
    let w = seed(&pool).await;
    author_questions(
        &pool,
        w.ann_course,
        Some(w.ann_block),
        w.author,
        "prosody",
        2,
        AssessmentType::MidTermQuiz,
    )
    .await;
    author_questions(
        &pool,
        w.other_course,
        Some(w.other_block),
        w.author,
        "trespass",
        3,
        AssessmentType::MidTermQuiz,
    )
    .await;

    let scope = [w.ann_program.into_uuid()];
    let filter = question_pool::QuestionFilter {
        scope_program_ids: Some(&scope),
        ..Default::default()
    };

    let rows = question_pool::list(&pool, &filter, 50, 0)
        .await
        .expect("list succeeds");
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|q| q.program_id == w.ann_program));
    assert!(
        rows.iter().all(|q| q.program_id != w.other_program),
        "a scoped list must not reach outside the sub-admin's programs"
    );
    assert_eq!(
        question_pool::count(&pool, &filter).await.expect("count"),
        2,
        "X-Total-Count is scoped in SQL before pagination, like the page is"
    );

    // A sub-admin with no scopes sees nothing, which is the correct answer and
    // not a reason to fall back to the unscoped query.
    let empty: [uuid::Uuid; 0] = [];
    assert_eq!(
        question_pool::count(
            &pool,
            &question_pool::QuestionFilter {
                scope_program_ids: Some(&empty),
                ..Default::default()
            }
        )
        .await
        .expect("count"),
        0
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_course_wide_question_needs_no_module_and_is_still_sampled(pool: PgPool) {
    let w = seed(&pool).await;

    // No unit/module link at all — the normal case now the pool is per course.
    let q = question_pool::create(
        &pool,
        w.ann_course,
        None,
        "prosody",
        "Course-wide question",
        &options(),
        1,
        "Because B.",
        AssessmentType::MidTermQuiz,
        DifficultyLevel::Beginner,
        w.author,
    )
    .await
    .expect("a question without a module inserts");

    assert!(q.block_id.is_none(), "the module link is optional");
    assert!(q.block_no.is_none() && q.block_title.is_none());
    assert_eq!(q.course_id, w.ann_course);

    let sampled =
        question_pool::sample_for_course(&pool, w.ann_course, AssessmentType::MidTermQuiz, 10)
            .await
            .expect("sample succeeds");
    assert_eq!(
        sampled.len(),
        1,
        "a course-wide question must be drawable; an inner join on blocks would hide it"
    );

    // It is also visible in the course listing — the LEFT JOIN matters on reads
    // as much as on the draw.
    let listed = question_pool::list(
        &pool,
        &question_pool::QuestionFilter {
            course_id: Some(w.ann_course),
            ..Default::default()
        },
        50,
        0,
    )
    .await
    .expect("list succeeds");
    assert_eq!(listed.len(), 1);

    // Filtering by module returns only module-filed questions, never this one.
    let by_module = question_pool::list(
        &pool,
        &question_pool::QuestionFilter {
            block_id: Some(w.ann_block),
            ..Default::default()
        },
        50,
        0,
    )
    .await
    .expect("list succeeds");
    assert!(by_module.is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn the_admin_list_filters_by_semester_and_course(pool: PgPool) {
    let w = seed(&pool).await;
    author_questions(
        &pool,
        w.ann_course,
        Some(w.ann_block),
        w.author,
        "prosody",
        2,
        AssessmentType::MidTermQuiz,
    )
    .await;
    author_questions(
        &pool,
        w.other_semester_course,
        Some(w.other_semester_block),
        w.author,
        "next-semester",
        3,
        AssessmentType::MidTermQuiz,
    )
    .await;

    let in_semester = question_pool::count(
        &pool,
        &question_pool::QuestionFilter {
            semester_id: Some(w.ann_semester),
            ..Default::default()
        },
    )
    .await
    .expect("count succeeds");
    assert_eq!(
        in_semester, 2,
        "the semester filter reaches through courses"
    );

    let in_course = question_pool::count(
        &pool,
        &question_pool::QuestionFilter {
            course_id: Some(w.other_semester_course),
            ..Default::default()
        },
    )
    .await
    .expect("count succeeds");
    assert_eq!(in_course, 3);
}

#[sqlx::test(migrations = "../../migrations")]
async fn per_course_pool_counts_include_the_empty_courses(pool: PgPool) {
    let w = seed(&pool).await;
    author_questions(
        &pool,
        w.ann_course,
        None,
        w.author,
        "prosody",
        4,
        AssessmentType::MidTermQuiz,
    )
    .await;

    let counts = question_pool::pool_counts_by_course(&pool, w.ann_program)
        .await
        .expect("counts succeed");

    // Both of Ann's program's courses appear — the empty one is the whole point
    // of the screen this feeds.
    assert_eq!(counts.len(), 2);
    let filled = counts
        .iter()
        .find(|c| c.course_id == w.ann_course)
        .expect("the populated course is listed");
    assert_eq!(filled.active_questions, 4);
    let empty = counts
        .iter()
        .find(|c| c.course_id == w.other_semester_course)
        .expect("the empty course is listed too");
    assert_eq!(empty.active_questions, 0);
}
