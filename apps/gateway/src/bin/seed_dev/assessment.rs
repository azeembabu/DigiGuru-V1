//! Stage 3 of `seed_dev`: the MCQ question pool and the exams that sample it.
//!
//! Without this stage a freshly set-up database has a complete catalogue, real
//! students and thirty days of tutoring history — and an exam engine that
//! cannot serve a single paper. `POST /student/exams/{id}/attempts` samples the
//! **course's** pool and answers `422 INSUFFICIENT_QUESTIONS` when it holds
//! fewer active questions of the paper's `assessment_type` than
//! `question_count` asks for, so an empty pool means every contributor's first
//! encounter with the exam runner is that error rather than a paper.
//!
//! What it guarantees, and why each one matters:
//!
//! * Every course gets a pool for **every** `assessment_type`, each with more
//!   questions than the exam that draws on it. The sampler re-checks the count
//!   after sampling, so a pool sized exactly to `question_count` is one
//!   retirement away from failing.
//! * Every question is A/B/C/D with a `correct_option_index` in `0..=3` and a
//!   non-empty `explanation`. `explanation` is NOT NULL because it drives the
//!   post-exam feedback screen, and a pool seeded with blank ones would make
//!   that screen look broken rather than empty.
//! * Exams are created `published`, because a `draft` exam is invisible to
//!   `GET /student/exams` and a contributor would reasonably read an empty list
//!   as a bug in the endpoint.
//! * `block_id` is left NULL on most questions. A course-wide question is the
//!   normal shape (`api-conventions.md`), and sampling deliberately does not
//!   narrow by module — seeding everything under a module would hide that.
//!
//! Idempotency matches the other stages: this one owns the pool and exams
//! belonging to the seeded program's courses and replaces them, so a second run
//! does not double the pool. Ids are derived deterministically from the course
//! and index, so re-running lands on the same rows.

use sqlx::PgPool;
use uuid::Uuid;

use crate::SeededCatalogue;

/// Questions seeded per course per assessment type.
///
/// Comfortably above `QUESTIONS_PER_EXAM` so the post-sampling re-check has
/// room, and small enough that the pool screen is still readable.
const POOL_PER_TYPE: usize = 12;

/// Questions each seeded exam asks for.
const QUESTIONS_PER_EXAM: i16 = 5;

/// The three `assessment_type` values, with the exam each one produces.
const ASSESSMENTS: [(&str, &str, i16); 3] = [
    ("assignment", "Assignment 1", 20),
    ("mid_term_quiz", "Mid-term quiz", 30),
    ("semester_exam", "Semester examination", 60),
];

const DIFFICULTIES: [&str; 3] = ["beginner", "intermediate", "advanced"];

pub async fn seed(pool: &PgPool, cat: &SeededCatalogue) {
    // `created_by` is a FK to a real user on both tables. Reuse whichever admin
    // the operator bootstrapped rather than minting one with a known password
    // (`.claude/rules/security.md`), exactly as the document stage does.
    let author: Option<Uuid> = sqlx::query_scalar!(
        r#"SELECT id FROM users WHERE role IN ('super_admin', 'sub_admin')
           ORDER BY created_at LIMIT 1"#
    )
    .fetch_optional(pool)
    .await
    .expect("failed to look for an admin user");

    let Some(author) = author else {
        println!(
            "question pool: SKIPPED -- no admin user exists to own the questions. Run \
             `cargo run -p gateway --bin create_admin ...` first, then re-run this seeder."
        );
        return;
    };

    let mut tx = pool.begin().await.expect("failed to open a transaction");

    // Owned-and-replaced, so a second run does not double the pool. Attempts
    // are left alone: they belong to the activity stage and cascade from their
    // own exams if those are replaced.
    sqlx::query!(
        "DELETE FROM question_pool WHERE course_id = ANY($1)",
        &cat.course_ids
    )
    .execute(&mut *tx)
    .await
    .expect("failed to clear the seeded question pool");

    let mut questions = 0usize;
    for (course_index, course_id) in cat.course_ids.iter().enumerate() {
        for (type_index, (assessment_type, _, _)) in ASSESSMENTS.iter().enumerate() {
            for n in 0..POOL_PER_TYPE {
                let key = format!("{course_id}:{assessment_type}:{n}");
                let id = crate::catalogue::det_id("question", &key);
                let correct = ((course_index + type_index + n) % 4) as i16;
                let difficulty = DIFFICULTIES[(n + type_index) % DIFFICULTIES.len()];
                let topic = TOPICS[(course_index + n) % TOPICS.len()];
                let options = serde_json::json!([
                    format!("{topic} — the definition given in the unit"),
                    format!("{topic} — a related idea from a later unit"),
                    format!("{topic} — a common misreading of the term"),
                    format!("{topic} — an unrelated term with a similar name"),
                ]);

                sqlx::query!(
                    r#"INSERT INTO question_pool
                         (id, course_id, block_id, topic, question_text, options,
                          correct_option_index, explanation, assessment_type,
                          difficulty_level, status, created_by)
                       VALUES ($1, $2, NULL, $3, $4, $5, $6, $7,
                               $8::text::assessment_type,
                               $9::text::difficulty_level, 'active', $10)"#,
                    id,
                    course_id,
                    topic,
                    format!(
                        "Seed question {}: which option states what the unit says about {topic}?",
                        n + 1
                    ),
                    options,
                    correct,
                    format!(
                        "Option {} is the definition the unit gives for {topic}; the others \
                         describe adjacent or similarly named ideas.",
                        (b'A' + correct as u8) as char
                    ),
                    *assessment_type,
                    difficulty,
                    author,
                )
                .execute(&mut *tx)
                .await
                .expect("failed to insert a seeded question");
                questions += 1;
            }
        }
    }

    // ---- Exams -------------------------------------------------------------
    // One published paper per assessment type on each course's first block. An
    // exam anchors to a block and reaches its course through it, so the first
    // block of the course is the one that matches the pool being sampled.
    let mut exams = 0usize;
    for course_id in &cat.course_ids {
        let Some((block_id, _)) = cat.blocks.iter().find(|(_, c)| c == course_id) else {
            continue;
        };
        for (assessment_type, title, minutes) in ASSESSMENTS {
            let id = crate::catalogue::det_id("exam", &format!("{block_id}:{assessment_type}"));
            sqlx::query!(
                r#"INSERT INTO exams
                     (id, block_id, title, description, max_score, duration_minutes,
                      status, question_count, assessment_type, created_by)
                   VALUES ($1, $2, $3, $4, $5, $6, 'published', $7,
                           $8::text::assessment_type, $9)
                   ON CONFLICT (block_id, title) DO UPDATE
                     SET status = 'published',
                         question_count = EXCLUDED.question_count,
                         assessment_type = EXCLUDED.assessment_type,
                         duration_minutes = EXCLUDED.duration_minutes,
                         max_score = EXCLUDED.max_score"#,
                id,
                block_id,
                title,
                Some("Seeded for local development.".to_string()),
                f64::from(QUESTIONS_PER_EXAM),
                minutes,
                QUESTIONS_PER_EXAM,
                assessment_type,
                author,
            )
            .execute(&mut *tx)
            .await
            .expect("failed to insert a seeded exam");
            exams += 1;
        }
    }

    tx.commit().await.expect("failed to commit the assessment seed");
    println!("assessment: {questions} questions, {exams} published exams");
}

/// Topic labels, so the weak-area roll-up has something to group by.
///
/// The roll-up is `GROUP BY topic` over incorrectly answered questions, so a
/// pool whose every row shared one topic would make that feature look broken
/// rather than empty.
const TOPICS: [&str; 8] = [
    "Phonology",
    "Script and orthography",
    "Morphology",
    "Syntax",
    "Prosody",
    "Literary criticism",
    "Modern prose",
    "Drama",
];
