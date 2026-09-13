//! `POST /api/v1/auth/signup` — student self-registration.
//!
//! `IMPLEMENTATION_PLAN.md` §4.1 item 3: captures name, roll number,
//! program, semester, LSC, phone, email; every field is validated
//! server-side and program/semester/LSC must resolve to real rows, not
//! free text. `is_first_login` defaults to `TRUE` via the column default.
//!
//! **Registration completes the student's onboarding; no administrator has to
//! approve it.** Two things follow from that, and both are done here rather
//! than left for someone to do later:
//!
//! * The response carries the same session cookies `/auth/login` issues, so the
//!   student lands on their dashboard instead of being bounced to a login form
//!   to retype the password they just chose.
//! * The student is enrolled in every course of the semester they picked. A
//!   student reaches content only through `student_courses` (`CLAUDE.md`), so
//!   without this the "direct access" is a dashboard with no courses on it and
//!   no classroom they are allowed to enter.
//!
//! This is *not* a weakening of authorization. `student_courses` is still the
//! single enforcement point and still decides every content query; what changed
//! is who writes the row for a self-registered student. An administrator
//! retains full control afterwards — enrolments can be updated or dropped, and
//! `users.status` can be set to `inactive`/`suspended`, which `/auth/login`
//! already refuses.

use axum::{extract::State, http::HeaderMap, http::StatusCode, Json};
use axum_extra::extract::CookieJar;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use dg_core::{FieldError, LscId, ProgramId, PublicError, Role, SemesterId};
use dg_db::models::{auth_sessions, lscs, programs, semesters, student_courses, students, users};

use crate::auth::cookies::{access_cookie, refresh_cookie, REFRESH_TOKEN_TTL_DAYS};
use crate::auth::jwt::issue_access_token;
use crate::auth::password::hash_password;
use crate::auth::tokens::{generate_token, hash_token};
use crate::extractors::JsonBody;
use crate::state::AppState;
use crate::validation::{normalize_indian_phone, validate_roll_number};

#[derive(Debug, Deserialize, Validate)]
pub struct SignupRequest {
    #[validate(length(min = 2, max = 120, message = "must be 2-120 characters"))]
    pub full_name: String,
    pub roll_number: String,
    pub phone_number: String,
    #[validate(email(message = "must be a valid email address"))]
    pub email: String,
    #[validate(length(min = 8, max = 128, message = "must be 8-128 characters"))]
    pub password: String,
    pub program_id: Uuid,
    pub semester_id: Uuid,
    pub lsc_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct SignupResponse {
    pub user_id: Uuid,
    pub student_id: Uuid,
    /// Always `"student"` — signup creates no other role. Present so a client
    /// can route on one field regardless of whether it came from login or
    /// signup.
    pub role: Role,
    /// Always `true` for a new account. Mirrors `LoginResponse` so the
    /// post-auth routing is identical on both paths (NN-2: the greeting plays
    /// on the first login, and this *is* that login).
    pub is_first_login: bool,
    /// Courses the student was enrolled in, so a client can tell an empty
    /// dashboard caused by an empty semester from one caused by a bug.
    pub enrolled_courses: i64,
}

pub async fn signup(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    JsonBody(payload): JsonBody<SignupRequest>,
) -> Result<(StatusCode, CookieJar, Json<SignupResponse>), PublicError> {
    let mut errors: Vec<FieldError> = Vec::new();

    if let Err(validation_errors) = payload.validate() {
        for (field, field_errors) in validation_errors.field_errors() {
            for fe in field_errors {
                let message = fe
                    .message
                    .clone()
                    .map(|m| m.to_string())
                    .unwrap_or_else(|| "invalid value".to_string());
                errors.push(FieldError::new(field, message));
            }
        }
    }

    if let Err(fe) = validate_roll_number(&payload.roll_number) {
        errors.push(fe);
    }

    let normalized_phone = match normalize_indian_phone(&payload.phone_number) {
        Ok(phone) => Some(phone),
        Err(fe) => {
            errors.push(fe);
            None
        }
    };

    // `normalized_phone` is only `None` when an error was already pushed
    // above; check that first so this never needs `unwrap()`/`expect()` in
    // the request path (`.claude/rules/code-style.md`).
    let Some(phone_number) = normalized_phone else {
        return Err(PublicError::Validation(errors));
    };
    if !errors.is_empty() {
        return Err(PublicError::Validation(errors));
    }

    let program_id = ProgramId::from(payload.program_id);
    let semester_id = SemesterId::from(payload.semester_id);
    let lsc_id = LscId::from(payload.lsc_id);

    // Program / semester / LSC must resolve to real, active rows.
    if !programs::exists(&state.pool, program_id).await.map_err(PublicError::from)? {
        return Err(PublicError::Unprocessable {
            code: "INVALID_PROGRAM",
            message: "Selected program does not exist.".into(),
        });
    }
    if !semesters::belongs_to_program(&state.pool, semester_id, program_id)
        .await
        .map_err(PublicError::from)?
    {
        return Err(PublicError::Unprocessable {
            code: "INVALID_SEMESTER",
            message: "Selected semester does not belong to the selected program.".into(),
        });
    }
    if lscs::find_by_id(&state.pool, lsc_id).await.map_err(PublicError::from)?.is_none() {
        return Err(PublicError::Unprocessable {
            code: "INVALID_LSC",
            message: "Selected learner support centre does not exist.".into(),
        });
    }

    // Uniqueness, checked explicitly so the response can name the field
    // (the DB's UNIQUE constraints are the authoritative backstop under a
    // race, surfaced via `dg_db::Error::UniqueViolation` -> `PublicError`).
    if users::find_by_email(&state.pool, &payload.email)
        .await
        .map_err(PublicError::from)?
        .is_some()
    {
        return Err(PublicError::validation("email", "already registered"));
    }
    if students::roll_number_taken(&state.pool, &payload.roll_number)
        .await
        .map_err(PublicError::from)?
    {
        return Err(PublicError::validation("roll_number", "already taken"));
    }

    let password_hash = hash_password(&payload.password).map_err(|err| {
        tracing::error!(error = %err, "password hashing failed");
        PublicError::Internal
    })?;

    let user = users::create(&state.pool, Role::Student, &payload.email, &password_hash)
        .await
        .map_err(PublicError::from)?;

    let student = students::create(
        &state.pool,
        user.id,
        &payload.full_name,
        &payload.roll_number,
        &phone_number,
        program_id,
        semester_id,
        lsc_id,
    )
    .await
    .map_err(PublicError::from)?;

    // Enrolment. Best-effort by design: the account exists and is usable at
    // this point, and failing the request now would report an error for an
    // email that is already registered — leaving the student unable to retry
    // and unable to log in to the thing they just created. A failure here is a
    // database fault (the insert is a single statement against rows that were
    // validated above), so it is logged at error level and an administrator can
    // assign the courses by hand.
    let enrolled_courses = match student_courses::enroll_in_semester(
        &state.pool,
        student.id,
        semester_id,
    )
    .await
    {
        Ok(count) => count as i64,
        Err(err) => {
            tracing::error!(
                error = %err,
                student_id = %student.id,
                semester_id = %semester_id,
                "signup succeeded but semester auto-enrolment failed; the student has no courses"
            );
            0
        }
    };

    // Where the classroom opens. Same reasoning as the enrolment above and the
    // same best-effort handling: without it `/me/context` reports no
    // `current_block` and the student has courses but nowhere to start.
    if let Err(err) = students::start_at_semester(&state.pool, student.id, semester_id).await {
        tracing::error!(
            error = %err,
            student_id = %student.id,
            "signup succeeded but the starting block could not be set"
        );
    }

    // The session, issued exactly as `/auth/login` issues it — same access
    // token, same rotating refresh token hashed into `auth_sessions`, same
    // cookie flags. Duplicating the *flags* would be a security bug waiting to
    // happen, so both paths go through `cookies::{access_cookie, refresh_cookie}`.
    let access_token =
        issue_access_token(user.id.into_uuid(), user.role, &state.config.jwt_access_secret)
            .map_err(|err| {
                tracing::error!(error = %err, "access token issuance failed");
                PublicError::Internal
            })?;

    let refresh_raw = generate_token();
    let refresh_hash = hash_token(&state.config.jwt_refresh_secret, &refresh_raw);
    let device_info = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    auth_sessions::create(
        &state.pool,
        user.id,
        &refresh_hash,
        device_info.as_deref(),
        None,
        Utc::now() + Duration::days(REFRESH_TOKEN_TTL_DAYS),
    )
    .await
    .map_err(PublicError::from)?;

    crate::audit::log(
        &state,
        Some(user.id),
        "auth.signup",
        None,
        device_info.as_deref(),
        Some(serde_json::json!({
            "student_id": student.id,
            "program_id": program_id,
            "enrolled_courses": enrolled_courses,
        })),
    )
    .await;

    let jar = jar
        .add(access_cookie(access_token))
        .add(refresh_cookie(refresh_raw));

    Ok((
        StatusCode::CREATED,
        jar,
        Json(SignupResponse {
            user_id: user.id.into_uuid(),
            student_id: student.id.into_uuid(),
            role: user.role,
            is_first_login: student.is_first_login,
            enrolled_courses,
        }),
    ))
}
