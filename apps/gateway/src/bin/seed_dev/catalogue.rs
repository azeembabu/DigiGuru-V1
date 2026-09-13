//! Stage 1 of `seed_dev`: the academic catalogue and everyone attached to it.
//!
//! Program > Semester > Course > Block, plus LSCs, students, enrolments and
//! documents. Everything lands under ONE program so the sub-admin scoping
//! path is genuinely exercised rather than assumed.
//!
//! **Idempotency strategy: deterministic ids + `ON CONFLICT DO NOTHING`.**
//! Every row's primary key is derived from a stable string key (see
//! `det_id`), so a second run re-derives exactly the same ids, every insert
//! becomes a no-op, and the natural-key UNIQUE constraints (`programs.code`,
//! `users.email`, `students.roll_number`, `documents.sha256`) are never hit
//! with a *different* id. The alternative -- detect-and-bail -- would have
//! forced the operator to hand-delete a program whose `students` rows are
//! `ON DELETE RESTRICT`, which is a worse first-run experience.
//!
//! Consequently nothing here uses a random number generator: the shape of
//! the data is a pure function of the row index, or a re-run would produce
//! different content behind identical ids.

use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{SeedOpts, SeededCatalogue};

/// Natural key of the seeded program. Also the idempotency anchor.
const PROGRAM_CODE: &str = "BAML";

const SEMESTERS: usize = 4;
const COURSES_PER_SEMESTER: usize = 3;
const BLOCKS_PER_COURSE: usize = 4;

const COURSE_NAMES: [&str; 3] = ["Malayalam Poetry", "Prose and the Essay", "Language and Linguistics"];

const BLOCK_TITLES: [&str; 16] = [
    "പ്രാചീന മലയാള കവിത — Early Malayalam Poetry",
    "എഴുത്തച്ഛനും കിളിപ്പാട്ടും — Ezhuthachan and the Kilippattu",
    "ചെറുശ്ശേരിയുടെ കൃഷ്ണഗാഥ — Cherusseri's Krishnagatha",
    "കുഞ്ചൻ നമ്പ്യാരും തുള്ളൽ സാഹിത്യവും — Kunchan Nambiar and Thullal",
    "ആധുനിക കവിത്രയം — The Modern Triumvirate",
    "കുമാരനാശാന്റെ ഖണ്ഡകാവ്യങ്ങൾ — Kumaran Asan's Narrative Poems",
    "വള്ളത്തോളും ദേശീയതയും — Vallathol and Nationalism",
    "ചങ്ങമ്പുഴയും റൊമാന്റിസിസവും — Changampuzha and Romanticism",
    "മലയാള നോവലിന്റെ ഉദയം — The Rise of the Malayalam Novel",
    "ഇന്ദുലേഖ: ഒരു പഠനം — Indulekha: A Close Reading",
    "തകഴിയും സാമൂഹിക യാഥാർത്ഥ്യവും — Thakazhi and Social Realism",
    "ബഷീറിന്റെ ഭാഷ — The Language of Basheer",
    "ചെറുകഥയുടെ ഘടന — Structure of the Short Story",
    "നാടകവും അരങ്ങും — Drama and the Stage",
    "മലയാള ഭാഷാചരിത്രം — History of the Malayalam Language",
    "വ്യാകരണവും ശൈലിയും — Grammar and Style",
];

const LSCS: [(&str, &str, &str); 4] = [
    ("LSC-TVM", "Thiruvananthapuram Learner Support Centre", "Thiruvananthapuram"),
    ("LSC-EKM", "Ernakulam Learner Support Centre", "Ernakulam"),
    ("LSC-KZD", "Kozhikode Learner Support Centre", "Kozhikode"),
    ("LSC-TSR", "Thrissur Learner Support Centre", "Thrissur"),
];

const GIVEN_NAMES: [&str; 12] = [
    "Anjali", "Vishnu", "Fathima", "Rahul", "Meera", "Arun", "Sreelakshmi", "Nikhil", "Aiswarya",
    "Jithin", "Deepa", "Sabu",
];
const FAMILY_NAMES: [&str; 8] = [
    "Nair", "Menon", "Pillai", "Kurup", "Thomas", "Varghese", "Rahman", "Das",
];

/// Document statuses, weighted by repetition: mostly `embedded`, a little
/// review backlog, the odd in-flight job and one failure -- the shape a real
/// block library has rather than an even split across five states.
const DOC_STATES: [&str; 16] = [
    "embedded", "embedded", "embedded", "embedded", "pending_review", "embedded", "embedded",
    "failed", "embedded", "embedded", "pending", "embedded", "pending_review", "embedded",
    "parsing", "embedded",
];

/// Deterministic UUID from a namespaced key.
///
/// Hand-rolled rather than `Uuid::new_v5` because the workspace `uuid` is
/// declared with only the `v4` feature, and this file does not get to widen a
/// shared dependency. The bit-fiddling makes it a well-formed v5-shaped UUID.
fn det_id(kind: &str, key: &str) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(b"digiguru-seed-dev\0");
    hasher.update(kind.as_bytes());
    hasher.update(b"\0");
    hasher.update(key.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50; // version 5
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // RFC 4122 variant
    Uuid::from_bytes(bytes)
}

/// A stable pseudo-random spread in `0..n`, derived from the row index.
/// Deterministic by design -- see the module note on idempotency.
fn spread(index: usize, salt: u64, n: usize) -> usize {
    let mut x = (index as u64)
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(salt);
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 29;
    (x % n.max(1) as u64) as usize
}

fn days_ago(d: i64, index: usize) -> DateTime<Utc> {
    // Scatter within the day too, so a per-day chart is not a comb of
    // identical timestamps.
    Utc::now() - Duration::days(d) - Duration::minutes(spread(index, 991, 1400) as i64)
}

fn roman(n: i16) -> &'static str {
    match n {
        1 => "I",
        2 => "II",
        3 => "III",
        _ => "IV",
    }
}

pub async fn seed(pool: &PgPool, opts: &SeedOpts) -> SeededCatalogue {
    // One transaction for the whole catalogue: a half-seeded hierarchy
    // (courses with no program, students with no semester) is worse than no
    // seed at all, because the dashboard would render it as real data.
    let mut tx = pool.begin().await.expect("failed to open a transaction");

    // ---- Program, semesters, courses, blocks -------------------------------
    let program_id = det_id("program", PROGRAM_CODE);
    sqlx::query!(
        r#"INSERT INTO programs (id, code, name, description, status)
           VALUES ($1, $2, $3, $4, 'active') ON CONFLICT DO NOTHING"#,
        program_id,
        PROGRAM_CODE,
        "BA Malayalam",
        "Three-year distance-education undergraduate programme in Malayalam language and literature.",
    )
    .execute(&mut *tx)
    .await
    .expect("failed to insert the program");

    let mut semester_ids: Vec<Uuid> = Vec::with_capacity(SEMESTERS);
    let mut course_ids: Vec<Uuid> = Vec::new();
    let mut blocks: Vec<(Uuid, Uuid)> = Vec::new();
    // Courses grouped by semester, so enrolment stays inside a student's own
    // semester rather than scattering across the whole programme.
    let mut courses_by_semester: Vec<Vec<Uuid>> = Vec::with_capacity(SEMESTERS);

    for s in 0..SEMESTERS {
        let sem_no = (s + 1) as i16;
        let semester_id = det_id("semester", &format!("{PROGRAM_CODE}/{sem_no}"));
        sqlx::query!(
            r#"INSERT INTO semesters (id, program_id, semester_number, name, status)
               VALUES ($1, $2, $3, $4, 'active') ON CONFLICT DO NOTHING"#,
            semester_id,
            program_id,
            sem_no,
            format!("Semester {sem_no}"),
        )
        .execute(&mut *tx)
        .await
        .expect("failed to insert a semester");
        semester_ids.push(semester_id);

        let mut sem_courses = Vec::with_capacity(COURSES_PER_SEMESTER);
        for c in 0..COURSES_PER_SEMESTER {
            let code = format!("MAL{}{:02}", sem_no, c + 1);
            let course_id = det_id("course", &format!("{PROGRAM_CODE}/{code}"));
            sqlx::query!(
                r#"INSERT INTO courses (id, program_id, semester_id, code, name, description)
                   VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING"#,
                course_id,
                program_id,
                semester_id,
                code,
                format!("{} {}", COURSE_NAMES[c], roman(sem_no)),
                format!("Semester {sem_no} core paper: {}.", COURSE_NAMES[c]),
            )
            .execute(&mut *tx)
            .await
            .expect("failed to insert a course");
            course_ids.push(course_id);
            sem_courses.push(course_id);

            // The final course of the final semester is deliberately left
            // empty so `courses_without_blocks` is non-zero and the
            // "unfinished catalogue" warning has something real to show.
            if s == SEMESTERS - 1 && c == COURSES_PER_SEMESTER - 1 {
                continue;
            }

            for b in 0..BLOCKS_PER_COURSE {
                let block_no = (b + 1) as i16;
                let block_id = det_id("block", &format!("{code}/{block_no}"));
                let title = BLOCK_TITLES[(s * COURSES_PER_SEMESTER * BLOCKS_PER_COURSE
                    + c * BLOCKS_PER_COURSE
                    + b)
                    % BLOCK_TITLES.len()];
                // Retired units exist in a real catalogue; mixing status is
                // what makes blocks_active differ from blocks_inactive.
                let status = if blocks.len() % 7 == 5 {
                    "inactive"
                } else {
                    "active"
                };
                sqlx::query!(
                    r#"INSERT INTO blocks (id, course_id, block_no, title, description, status)
                       VALUES ($1, $2, $3, $4, $5, $6::text::entity_status)
                       ON CONFLICT DO NOTHING"#,
                    block_id,
                    course_id,
                    block_no,
                    title,
                    format!("Unit {block_no} of {code}."),
                    status,
                )
                .execute(&mut *tx)
                .await
                .expect("failed to insert a block");
                blocks.push((block_id, course_id));
            }
        }
        courses_by_semester.push(sem_courses);
    }
    println!(
        "catalogue: 1 program, {} semesters, {} courses, {} blocks",
        semester_ids.len(),
        course_ids.len(),
        blocks.len()
    );

    // ---- Learner support centres -------------------------------------------
    let mut lsc_ids: Vec<Uuid> = Vec::with_capacity(LSCS.len());
    for (code, name, location) in LSCS {
        let id = det_id("lsc", code);
        sqlx::query!(
            r#"INSERT INTO lscs (id, code, name, location, status)
               VALUES ($1, $2, $3, $4, 'active') ON CONFLICT DO NOTHING"#,
            id,
            code,
            name,
            location,
        )
        .execute(&mut *tx)
        .await
        .expect("failed to insert an LSC");
        lsc_ids.push(id);
    }
    println!("catalogue: {} LSCs", lsc_ids.len());

    // ---- Students ----------------------------------------------------------
    // One hash reused for every seeded student: argon2id is deliberately slow,
    // and re-hashing the same dev-only password 40 times would dominate the
    // runtime for no benefit on a disposable local database.
    let password_hash =
        crate::password::hash_password(&opts.student_password).expect("failed to hash the password");

    let mut student_ids: Vec<Uuid> = Vec::with_capacity(opts.students);
    for i in 0..opts.students {
        let email = format!("seed.student{i:03}@digiguru.local");
        let user_id = det_id("student-user", &email);
        let student_id = det_id("student", &email);
        let semester_index = spread(i, 17, SEMESTERS);
        let semester_id = semester_ids[semester_index];
        let lsc_id = lsc_ids[spread(i, 29, lsc_ids.len())];

        // A dormant and a suspended cohort, so the status KPI is not one bar.
        let status = match spread(i, 41, 10) {
            0 => "suspended",
            1 | 2 => "inactive",
            _ => "active",
        };
        // Registrations trickle in over two months; without this every
        // "new students per day" bar would land on today.
        let created_at = days_ago(spread(i, 53, 60) as i64, i);

        sqlx::query!(
            r#"INSERT INTO users (id, role, status, email, password_hash, created_at, updated_at)
               VALUES ($1, 'student', $2::text::user_status, $3::text::citext, $4, $5, $5)
               ON CONFLICT DO NOTHING"#,
            user_id,
            status,
            email,
            password_hash,
            created_at,
        )
        .execute(&mut *tx)
        .await
        .expect("failed to insert a student users row");

        let full_name = format!(
            "{} {}",
            GIVEN_NAMES[spread(i, 61, GIVEN_NAMES.len())],
            FAMILY_NAMES[spread(i, 67, FAMILY_NAMES.len())]
        );
        // Never greeted yet (NN-2) for roughly a third of the cohort.
        let is_first_login = spread(i, 71, 3) == 0;
        // Context persistence: only a student who has already studied
        // something has a current block.
        let current_block_id = if is_first_login {
            None
        } else {
            Some(blocks[spread(i, 73, blocks.len())].0)
        };

        sqlx::query!(
            r#"INSERT INTO students (id, user_id, full_name, roll_number, phone_number,
                                     program_id, semester_id, lsc_id, current_block_id,
                                     is_first_login, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $11)
               ON CONFLICT DO NOTHING"#,
            student_id,
            user_id,
            full_name,
            format!("{PROGRAM_CODE}24{i:04}"),
            format!("+9198{:08}", 40_000_000 + spread(i, 79, 9_000_000)),
            program_id,
            semester_id,
            lsc_id,
            current_block_id,
            is_first_login,
            created_at,
        )
        .execute(&mut *tx)
        .await
        .expect("failed to insert a students row");
        student_ids.push(student_id);

        // Every ninth student is left unenrolled on purpose: "students with
        // no course assigned" is its own dashboard metric and a real
        // onboarding gap worth surfacing.
        if i % 9 == 4 {
            continue;
        }
        for (n, course_id) in courses_by_semester[semester_index].iter().enumerate() {
            let enrolment = match spread(i + n * 101, 83, 12) {
                0 => "dropped",
                1 | 2 => "completed",
                _ => "active",
            };
            sqlx::query!(
                r#"INSERT INTO student_courses (id, student_id, course_id, status, assigned_at)
                   VALUES ($1, $2, $3, $4::text::enrollment_status, $5)
                   ON CONFLICT DO NOTHING"#,
                det_id("enrolment", &format!("{student_id}/{course_id}")),
                student_id,
                course_id,
                enrolment,
                created_at,
            )
            .execute(&mut *tx)
            .await
            .expect("failed to insert a student_courses row");
        }
    }
    println!("catalogue: {} students", student_ids.len());

    // ---- Documents ---------------------------------------------------------
    // `documents.uploaded_by` is a FK to a real user. Reuse whichever admin
    // the operator already bootstrapped rather than minting a second account
    // with a known password (`.claude/rules/security.md`).
    let uploader: Option<Uuid> = sqlx::query_scalar!(
        r#"SELECT id FROM users WHERE role IN ('super_admin', 'sub_admin')
           ORDER BY created_at LIMIT 1"#
    )
    .fetch_optional(&mut *tx)
    .await
    .expect("failed to look for an admin user");

    let mut document_ids: Vec<Uuid> = Vec::new();
    match uploader {
        None => println!(
            "documents: SKIPPED -- no admin user exists to own an upload. Run `cargo run -p \
             gateway --bin create_admin ...` first, then re-run this seeder."
        ),
        Some(uploaded_by) => {
            for (n, (block_id, _)) in blocks.iter().enumerate() {
                // A couple of blocks are left bare so
                // `blocks_without_documents` is a real number a sub-admin can
                // act on.
                if n % 11 == 3 {
                    continue;
                }
                let status = DOC_STATES[n % DOC_STATES.len()];
                let key = format!("{block_id}/study-material");
                let document_id = det_id("document", &key);
                // `sha256` is UNIQUE: derive it from the same key so a re-run
                // collides with itself, never with a sibling document.
                let sha256 = format!("{:x}", Sha256::digest(key.as_bytes()));
                // Scanned reprints carry an OCR score; born-digital PDFs have
                // none, which is why the column is nullable.
                let ocr_confidence: Option<f32> = match n % 4 {
                    0 => None,
                    1 => Some(0.71), // low enough to explain a pending_review
                    _ => Some(0.93 + (spread(n, 97, 60) as f32) / 1000.0),
                };
                let created_at = days_ago(spread(n, 103, opts.days.max(1) as usize) as i64, n);

                sqlx::query!(
                    r#"INSERT INTO documents (id, block_id, uploaded_by, title, storage_key,
                                              sha256, page_count, ocr_confidence, status,
                                              created_at)
                       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                       ON CONFLICT DO NOTHING"#,
                    document_id,
                    block_id,
                    uploaded_by,
                    format!("Study Material — Unit {}", (n % BLOCKS_PER_COURSE) + 1),
                    format!("blocks/{block_id}/{sha256}.pdf"),
                    sha256,
                    (24 + spread(n, 107, 120)) as i32,
                    ocr_confidence,
                    status,
                    created_at,
                )
                .execute(&mut *tx)
                .await
                .expect("failed to insert a document");

                // The job row is the document's ingestion history; a console
                // reads `last_error` from here because `documents` has no
                // error column of its own.
                let (job_status, attempts, last_error) = match status {
                    "pending" => ("pending", 0, None),
                    "parsing" => ("processing", 1, None),
                    "failed" => (
                        "failed",
                        3,
                        Some("pdfium: page 12 could not be rasterised (corrupt xref table)"),
                    ),
                    // `pending_review` is a *human* gate: the job itself
                    // completed, the OCR score merely fell below the floor.
                    _ => ("completed", 1, None),
                };
                sqlx::query!(
                    r#"INSERT INTO ingestion_jobs (id, document_id, status, attempts, last_error,
                                                   created_at, updated_at)
                       VALUES ($1, $2, $3::text::ingestion_job_status, $4, $5, $6, $6)
                       ON CONFLICT DO NOTHING"#,
                    det_id("ingestion-job", &key),
                    document_id,
                    job_status,
                    attempts,
                    last_error,
                    created_at,
                )
                .execute(&mut *tx)
                .await
                .expect("failed to insert an ingestion_jobs row");

                document_ids.push(document_id);
            }
            println!(
                "catalogue: {} documents (+ ingestion jobs)",
                document_ids.len()
            );
        }
    }

    // ---- Sub-admin scope ---------------------------------------------------
    if let Some(email) = opts.scope_admin_email.as_deref() {
        let target = sqlx::query!(
            r#"SELECT id, role::text AS "role!" FROM users WHERE email = $1::text::citext"#,
            email
        )
        .fetch_optional(&mut *tx)
        .await
        .expect("failed to look up the scope admin");

        match target {
            Some(row) if row.role == "sub_admin" => {
                sqlx::query!(
                    r#"INSERT INTO sub_admin_scopes (user_id, program_id) VALUES ($1, $2)
                       ON CONFLICT DO NOTHING"#,
                    row.id,
                    program_id,
                )
                .execute(&mut *tx)
                .await
                .expect("failed to grant the sub-admin scope");
                println!("scope: granted program {program_id} to sub_admin {}", row.id);
            }
            // A missing or wrong-role scope target is a warning, never a
            // failure: the catalogue above is still worth having.
            Some(row) => println!(
                "scope: WARNING -- user {} is a {}, not a sub_admin; no scope granted, so the \
                 scoped-vs-platform-wide split will not be exercised.",
                row.id, row.role
            ),
            None => println!(
                "scope: WARNING -- no user with that email exists; no scope granted. Create one \
                 with `cargo run -p gateway --bin create_admin -- --role sub_admin ...`."
            ),
        }
    }

    // Ids and counts only -- never a name, email, roll number or phone number
    // (`.claude/rules/security.md`, H-43).
    sqlx::query!(
        r#"INSERT INTO audit_logs (user_id, action, device_info, metadata)
           VALUES (NULL, 'dev.seed_catalogue', 'seed_dev CLI', $1)"#,
        serde_json::json!({
            "program_id": program_id,
            "semesters": semester_ids.len(),
            "courses": course_ids.len(),
            "blocks": blocks.len(),
            "lscs": lsc_ids.len(),
            "students": student_ids.len(),
            "documents": document_ids.len(),
        }),
    )
    .execute(&mut *tx)
    .await
    .expect("failed to write the audit log row");

    tx.commit().await.expect("failed to commit the catalogue");

    SeededCatalogue {
        program_id,
        semester_ids,
        course_ids,
        blocks,
        student_ids,
        lsc_ids,
        document_ids,
    }
}
