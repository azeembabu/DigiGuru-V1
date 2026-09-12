//! Small enums mirrored 1:1 from Postgres types in `IMPLEMENTATION_PLAN.md`
//! §3.1, kept separate from the RBAC vocabulary in `auth.rs` since they are
//! entity lifecycle states, not permissions.

use serde::{Deserialize, Serialize};

/// Mirrors `user_status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "user_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    Active,
    Inactive,
    Suspended,
}

/// Mirrors `entity_status` — used by `lscs`, `programs`, `semesters`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "entity_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum EntityStatus {
    Active,
    Inactive,
}

/// Mirrors `enrollment_status` — `student_courses.status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "enrollment_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum EnrollmentStatus {
    Active,
    Completed,
    Dropped,
}

/// Mirrors `session_status` — `learning_sessions.status` (Phase 3/4 writes
/// this; the type is defined now so `crates/db`'s struct-only model for
/// `learning_sessions` can use it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "session_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    InProgress,
    Completed,
    Abandoned,
}
