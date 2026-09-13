//! `seed_dev` -- fills a LOCAL database with realistic demo data.
//!
//! Why a binary and not a migration: `migrations/` is forward-only and runs
//! everywhere, production included. Demo students, fake sessions and a known
//! password have no business in that path. This tool is run by hand, against
//! a developer's own database, and refuses to touch anything else.
//!
//! The data is shaped for the admin dashboard (`GET /api/v1/admin/analytics`):
//! a full Program > Semester > Course > Block hierarchy, students spread
//! across it, documents in every ingestion state, and 30 days of tutoring
//! activity so the charts have a shape rather than a flat line.
//!
//! Everything is scoped under ONE program, which is also granted to a
//! sub-admin, so the scoped-vs-platform-wide split is actually exercised
//! rather than assumed.
//!
//! `expect`/`panic!` are used freely: this is a CLI, not the request or audio
//! path (`.claude/rules/code-style.md` permits both outside those).

use sqlx::PgPool;
use uuid::Uuid;

mod activity;
mod catalogue;

#[path = "../../auth/password.rs"]
#[allow(dead_code)]
mod password;

/// Knobs the two seeding stages share.
pub struct SeedOpts {
    /// How many students to create under the seeded program.
    pub students: usize,
    /// How many days of tutoring history to synthesise, ending today.
    pub days: i64,
    /// Login password for every seeded student. Dev-only by construction.
    pub student_password: String,
    /// Sub-admin to grant the seeded program to, by email.
    pub scope_admin_email: Option<String>,
}

/// What `catalogue` produced, so `activity` can hang sessions off real rows
/// instead of re-querying for them.
pub struct SeededCatalogue {
    pub program_id: Uuid,
    pub semester_ids: Vec<Uuid>,
    pub course_ids: Vec<Uuid>,
    /// `(block_id, course_id)` -- `learning_sessions` needs both.
    pub blocks: Vec<(Uuid, Uuid)>,
    pub student_ids: Vec<Uuid>,
    pub lsc_ids: Vec<Uuid>,
    pub document_ids: Vec<Uuid>,
}

const USAGE: &str = "\
Seed a LOCAL Digi Guru database with demo data for the admin dashboard.

USAGE:
    seed_dev [--students <N>] [--days <N>] [--scope-admin <EMAIL>] [--force]

OPTIONS:
    --students <N>        Students to create (default 40).
    --days <N>            Days of tutoring history (default 30).
    --scope-admin <EMAIL> Grant the seeded program to this sub-admin.
                          Default: subadmin@digiguru.local if it exists.
    --student-password <P>  Password for seeded students (default: SeedStudent-Dev-1!).
    --force               Seed even if DATABASE_URL is not local. Do not use
                          this against anything you care about.
    -h, --help            Print this help.
";

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

    let mut students = 40usize;
    let mut days = 30i64;
    let mut scope_admin: Option<String> = None;
    let mut student_password = "SeedStudent-Dev-1!".to_string();
    let mut force = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                return;
            }
            "--students" => students = next_val(&mut args, "--students").parse().expect("--students must be a number"),
            "--days" => days = next_val(&mut args, "--days").parse().expect("--days must be a number"),
            "--scope-admin" => scope_admin = Some(next_val(&mut args, "--scope-admin")),
            "--student-password" => student_password = next_val(&mut args, "--student-password"),
            "--force" => force = true,
            other => {
                eprintln!("unexpected argument '{other}'\n\n{USAGE}");
                std::process::exit(2);
            }
        }
    }

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL is not set");

    // A seeder that can reach a shared database is a seeder that will
    // eventually be pointed at one by accident. Refuse unless it is local.
    let local = database_url.contains("localhost") || database_url.contains("127.0.0.1");
    if !local && !force {
        eprintln!(
            "refusing to seed: DATABASE_URL does not look local. Pass --force only if you are \
             certain this database is disposable."
        );
        std::process::exit(2);
    }

    let pool = PgPool::connect(&database_url)
        .await
        .expect("failed to connect to Postgres");

    let opts = SeedOpts {
        students,
        days,
        student_password,
        scope_admin_email: scope_admin.or_else(|| Some("subadmin@digiguru.local".to_string())),
    };

    let cat = catalogue::seed(&pool, &opts).await;
    activity::seed(&pool, &cat, &opts).await;

    println!(
        "\nSeeded program {} -- {} semesters, {} courses, {} blocks, {} students, {} documents.",
        cat.program_id,
        cat.semester_ids.len(),
        cat.course_ids.len(),
        cat.blocks.len(),
        cat.student_ids.len(),
        cat.document_ids.len()
    );
    println!("Student login password: {}", opts.student_password);
}

fn next_val(args: &mut impl Iterator<Item = String>, flag: &str) -> String {
    args.next()
        .unwrap_or_else(|| panic!("{flag} requires a value"))
}
