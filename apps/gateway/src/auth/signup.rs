//! `POST /api/v1/auth/signup` — student self-registration.
//!
//! `IMPLEMENTATION_PLAN.md` §4.1 item 3: captures name, roll number,
//! program, semester, LSC, phone, email; every field is validated
//! server-side and program/semester/LSC must resolve to real rows, not
//! free text. `is_first_login` defaults to `TRUE` via the column default.

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use dg_core::{FieldError, LscId, ProgramId, PublicError, Role, SemesterId};
use dg_db::models::{lscs, programs, semesters, students, users};

use crate::auth::password::hash_password;
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
}

pub async fn signup(
    State(state): State<AppState>,
    Json(payload): Json<SignupRequest>,
) -> Result<(StatusCode, Json<SignupResponse>), PublicError> {
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

    crate::audit::log(
        &state,
        Some(user.id),
        "auth.signup",
        None,
        None,
        Some(serde_json::json!({ "student_id": student.id, "program_id": program_id })),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(SignupResponse {
            user_id: user.id.into_uuid(),
            student_id: student.id.into_uuid(),
        }),
    ))
}
