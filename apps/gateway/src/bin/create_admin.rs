//! `create_admin` — the first-super-admin bootstrap.
//!
//! The chicken-and-egg this solves: `POST /api/v1/admin/users` requires an
//! existing super-admin, `POST /api/v1/auth/signup` only ever creates
//! students, and no migration or seed inserts an admin. Without this there
//! is no way for anyone to sign into the admin console the first time.
//!
//! **Deliberately a binary, not a route.** A "create the first admin"
//! endpoint is reachable over the network and has to be disabled again the
//! moment it succeeds, which is exactly the kind of switch that gets left
//! on. Running this requires shell access to a host that already holds
//! `DATABASE_URL`.
//!
//! **No credential is committed anywhere.** `.claude/rules/security.md`
//! forbids production secrets in files, and a seeded admin hash in
//! `migrations/seed/` is precisely that — a published password for every
//! deployment that ever ran the seed. The operator supplies the password at
//! run time and it is never written to disk, logged, or echoed.
//!
//! `expect`/`panic!` are used freely below: this is a CLI, not the request
//! or audio path (`.claude/rules/code-style.md` allows both outside those).

use std::collections::BTreeSet;
use std::io::{IsTerminal, Write};

use dg_core::{ProgramId, Role};
use dg_db::models::{admins, audit_logs, programs, sub_admin_scopes, users};
use uuid::Uuid;

// The *same* hashing code the signup path uses, included by path rather
// than copied: `apps/gateway` has no library target, so a second binary
// cannot `use` a module of `main.rs`. Including the file keeps one argon2id
// implementation in the repo — a re-implementation here could silently
// diverge in parameters and produce hashes `auth/login.rs` cannot verify.
#[path = "../auth/password.rs"]
// `verify_password` is the login path's half of that module; including the
// file brings it along unused, which is the price of not duplicating the
// hashing parameters.
#[allow(dead_code)]
mod password;

const USAGE: &str = "\
Create the first admin user for Digi Guru.

USAGE:
    create_admin --email <EMAIL> --full-name <NAME> --role <ROLE> [--scope <PROGRAM_ID>]...

OPTIONS:
    --email <EMAIL>           Login email. Must not already exist.
    --full-name <NAME>        Display name (2-120 characters) for the `admins` row.
    --role <ROLE>             `super_admin` or `sub_admin`. `student` is rejected.
    --scope <PROGRAM_ID>      Program UUID a sub-admin may act on. Repeatable.
                              Required at least once for `sub_admin` unless
                              --allow-no-scopes is given. Ignored roles: none —
                              a super_admin must have no scopes at all.
    --allow-no-scopes         Create a sub-admin with no scopes anyway. It will
                              be able to see nothing until scopes are granted.
    --password-stdin          Read the password from stdin instead of prompting.
    -h, --help                Print this help.

PASSWORD (never a command-line argument, so it cannot land in shell history):
    1. $ADMIN_PASSWORD, if set; otherwise
    2. the first line of stdin, with --password-stdin; otherwise
    3. an interactive prompt (typed twice, not echoed).
";

struct Args {
    email: String,
    full_name: String,
    role: Role,
    scopes: Vec<Uuid>,
    allow_no_scopes: bool,
    password_stdin: bool,
}

fn main() {
    // Same as the gateway: load a local `.env` if present so a developer
    // does not have to re-export `DATABASE_URL` by hand. Never required.
    let _ = dotenvy::dotenv();

    let args = parse_args();
    let password = read_password(args.password_stdin);

    validate(&args, &password);

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL is not set (put it in .env or export it)");

    let runtime = tokio::runtime::Runtime::new().expect("failed to start the tokio runtime");
    runtime.block_on(run(args, password, database_url));
}

fn parse_args() -> Args {
    let mut email = None;
    let mut full_name = None;
    let mut role = None;
    let mut scopes: Vec<Uuid> = Vec::new();
    let mut allow_no_scopes = false;
    let mut password_stdin = false;

    let mut argv = std::env::args().skip(1);
    while let Some(flag) = argv.next() {
        match flag.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            "--email" => email = Some(next_value(&mut argv, "--email")),
            "--full-name" => full_name = Some(next_value(&mut argv, "--full-name")),
            "--role" => role = Some(next_value(&mut argv, "--role")),
            "--scope" => {
                let raw = next_value(&mut argv, "--scope");
                let id: Uuid = raw
                    .parse()
                    .unwrap_or_else(|_| fail(&format!("--scope '{raw}' is not a UUID")));
                scopes.push(id);
            }
            "--allow-no-scopes" => allow_no_scopes = true,
            "--password-stdin" => password_stdin = true,
            // A bare positional is almost certainly someone passing the
            // password the way this tool refuses to accept it.
            other => fail(&format!(
                "unexpected argument '{other}'. The password is never an argument — see --help"
            )),
        }
    }

    let role = match role.as_deref() {
        Some("super_admin") => Role::SuperAdmin,
        Some("sub_admin") => Role::SubAdmin,
        Some("student") => fail("role 'student' is rejected — students are created via POST /api/v1/auth/signup"),
        Some(other) => fail(&format!("unknown role '{other}' (expected super_admin or sub_admin)")),
        None => fail("--role is required (super_admin or sub_admin)"),
    };

    Args {
        email: email.unwrap_or_else(|| fail("--email is required")),
        full_name: full_name.unwrap_or_else(|| fail("--full-name is required")),
        role,
        scopes,
        allow_no_scopes,
        password_stdin,
    }
}

fn next_value(argv: &mut impl Iterator<Item = String>, flag: &str) -> String {
    argv.next()
        .unwrap_or_else(|| fail(&format!("{flag} needs a value")))
}

/// Resolve the password without ever reading it from `argv`.
fn read_password(from_stdin: bool) -> String {
    if let Ok(from_env) = std::env::var("ADMIN_PASSWORD") {
        if !from_env.is_empty() {
            return from_env;
        }
    }

    if from_stdin {
        let mut line = String::new();
        std::io::stdin()
            .read_line(&mut line)
            .expect("failed to read the password from stdin");
        return line.trim_end_matches(['\r', '\n']).to_string();
    }

    if !std::io::stdin().is_terminal() {
        fail("no terminal to prompt on — set ADMIN_PASSWORD or pass --password-stdin");
    }

    let first = prompt_hidden("Password: ");
    let second = prompt_hidden("Confirm password: ");
    if first != second {
        fail("passwords did not match");
    }
    first
}

fn prompt_hidden(label: &str) -> String {
    print!("{label}");
    std::io::stdout().flush().expect("failed to flush stdout");
    rpassword::read_password().expect("failed to read the password")
}

/// Everything checkable before touching the database, so a typo fails in
/// milliseconds instead of after an argon2id hash.
fn validate(args: &Args, password: &str) {
    // Deliberately looser than a full RFC 5322 parse — matching
    // `admin/users.rs`'s `validator::email` exactly is not worth a
    // dependency here, and a wrong address fails at first login, loudly.
    if !args.email.contains('@') || args.email.starts_with('@') || args.email.ends_with('@') {
        fail("--email does not look like an email address");
    }

    // Same bounds as `CreateAdminRequest` in `admin/users.rs`, so an account
    // made here is one the API would also have accepted.
    let name_len = args.full_name.trim().chars().count();
    if !(2..=120).contains(&name_len) {
        fail("--full-name must be 2-120 characters");
    }
    let password_len = password.chars().count();
    if !(8..=128).contains(&password_len) {
        fail("password must be 8-128 characters");
    }

    if args.role == Role::SuperAdmin && !args.scopes.is_empty() {
        fail("a super_admin is unscoped by definition — drop the --scope arguments");
    }

    if args.role == Role::SubAdmin && args.scopes.is_empty() && !args.allow_no_scopes {
        fail(
            "a sub_admin with no --scope can see nothing at all: every scoped capability \
             fails closed, so it would get 403 on every program, student, and document. \
             Pass --scope <PROGRAM_ID> (repeatable), or --allow-no-scopes to create it anyway",
        );
    }
}

async fn run(args: Args, password: String, database_url: String) {
    let pool = dg_db::create_pool(&database_url)
        .await
        .expect("failed to connect to Postgres");

    // Pre-flight so the common mistake gets a sentence, not a constraint
    // violation. `users.email` is UNIQUE, so a race still cannot produce a
    // duplicate — the transaction below would simply fail.
    if users::find_by_email(&pool, &args.email)
        .await
        .expect("failed to query users")
        .is_some()
    {
        fail(
            "an account with that email already exists. This tool never updates an existing \
             account or resets its password — use the admin console, or pick another email",
        );
    }

    // Deduplicate before validating so `--scope X --scope X` is one grant,
    // not a repeat insert relying on the ON CONFLICT clause.
    let scopes: Vec<ProgramId> = args
        .scopes
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(ProgramId::from)
        .collect();

    for scope in &scopes {
        if !programs::exists(&pool, *scope)
            .await
            .expect("failed to query programs")
        {
            fail(&format!(
                "program {scope} does not exist (or is not active) — check `SELECT id, code FROM programs`"
            ));
        }
    }

    let password_hash = password::hash_password(&password).expect("failed to hash the password");
    drop(password);

    // One transaction: a `users` row without its `admins` row is an account
    // that can log in but has no name anywhere, and a sub-admin whose scope
    // inserts failed is an account that silently sees nothing.
    let mut tx = pool.begin().await.expect("failed to open a transaction");

    let user = users::create(&mut *tx, args.role, &args.email, &password_hash)
        .await
        .expect("failed to insert the users row");

    admins::create(&mut *tx, user.id, args.full_name.trim())
        .await
        .expect("failed to insert the admins row");

    for scope in &scopes {
        sub_admin_scopes::add_scope(&mut *tx, user.id, *scope)
            .await
            .expect("failed to insert a sub_admin_scopes row");
    }

    tx.commit().await.expect("failed to commit");

    // Audit the mutation like every admin write does. `user_id` is the
    // account created rather than an actor: there is no authenticated caller
    // here, and that is itself the fact worth recording. Metadata carries
    // ids and the role only — never the email, name, or password
    // (`.claude/rules/security.md`).
    if let Err(err) = audit_logs::insert(
        &pool,
        Some(user.id),
        "admin.bootstrap_admin_created",
        None,
        Some("create_admin CLI"),
        Some(serde_json::json!({
            "created_user_id": user.id,
            "role": args.role,
            "scope_count": scopes.len(),
        })),
    )
    .await
    {
        eprintln!("warning: the account was created but the audit log write failed: {err}");
    }

    println!("Created {} {}", args.role.as_db_str(), user.id);
    println!("  email:  {}", args.email);
    println!("  scopes: {}", scopes.len());
    if args.role == Role::SubAdmin && scopes.is_empty() {
        println!();
        println!(
            "WARNING: this sub_admin has no scopes. Every scoped capability fails closed, so it \
             will receive 403 on every program, student, and document until a super-admin grants \
             a scope via POST /api/v1/admin/users/{}/scopes.",
            user.id
        );
    }
    println!();
    println!("Log in at POST /api/v1/auth/login with that email and the password you just set.");
}

/// Print a message to stderr and exit non-zero. Never prints the password.
fn fail(message: &str) -> ! {
    eprintln!("error: {message}");
    std::process::exit(1);
}
